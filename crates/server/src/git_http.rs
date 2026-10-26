//! Authenticated, read-only Git status and worktree inventory for one session.
//! All commands have fixed argv, no shell and short timeouts.
use crate::{parse_session_id, session_workdir, ApiFailure, AppState};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

const MAX_STDOUT: usize = 256 * 1024;
const MAX_WORKTREES: usize = 256;

#[derive(Default)]
struct WorktreeEntry {
    path: String,
    head: String,
    branch: String,
    locked: bool,
    prunable: bool,
}

fn parse_worktrees(raw: &str) -> Value {
    let mut trees = Vec::new();
    let mut current = WorktreeEntry::default();
    let mut present = false;
    for line in raw.lines().chain(std::iter::once("")) {
        if line.is_empty() {
            if present && trees.len() < MAX_WORKTREES {
                trees.push(json!({
                    "path":current.path, "head":current.head, "branch":current.branch,
                    "locked":current.locked, "prunable":current.prunable,
                }));
            }
            current = WorktreeEntry::default();
            present = false;
            continue;
        }
        if let Some(path) = line.strip_prefix("worktree ") {
            current.path = path.chars().take(4096).collect();
            present = true;
        } else if let Some(head) = line.strip_prefix("HEAD ") {
            current.head = head.chars().take(128).collect();
        } else if let Some(branch) = line.strip_prefix("branch ") {
            current.branch = branch
                .strip_prefix("refs/heads/")
                .unwrap_or(branch)
                .chars()
                .take(256)
                .collect();
        } else if line == "detached" {
            current.branch = "(detached)".to_owned();
        } else if line.starts_with("locked") {
            current.locked = true;
        } else if line.starts_with("prunable") {
            current.prunable = true;
        }
    }
    json!(trees)
}

async fn git(root: &PathBuf, args: &[&str]) -> Result<Option<String>, String> {
    let mut command = Command::new("git");
    command
        .args([
            "--no-optional-locks",
            "--literal-pathspecs",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.hooksPath=/dev/null",
            "-C",
        ])
        .arg(root)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|_| "Git is unavailable")?;
    let (status, output) = tokio::time::timeout(Duration::from_secs(3), async {
        let stdout = child.stdout.take().ok_or("Git output unavailable")?;
        // Bounded streaming read: never buffer arbitrarily large git output.
        // If the limit is exceeded, drop the child and kill it immediately.
        let mut limited = stdout.take((MAX_STDOUT + 1) as u64);
        let mut output = Vec::with_capacity(MAX_STDOUT.min(32 * 1024));
        limited
            .read_to_end(&mut output)
            .await
            .map_err(|_| "Git output unavailable")?;
        if output.len() > MAX_STDOUT {
            return Err("Git output exceeds safety limit");
        }
        let status = child.wait().await.map_err(|_| "Git process unavailable")?;
        Ok((status, output))
    })
    .await
    .map_err(|_| "Git read timed out")??;
    if !status.success() {
        return Ok(None);
    }
    String::from_utf8(output)
        .map(Some)
        .map_err(|_| "Git output is not text".into())
}

pub(crate) async fn get(
    State(state): State<AppState>,
    Path(raw_session): Path<String>,
) -> Result<Json<Value>, ApiFailure> {
    let id = parse_session_id(&raw_session)?;
    state.sessions.get(id).await.map_err(ApiFailure::internal)?;
    let root = session_workdir(&state.sessions, id, None).await?;
    let Some(status) = git(
        &root,
        &["status", "--short", "--branch", "--untracked-files=no"],
    )
    .await
    .map_err(ApiFailure::bad_request)?
    else {
        return Ok(Json(
            json!({"available":false,"root":root,"reason":"Current session is not in a Git repository","worktrees":[]}),
        ));
    };
    let worktrees = git(&root, &["worktree", "list", "--porcelain"])
        .await
        .map_err(ApiFailure::bad_request)?
        .map(|raw| parse_worktrees(&raw))
        .unwrap_or_else(|| json!([]));
    let status = status
        .lines()
        .take(200)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    Ok(Json(
        json!({"available":true,"root":root,"status":status,"worktrees":worktrees,"read_only":true}),
    ))
}

#[derive(Deserialize)]
pub(crate) struct DiffQuery {
    path: String,
}

/// Git-only diff preview; no arbitrary git arguments or shell execution.
pub(crate) async fn diff(
    State(state): State<AppState>,
    Path(raw_session): Path<String>,
    Query(query): Query<DiffQuery>,
) -> Result<Json<Value>, ApiFailure> {
    let session = parse_session_id(&raw_session)?;
    state
        .sessions
        .get(session)
        .await
        .map_err(ApiFailure::internal)?;
    if query.path == "." || query.path.is_empty() {
        return Err(ApiFailure::bad_request("choose an individual file"));
    }
    let parts = crate::workspace_files::safe_parts(&query.path).map_err(ApiFailure::bad_request)?;
    if parts.is_empty() {
        return Err(ApiFailure::bad_request("invalid Git file"));
    }
    let root = session_workdir(&state.sessions, session, None).await?;
    // Explicit pathspec boundary, with text conversion and external diff
    // disabled. Safe for an untrusted .gitattributes file in the repository.
    let output = git(
        &root,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "--",
            &query.path,
        ],
    )
    .await
    .map_err(ApiFailure::bad_request)?;
    let available = output.is_some();
    Ok(Json(
        json!({"path":query.path,"diff":output.unwrap_or_default(),
        "read_only":true,"available":available}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn porcelain_parser_handles_branch_detached_and_locks() {
        let raw = "worktree /tmp/main\nHEAD 012345\nbranch refs/heads/main-v2\n\nworktree /tmp/other\nHEAD 987654\ndetached\nlocked blocked\n";
        let trees = parse_worktrees(raw);
        assert_eq!(trees[0]["branch"], "main-v2");
        assert_eq!(trees[1]["branch"], "(detached)");
        assert_eq!(trees[1]["locked"], true);
    }
    #[test]
    fn empty_inventory_is_empty() {
        assert_eq!(parse_worktrees("").as_array().unwrap().len(), 0);
    }
    #[test]
    fn git_diff_paths_use_same_strict_session_file_policy() {
        for path in [
            "../outside",
            "/etc/passwd",
            ".env",
            "src/.env.local",
            "src/secrets/api.key",
            "src/../../other",
            "src\\relative",
        ] {
            assert!(crate::workspace_files::safe_parts(path).is_err(), "{path}");
        }
        assert!(crate::workspace_files::safe_parts("src/App.tsx").is_ok());
    }
}
