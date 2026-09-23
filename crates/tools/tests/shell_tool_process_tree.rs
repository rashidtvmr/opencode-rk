#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::time::{Duration, Instant};

use opencode_rk_tools::shell_tool::{ShellConfig, ShellTool};
use tempfile::TempDir;
use tokio::process::Command;

const FIXTURE: &str = "#!/bin/sh\nprintf '%s\\n' \"$$\" > \"$1\"\nsleep 5 &\ndescendant=$!\nprintf '%s\\n' \"$descendant\" > \"$2\"\nwait \"$descendant\"\nprintf '%s\\n' sentinel > \"$3\"\n";

struct FixtureGuard {
    _dir: TempDir,
    parent_pid_path: PathBuf,
    descendant_pid_path: PathBuf,
}

impl Drop for FixtureGuard {
    fn drop(&mut self) {
        // Failure containment only: kill both fixture processes explicitly.
        for path in [&self.parent_pid_path, &self.descendant_pid_path] {
            let Ok(text) = fs::read_to_string(path) else {
                continue;
            };
            let Ok(pid) = text.trim().parse::<u32>() else {
                continue;
            };
            let _ = StdCommand::new("/bin/kill")
                .args(["-KILL", &pid.to_string()])
                .status();
        }
    }
}

fn executable(dir: &TempDir) -> PathBuf {
    let path = dir.path().join("process-tree.sh");
    fs::write(&path, FIXTURE).expect("write fixture");
    let mut permissions = fs::metadata(&path).expect("fixture metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&path, permissions).expect("fixture executable");
    path
}

fn read_pid(path: &Path) -> Option<u32> {
    let text = fs::read_to_string(path).ok()?;
    let text = text.trim();
    if text.is_empty() || text.len() > 20 || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

async fn is_alive(pid: u32) -> bool {
    Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .output()
        .await
        .expect("kill -0 runner")
        .status
        .success()
}

#[tokio::test]
async fn aborting_execute_kills_process_tree_and_prevents_delayed_side_effect() {
    let dir = tempfile::tempdir().expect("tempdir");
    let script = executable(&dir);
    let parent_pid_path = dir.path().join("parent.pid");
    let descendant_pid_path = dir.path().join("descendant.pid");
    let sentinel_path = dir.path().join("sentinel");
    let guard = FixtureGuard {
        _dir: dir,
        parent_pid_path: parent_pid_path.clone(),
        descendant_pid_path: descendant_pid_path.clone(),
    };
    let config = ShellConfig {
        timeout_secs: 10,
        max_output_mb: 1,
        allowed_commands: vec![script.to_string_lossy().into_owned()],
    };
    let tool = ShellTool::new(
        script.to_string_lossy().into_owned(),
        vec![
            parent_pid_path.to_string_lossy().into_owned(),
            descendant_pid_path.to_string_lossy().into_owned(),
            sentinel_path.to_string_lossy().into_owned(),
        ],
    );
    let handle = tokio::spawn(tool.execute(config));

    let pid_deadline = Instant::now() + Duration::from_secs(2);
    let (parent_pid, descendant_pid) = loop {
        if let (Some(parent), Some(descendant)) =
            (read_pid(&parent_pid_path), read_pid(&descendant_pid_path))
        {
            break (parent, descendant);
        }
        assert!(
            Instant::now() < pid_deadline,
            "fixture did not publish both bounded numeric PIDs"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    };

    handle.abort();
    let _ = handle.await;

    let kill_deadline = Instant::now() + Duration::from_millis(750);
    loop {
        let parent_alive = is_alive(parent_pid).await;
        let descendant_alive = is_alive(descendant_pid).await;
        if !parent_alive && !descendant_alive {
            break;
        }
        assert!(
            Instant::now() < kill_deadline,
            "process-tree cancellation leaked parent PID {parent_pid} or descendant PID {descendant_pid}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    tokio::time::timeout(
        Duration::from_secs(1),
        tokio::time::sleep(Duration::from_millis(250)),
    )
    .await
    .expect("bounded post-cancellation side-effect wait");
    assert!(
        !sentinel_path.exists(),
        "descendant PID {descendant_pid} created delayed sentinel"
    );
    drop(guard);
}
