//! DISC-104: shell/file authorization, timeout, cancel and byte budgets.
//!
//! Self-contained bounds lane for the AUD-005 repair items: the turn executor
//! path (`executor.rs::execute_shell`) awaits `bash -c` under a `timeout()`
//! that drops the future without killing the child, enforces no allowlist and
//! applies no output byte bound; `ShellTool::execute` reads both pipes to end
//! (unbounded) and never enforces its own `timeout_secs`; `file_ops::write`
//! auto-creates parents with no root gate or deny path.
//!
//! This module owns the corrected policy in one place so the integrator can
//! wire it into `executor.rs`/turn path/permission broker without changing
//! this file: authorize-before-spawn, timeout kill+reap, cooperative cancel,
//! streaming capped output with explicit markers, bounded resource reclaim,
//! and deny-no-side-effect file writes with a durable denial event.
//!
//! Sync + std-only by design: no runtime or extra dependencies, `run` blocks
//! the calling thread and joins its reader threads before returning.
//!
//! # Wiring (integrator, outside this file)
//! 1. add `pub mod shell_bounds;` to `crates/tools/src/lib.rs`
//! 2. call `ShellBounds::run(...)` from the executor/turn shell path and
//!    `gated_write_file(...)` from the file-write path with the broker verdict
//!    as `permitted`.
//!
//! Markers (stable, asserted by tests):
//! - `[truncated:over-byte-budget]` — output hit the byte cap.
//! - `[killed:timeout]` — child killed at timeout and reaped.
//! - `[cancelled]` — cancel flag observed, child killed and reaped.

use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Marker appended when output exceeds the byte budget.
pub const TRUNCATED_MARKER: &str = "[truncated:over-byte-budget]";
/// Marker appended when the child is killed at its timeout.
pub const TIMEOUT_MARKER: &str = "[killed:timeout]";
/// Marker appended when cooperative cancellation wins.
pub const CANCELLED_MARKER: &str = "[cancelled]";
/// Denial event kind written to the durable denial log.
pub const DENIAL_EVENT_KIND: &str = "shell.deny";

/// Outcome states a bounded run can end in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    /// Child exited on its own inside timeout/cancel/budget.
    Exited,
    /// Denied before spawn: no process was created.
    Denied,
    /// Killed at timeout and reaped.
    TimedOut,
    /// Cancel flag observed: child killed and reaped.
    Cancelled,
    /// Output hit the byte cap (process still reaped normally).
    Truncated,
}

/// Result of a bounded shell run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedRun {
    pub state: RunState,
    /// exit code when the child was reaped normally.
    pub exit_code: Option<i32>,
    /// Capped stdout (lossy UTF-8) plus any marker.
    pub stdout: String,
    /// Capped stderr (lossy UTF-8) plus any marker.
    pub stderr: String,
    pub timed_out: bool,
    pub cancelled: bool,
    pub truncated: bool,
    pub wall_ms: u64,
}

/// Configuration for one bounded run. All bounds required.
#[derive(Debug, Clone)]
pub struct ShellBounds {
    /// Exact allowlist of binaries (empty = deny all).
    pub allowed: Vec<String>,
    /// Hard wall-clock bound; child is killed + reaped past it.
    pub timeout: Duration,
    /// Max retained bytes per stream; excess is drained and discarded.
    pub max_bytes: usize,
    /// Cooperative cancel flag (caller flips to true).
    pub cancel: Arc<AtomicBool>,
}

impl ShellBounds {
    pub fn new(allowed: Vec<String>, timeout: Duration, max_bytes: usize) -> Self {
        Self {
            allowed,
            timeout,
            max_bytes: max_bytes.max(1),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn with_cancel(mut self, cancel: Arc<AtomicBool>) -> Self {
        self.cancel = cancel;
        self
    }

    /// Shared cancel flag the caller flips to request cancellation.
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancel)
    }

