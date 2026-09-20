//! Git lane: brokered git operations with read/mutation policy.
//!
//! Pure policy + bounded execution. No network, no ambient authority:
//! every mutating op must resolve under an authorized root, and `push`
//! additionally requires an explicit human-only grant.

#![forbid(unsafe_code)]

use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

/// Explicit marker appended when output is truncated to the byte budget.
pub const TRUNCATION_MARKER: &str = "[git-lane: truncated, output exceeded byte budget]";

/// Default output budget (64 KiB).
pub const MAX_GIT_OUTPUT_BYTES: usize = 64 * 1024;

/// Operation class of a git subcommand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitOp {
    ReadOnly,
    Mutating,
    Push,
}

/// Explicit repository state report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitStatus {
    Clean,
    DirtyTree,
    DetachedHead,
}

/// Outcome of a bounded child-process run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome {
    pub timed_out: bool,
    pub killed: bool,
    pub exit_code: Option<i32>,
}

/// Typed errors for the git lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitLaneError {
    OutsideAuthorizedRoot,
    PushRequiresHumanGrant,
    Denied(String),
    Timeout,
    Cancelled,
    Spawn(String),
}

impl fmt::Display for GitLaneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutsideAuthorizedRoot => write!(f, "path outside authorized root"),
            Self::PushRequiresHumanGrant => write!(f, "push requires explicit human grant"),
            Self::Denied(reason) => write!(f, "denied by policy: {reason}"),
            Self::Timeout => write!(f, "git child timed out and was killed"),
            Self::Cancelled => write!(f, "git child cancelled and was killed"),
            Self::Spawn(detail) => write!(f, "spawn failed: {detail}"),
        }
    }
}

impl std::error::Error for GitLaneError {}

/// Classify a git subcommand (`git <sub> ...`) into an operation class.
/// Unknown subcommands default to [`GitOp::Mutating`] (deny-by-default).
pub fn classify(subcommand: &str) -> GitOp {
    match subcommand {
        "status" | "log" | "diff" | "show" | "rev-parse" | "ls-files" | "blame" | "grep" => {
            GitOp::ReadOnly
        }
        "push" => GitOp::Push,
        _ => GitOp::Mutating,
    }
}

/// Policy gate. Pure: no side effects on deny.
/// `under_root` must come from [`ensure_under_root`]; `human_grant` is the
/// explicit human-only approval for push.
pub fn authorize(op: GitOp, under_root: bool, human_grant: bool) -> Result<(), GitLaneError> {
    if !under_root {
        return Err(GitLaneError::OutsideAuthorizedRoot);
    }
    if op == GitOp::Push && !human_grant {
        return Err(GitLaneError::PushRequiresHumanGrant);
    }
    Ok(())
}

/// Lexically resolve `target` against `root`; error when it escapes `root`.
/// Pure path computation: touches no filesystem, spawns nothing.
pub fn ensure_under_root(root: &Path, target: &Path) -> Result<PathBuf, GitLaneError> {
    let joined = if target.is_absolute() {
        target.to_path_buf()
    } else {
        root.join(target)
    };
    let mut normalized = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(GitLaneError::OutsideAuthorizedRoot);
                }
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    if normalized.starts_with(root) {
        Ok(normalized)
    } else {
        Err(GitLaneError::OutsideAuthorizedRoot)
    }
}

/// Bound output to `budget` bytes at a char boundary, appending
/// [`TRUNCATION_MARKER`] with the dropped byte count. Returns
/// `(bounded, was_truncated)`.
pub fn bound_output(output: &str, budget: usize) -> (String, bool) {
    if output.len() <= budget {
        return (output.to_owned(), false);
    }
    let mut end = budget;
    while end > 0 && !output.is_char_boundary(end) {
        end -= 1;
    }
    let dropped = output.len() - end;
    let mut bounded = output[..end].to_owned();
    bounded.push_str("\n");
    bounded.push_str(TRUNCATION_MARKER);
    bounded.push_str(&format!(" (dropped {dropped} bytes)"));
    (bounded, true)
}