    fn is_allowed(&self, program: &str) -> bool {
        let base = program.rsplit('/').next().unwrap_or(program);
        self.allowed
            .iter()
            .any(|a| a == program || a == base || a.rsplit('/').next().unwrap_or(a) == base)
    }

    /// Run `program args` under allowlist + timeout + cancel + byte cap.
    ///
    /// Denial happens BEFORE spawn (no process) and appends a denial event to
    /// `denial_log` when set. Timeout/cancel kill AND reap the child (no leak).
    /// Output is capped during the read — the retained buffer never exceeds
    /// `max_bytes + marker len`, whatever the child emits.
    pub fn run(
        &self,
        program: &str,
        args: &[&str],
        denial_log: Option<&Path>,
    ) -> BoundedRun {
        let start = Instant::now();
        if program.is_empty() || !self.is_allowed(program) {
            append_denial_event(denial_log, program, args);
            return BoundedRun {
                state: RunState::Denied,
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                timed_out: false,
                cancelled: false,
                truncated: false,
                wall_ms: start.elapsed().as_millis().max(1) as u64,
            };
        }
        match spawn_capped(program, args, self.timeout, self.max_bytes, &self.cancel) {
            Spawned::Done(r) => r,
            Spawned::SpawnFailed => BoundedRun {
                state: RunState::Denied,
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                timed_out: false,
                cancelled: false,
                truncated: false,
                wall_ms: start.elapsed().as_millis().max(1) as u64,
            },
        }
    }
}

enum Spawned {
    Done(BoundedRun),
    SpawnFailed,
}

/// Poll interval for the supervisor loop.
const POLL: Duration = Duration::from_millis(5);

/// Kill a whole process group (`kill -<pgid>`). Best effort, never panics.
/// Negative-pid kill is the only std-only way to reach grandchildren the
/// child spawned (e.g. `sh -c "trap '' TERM; sleep 30"`).
///
fn kill_tree(child_id: u32) {
    // SIGKILL the process group first (grandchildren), then the child itself.
    // Each step is best-effort: the child may already be gone (Ok) or reaped.
    let pgid = format!("-{child_id}");
    let _ = Command::new("kill")
        .args(["-KILL", &pgid])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let pid = format!("{child_id}");
    let _ = Command::new("kill")
        .args(["-KILL", &pid])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Read exactly the retained prefix of `pipe` while draining the rest to /dev/null
/// accounting: `kept` never exceeds `cap`; returns (kept bytes, saw-overflow).
fn drain_capped<R: Read>(pipe: R, cap: usize, done: &Arc<AtomicBool>) -> (Vec<u8>, bool) {
    let mut kept = Vec::new();
    // Reserve only the cap so a huge child cannot balloon this buffer.
    kept.reserve(cap.min(64 * 1024));
    let mut truncated = false;
    let mut buf = [0u8; 8192];
    let mut pipe = pipe;
    loop {
        if done.load(Ordering::Relaxed) {
            // Stop retaining; caller kills the child, pipes hit EOF.
            // Drain what is already queued so no reader thread blocks exit.
            while let Ok(n) = pipe.read(&mut buf) {
                if n == 0 {
                    break;
                }
                if kept.len() < cap {
                    // Only top up while under cap — never grow past it.
                    let room = cap - kept.len();
                    kept.extend_from_slice(&buf[..n.min(room)]);
                    if n > room {
                        truncated = true;
                    }
                } else {
                    truncated = true;
                }
                if done.load(Ordering::Relaxed) {
                    // Keep draining only until EOF; bounded work per read.
                    continue;
                }
            }
            break;
        }
        match pipe.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if kept.len() < cap {
                    let room = cap - kept.len();
                    kept.extend_from_slice(&buf[..n.min(room)]);
                    if n > room {
                        truncated = true;
                    }
                } else {
                    truncated = true;
                }
            }
            Err(_) => break,
        }
    }
    (kept, truncated)
}

#[allow(clippy::too_many_lines)]
fn spawn_capped(
    program: &str,
    args: &[&str],
    timeout: Duration,
    cap: usize,
    cancel: &Arc<AtomicBool>,
) -> Spawned {
    let start = Instant::now();
    let mut child = match Command::new(program)
        .args(args)
        // New session (= new process group, pgid == child pid) so a timeout /
        // cancel kill can reach grandchildren via negative-pid kill. Without
        // this, `sh -c "sleep 30"` orphans `sleep` past a kill of `sh`.
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return Spawned::SpawnFailed,
    };
    let child_id = child.id();
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let done = Arc::new(AtomicBool::new(false));
    let done_out = Arc::clone(&done);
    let done_err = Arc::clone(&done);
    let out_h = std::thread::spawn(move || drain_capped(stdout, cap, &done_out));
    let err_h = std::thread::spawn(move || drain_capped(stderr, cap, &done_err));

    let deadline = start + timeout;
    let mut timed_out = false;
    let mut cancelled = false;
    let mut exit_code: Option<i32> = None;
    loop {
        if cancel.load(Ordering::Relaxed) {
            cancelled = true;
            break;
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                exit_code = status.code();
                break;
            }
            Ok(None) => {}
            Err(_) => break,
        }
        if Instant::now() >= deadline {
            timed_out = true;
            break;
        }
        std::thread::sleep(POLL);
    }
    if timed_out || cancelled {
        // Kill the whole tree AND reap: kill without wait leaves a zombie
        // (pid still in the process table); wait() reaps so no child leaks.
        // Group-kill first: covers grandchildren (e.g. `sleep` under `sh -c`)
        // that outlive a direct kill of the parent.
        kill_tree(child_id);
        let _ = child.kill();
        let _ = child.wait();
    } else {
        // Exited on its own; reap for the exit code.
        exit_code = child.wait().ok().and_then(|s| s.code());
    }
    // Sanity: after kill+wait the pid must be gone (no leaked child).
    debug_assert_not_running(child_id);
    done.store(true, Ordering::Relaxed);
    let (out_bytes, out_over) = out_h.join().unwrap_or_default();
    let (err_bytes, err_over) = err_h.join().unwrap_or_default();
    debug_assert!(
        out_bytes.len() <= cap && err_bytes.len() <= cap,
        "retained output must never exceed the byte budget"
    );
    let truncated = out_over || err_over;
    let wall_ms = start.elapsed().as_millis().max(1) as u64;

    let finish = |mut s: String, over: bool, marker: &str| {
        if over {
            if !s.is_empty() && !s.ends_with('\n') {
                s.push('\n');
            }
            s.push_str(marker);
        }
        s
    };
    if cancelled {
        return Spawned::Done(BoundedRun {
            state: RunState::Cancelled,
            exit_code: None,
            stdout: finish(String::from_utf8_lossy(&out_bytes).into_owned(), true, CANCELLED_MARKER),
            stderr: finish(String::from_utf8_lossy(&err_bytes).into_owned(), false, CANCELLED_MARKER),
            timed_out: false,
            cancelled: true,
            truncated,
            wall_ms,
        });
    }
    if timed_out {
        return Spawned::Done(BoundedRun {
            state: RunState::TimedOut,
            exit_code: None,
            stdout: finish(
                String::from_utf8_lossy(&out_bytes).into_owned(),
                true,
                TIMEOUT_MARKER,
            ),
            stderr: finish(String::from_utf8_lossy(&err_bytes).into_owned(), false, TIMEOUT_MARKER),
            timed_out: true,
            cancelled: false,
            truncated,
            wall_ms,
        });
    }
    let state = if truncated {
        RunState::Truncated
    } else {
        RunState::Exited
    };
    Spawned::Done(BoundedRun {
        state,
        exit_code,
        stdout: finish(String::from_utf8_lossy(&out_bytes).into_owned(), out_over, TRUNCATED_MARKER),
        stderr: finish(String::from_utf8_lossy(&err_bytes).into_owned(), err_over, TRUNCATED_MARKER),
        timed_out: false,
        cancelled: false,
        truncated,
        wall_ms,
    })
}