/// Report repository state explicitly. Detached HEAD takes precedence
/// because it changes the meaning of every subsequent mutation.
pub fn report_status(dirty: bool, detached_head: bool) -> GitStatus {
    if detached_head {
        GitStatus::DetachedHead
    } else if dirty {
        GitStatus::DirtyTree
    } else {
        GitStatus::Clean
    }
}

fn kill_child(child: &mut Child) -> bool {
    match child.try_wait() {
        Ok(None) => child.kill().is_ok(),
        _ => false,
    }
}

/// Run a child process with a wall-clock timeout, killing it on expiry.
/// Test with `true`/`sleep` fixtures; never point at real git push/network.
pub fn run_bounded(
    program: &str,
    args: &[&str],
    timeout: Duration,
) -> Result<RunOutcome, GitLaneError> {
    let mut child = Command::new(program)
        .args(args)
        .spawn()
        .map_err(|e| GitLaneError::Spawn(e.to_string()))?;
    let start = Instant::now();
    loop {
        match child.try_wait().map_err(|e| GitLaneError::Spawn(e.to_string()))? {
            Some(status) => {
                return Ok(RunOutcome {
                    timed_out: false,
                    killed: false,
                    exit_code: status.code(),
                });
            }
            None if start.elapsed() >= timeout => {
                let killed = kill_child(&mut child);
                let _ = child.wait();
                return Ok(RunOutcome {
                    timed_out: true,
                    killed,
                    exit_code: None,
                });
            }
            None => std::thread::sleep(Duration::from_millis(5)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn root_denied_no_side_effects() {
        let root = Path::new("/repo/root");
        assert_eq!(
            ensure_under_root(root, Path::new("../escape")),
            Err(GitLaneError::OutsideAuthorizedRoot)
        );
        assert_eq!(
            ensure_under_root(root, Path::new("/elsewhere/x")),
            Err(GitLaneError::OutsideAuthorizedRoot)
        );
        // Policy gate denies before any execution could happen.
        assert_eq!(
            authorize(GitOp::Mutating, false, true),
            Err(GitLaneError::OutsideAuthorizedRoot)
        );
        // In-root path resolves fine.
        assert!(ensure_under_root(root, Path::new("sub/dir")).is_ok());
    }

    #[test]
    fn push_requires_human_grant() {
        assert_eq!(
            authorize(GitOp::Push, true, false),
            Err(GitLaneError::PushRequiresHumanGrant)
        );
        assert_eq!(authorize(GitOp::Push, true, true), Ok(()));
        // Read-only flows under policy without a grant.
        assert_eq!(classify("log"), GitOp::ReadOnly);
        assert_eq!(classify("diff"), GitOp::ReadOnly);
        assert_eq!(authorize(GitOp::ReadOnly, true, false), Ok(()));
        assert_eq!(classify("push"), GitOp::Push);
    }

    #[test]
    fn timeout_kills_child_stable_state() {
        let outcome = run_bounded("true", &[], Duration::from_secs(5))
            .expect("true must exit 0");
        assert_eq!(
            outcome,
            RunOutcome {
                timed_out: false,
                killed: false,
                exit_code: Some(0),
            }
        );
        let slow = run_bounded("sleep", &["30"], Duration::from_millis(100))
            .expect("sleep run must return an outcome");
        assert!(slow.timed_out);
        assert!(slow.killed);
        assert_eq!(slow.exit_code, None);
    }

    #[test]
    fn ambiguous_state_reported_explicitly() {
        assert_eq!(report_status(false, false), GitStatus::Clean);
        assert_eq!(report_status(true, false), GitStatus::DirtyTree);
        assert_eq!(report_status(false, true), GitStatus::DetachedHead);
        // Detached HEAD wins: mutations mean something different there.
        assert_eq!(report_status(true, true), GitStatus::DetachedHead);
    }

    #[test]
    fn large_output_truncates_with_marker() {
        let big = "x".repeat(MAX_GIT_OUTPUT_BYTES + 1024);
        let (bounded, truncated) = bound_output(&big, MAX_GIT_OUTPUT_BYTES);
        assert!(truncated);
        assert!(bounded.contains(TRUNCATION_MARKER));
        assert!(bounded.len() < big.len());
        let (small, truncated) = bound_output("short", MAX_GIT_OUTPUT_BYTES);
        assert!(!truncated);
        assert_eq!(small, "short");
    }
}