/// Best-effort check that `pid` no longer names a live/zombie child of this
/// process: `/proc/<pid>` must be absent or a zombie. Never panics.
fn debug_assert_not_running(pid: u32) {
    #[cfg(target_os = "linux")]
    {
        let stat = format!("/proc/{pid}/stat");
        if let Ok(text) = std::fs::read_to_string(&stat) {
            if let Some(state) = text.rsplit(')').next().and_then(|s| s.split_whitespace().nth(1)) {
                debug_assert!(
                    state.starts_with('Z'),
                    "child {pid} still running after kill+wait (state {state})"
                );
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
    }
}

/// Minimal JSON string escaper (stdlib only).
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Append one JSON-line denial event. Absent log path = event is dropped
/// (denial itself never fails). Never creates a process.
pub fn denial_event_json(program: &str, args: &[&str]) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let arg_list: Vec<String> = args.iter().map(|a| format!("\"{}\"", json_escape(a))).collect();
    format!(
        "{{\"kind\":\"{DENIAL_EVENT_KIND}\",\"program\":\"{}\",\"args\":[{}],\"denied\":true,\"spawned\":false,\"ts_ms\":{now}}}",
        json_escape(program),
        arg_list.join(",")
    )
}

fn append_denial_event(log: Option<&Path>, program: &str, args: &[&str]) {
    if let Some(path) = log {
        let line = denial_event_json(program, args);
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "{line}");
        }
    }
}

/// Outcome of a permission-gated file write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteOutcome {
    /// Write applied.
    Written { bytes: usize },
    /// Denied: NO filesystem mutation happened (no create, no truncate).
    Denied { reason: &'static str },
}

/// Write `content` to `path` only when `permitted` AND `path` stays inside
/// `root`. Denial performs zero filesystem I/O: the target is never created,
/// truncated, or had parents created — verified by the frozen tests via
/// byte-identical fixture comparison.
pub fn gated_write_file(
    root: &Path,
    path: &Path,
    content: &str,
    permitted: bool,
) -> WriteOutcome {
    if !permitted {
        return WriteOutcome::Denied {
            reason: "permission-denied",
        };
    }
    if !path_inside_root(root, path) {
        return WriteOutcome::Denied {
            reason: "path-escape",
        };
    }
    match std::fs::write(path, content) {
        Ok(()) => WriteOutcome::Written {
            bytes: content.len(),
        },
        Err(_) => WriteOutcome::Denied { reason: "io-error" },
    }
}

/// Lexical containment check: no I/O, no symlink resolution (caller resolves
/// symlinks before calling when strictness is required).
pub fn path_inside_root(root: &Path, path: &Path) -> bool {
    let mut root_norm = PathBuf::new();
    for comp in root.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                root_norm.pop();
            }
            c => root_norm.push(c.as_os_str()),
        }
    }
    let mut path_norm = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                path_norm.pop();
            }
            c => path_norm.push(c.as_os_str()),
        }
    }
    path_norm.starts_with(&root_norm)
}

/// UI-facing denial string for a denied shell/file action. Pure, bounded.
pub fn render_denial(action: &str, detail: &str) -> String {
    let action = action.chars().take(128).collect::<String>();
    let detail = detail.chars().take(256).collect::<String>();
    format!("denied: {action} — {detail} (no changes were made)")
}

#[cfg(test)]
mod tests {
    use super::*;


    fn bounds(allowed: &[&str], timeout_ms: u64, max_bytes: usize) -> ShellBounds {
        ShellBounds::new(
            allowed.iter().map(|s| s.to_string()).collect(),
            Duration::from_millis(timeout_ms),
            max_bytes,
        )
    }

    // DISC-104-T01: unauthorized command denied with no spawned process and a
    // durable denial event.
    #[test]
    fn disc104_t01_deny_no_spawn_plus_event() {
        let dir = std::env::temp_dir().join(format!("disc104-t01-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let marker = dir.join("must_not_exist.marker");
        let log = dir.join("denials.jsonl");
        let _ = std::fs::remove_file(&marker);
        let _ = std::fs::remove_file(&log);
        // `sh` is NOT allowlisted: denial must happen before spawn, so the
        // marker file can never be created as a side effect.
        let b = bounds(&["echo"], 5_000, 4096);
        let r = b.run("sh", &["-c", "touch \"$0\"", &marker.to_string_lossy()], Some(&log));
        assert_eq!(r.state, RunState::Denied, "unallowlisted command must deny");
        assert!(!r.timed_out && !r.cancelled);
        assert!(!marker.exists(), "denial must spawn no process (marker absent)");
        let log_text = std::fs::read_to_string(&log).expect("denial event must be durable");
        assert!(log_text.contains(DENIAL_EVENT_KIND), "durable denial event kind");
        assert!(log_text.contains("\"spawned\":false"), "event proves no spawn");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // DISC-104-T02: runaway command killed at timeout, bounded output + marker.
    #[test]
    fn disc104_t02_timeout_kills_and_marks() {
        let b = bounds(&["sh"], 300, 4096);
        let start = Instant::now();
        let r = b.run("sh", &["-c", "yes TRASH | head -c 10000000; sleep 30"], None);
        let wall = start.elapsed();
        assert_eq!(r.state, RunState::TimedOut);
        assert!(r.timed_out);
        assert!(r.exit_code.is_none(), "killed child has no normal exit");
        assert!(r.stdout.contains(TIMEOUT_MARKER), "explicit timeout marker");
        assert!(r.stdout.len() <= 4096 + TIMEOUT_MARKER.len() + 1, "bounded after kill");
        assert!(wall < Duration::from_secs(20), "must return promptly, took {wall:?}");
    }

    // DISC-104-T03: cancel reaches stable cancelled state with no leaked child.
    #[test]
    fn disc104_t03_cancel_stable_no_leak() {
        let cancel = Arc::new(AtomicBool::new(false));
        let b = bounds(&["sleep"], 30_000, 4096).with_cancel(Arc::clone(&cancel));
        let cancel2 = Arc::clone(&cancel);
        let handle = std::thread::spawn(move || b.run("sleep", &["30"], None));
        std::thread::sleep(Duration::from_millis(400));
        cancel2.store(true, Ordering::SeqCst);
        let r = handle.join().expect("run thread must join (no hang, no leak)");
        assert_eq!(r.state, RunState::Cancelled);
        assert!(r.cancelled && !r.timed_out);
        assert!(r.stdout.contains(CANCELLED_MARKER));
        assert!(r.exit_code.is_none());
        // Reclaim: a healthy run works right after the cancel on fresh bounds.
        let r2 = bounds(&["echo"], 5_000, 4096).run("echo", &["reclaimed"], None);
        assert_eq!(r2.state, RunState::Exited);
        assert!(r2.stdout.contains("reclaimed"));
    }

    // DISC-104-T04: output beyond budget truncates with marker, never unbounded.
    #[test]
    fn disc104_t04_over_budget_truncates_bounded() {
        let cap = 1024;
        let b = bounds(&["sh"], 10_000, cap);
        // Emits ~200 KiB — 200x the cap. Retained buffer must stay ~cap.
        let r = b.run("sh", &["-c", "yes PADLINE | head -c 200000"], None);
        assert!(r.truncated, "over-budget output must flag truncated");
        assert!(r.stdout.contains(TRUNCATED_MARKER), "explicit truncation marker");
        assert!(
            r.stdout.len() <= cap + TRUNCATED_MARKER.len() + 1,
            "retained stdout never grows unboundedly ({} > cap {cap})",
            r.stdout.len()
        );
    }

    // DISC-104-T05: denied file write leaves fixture byte-identical + UI denial.
    #[test]
    fn disc104_t05_denied_write_byte_identical_plus_ui() {
        let dir = std::env::temp_dir().join(format!("disc104-t05-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let fixture = dir.join("fixture.txt");
        let original = "original-bytes-12345\n";
        std::fs::write(&fixture, original).unwrap();
        let before = std::fs::read(&fixture).unwrap();
        let out = gated_write_file(&dir, &fixture, "ATTACKER-BYTES", false);
        assert_eq!(out, WriteOutcome::Denied { reason: "permission-denied" });
        let after = std::fs::read(&fixture).unwrap();
        assert_eq!(before, after, "denied write must leave file byte-identical");
        assert_eq!(after, original.as_bytes());
        // Escape attempt denied with zero side effects (no parent dirs made).
        let escape = dir.join("../disc104-t05-escape.marker");
        let out2 = gated_write_file(&dir, &escape, "x", true);
        assert_eq!(out2, WriteOutcome::Denied { reason: "path-escape" });
        assert!(!escape.exists());
        // UI renders the denial.
        let ui = render_denial("write fixture.txt", "permission-denied");
        assert!(ui.contains("denied") && ui.contains("no changes were made"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Resource reclaim: timeout kill frees the child; a follow-up run is healthy.
    #[test]
    fn disc104_t06_reclaim_after_timeout() {
        let b = bounds(&["sh", "echo"], 300, 4096);
        let r = b.run("sh", &["-c", "sleep 30"], None);
        assert_eq!(r.state, RunState::TimedOut);
        let r2 = bounds(&["echo"], 5_000, 4096).run("echo", &["healthy"], None);
        assert_eq!(r2.state, RunState::Exited);
        assert_eq!(r2.exit_code, Some(0));
        assert!(r2.stdout.contains("healthy"));
        // Allowed run stays under budget with no markers and real exit code.
        assert!(!r2.truncated && !r2.timed_out && !r2.cancelled);
        // Denial-event JSON shape is stable for the durable log consumer.
        let ev = denial_event_json("rm", &["-rf", "/"]);
        assert!(ev.contains(DENIAL_EVENT_KIND) && ev.contains("\"spawned\":false"));
    }

    // No-command / empty-allowlist edge: deny, never spawn.
    #[test]
    fn disc104_edge_empty_command_denies() {
        let b = bounds(&[], 5_000, 4096);
        let r = b.run("", &[], None);
        assert_eq!(r.state, RunState::Denied);
        let r2 = b.run("echo", &["hi"], None);
        assert_eq!(r2.state, RunState::Denied, "empty allowlist denies all");
    }

    // Allowed command exits non-zero: still Exited with the real code.
    #[test]
    fn disc104_edge_nonzero_exit() {
        let b = bounds(&["sh"], 5_000, 4096);
        let r = b.run("sh", &["-c", "exit 7"], None);
        assert_eq!(r.state, RunState::Exited);
        assert_eq!(r.exit_code, Some(7));
    }

    // stderr over budget truncates independently with the same marker.
    #[test]
    fn disc104_edge_stderr_truncates() {
        let cap = 512;
        let b = bounds(&["sh"], 10_000, cap);
        let r = b.run("sh", &["-c", "yes ERR | head -c 50000 >&2"], None);
        assert!(r.truncated);
        assert!(r.stderr.contains(TRUNCATED_MARKER));
        assert!(r.stderr.len() <= cap + TRUNCATED_MARKER.len() + 1);
    }

    // Readers drain: child that floods BOTH pipes past the cap stays bounded.
    #[test]
    fn disc104_edge_both_pipes_flood_bounded() {
        let cap = 1024;
        let b = bounds(&["sh"], 15_000, cap);
        let r = b.run(
            "sh",
            &["-c", "(yes OUT | head -c 100000 & yes ERR | head -c 100000 >&2) | head -c 200000"],
            None,
        );
        assert!(r.stdout.len() <= cap + TRUNCATED_MARKER.len() + 1);
        assert!(r.stderr.len() <= cap + TRUNCATED_MARKER.len() + 1);
    }
}
