#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::time::{Duration, Instant};

use opencode_rk_tools::shell_tool::{ShellConfig, ShellTool};
use tempfile::TempDir;
use tokio::process::Command;

const FIXTURE: &str = "#!/bin/sh\nprintf '%s\\n' \"$$\" > \"$1\"\nsleep 1\nprintf '%s\\n' sentinel > \"$2\"\n";

struct FixtureGuard {
    _dir: TempDir,
    pid_path: PathBuf,
}

impl Drop for FixtureGuard {
    fn drop(&mut self) {
        // Fixture containment only: do not leave a leaked child after a failed assertion.
        let Ok(pid) = fs::read_to_string(&self.pid_path) else {
            return;
        };
        let Ok(pid) = pid.trim().parse::<u32>() else {
            return;
        };
        let _ = StdCommand::new("/bin/kill")
            .args(["-KILL", &pid.to_string()])
            .status();
    }
}

fn fixture() -> (FixtureGuard, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let script = dir.path().join("fixture.sh");
    let pid_path = dir.path().join("child.pid");
    let sentinel_path = dir.path().join("sentinel");
    fs::write(&script, FIXTURE).expect("write fixture");
    let mut permissions = fs::metadata(&script).expect("fixture metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&script, permissions).expect("fixture executable");
    (
        FixtureGuard {
            _dir: dir,
            pid_path,
        },
        script,
        sentinel_path,
    )
}

async fn read_pid(path: &Path) -> Option<u32> {
    let text = fs::read_to_string(path).ok()?;
    let text = text.trim();
    if text.is_empty() || text.len() > 20 || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

async fn is_alive(pid: u32) -> Result<bool, String> {
    let output = Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .output()
        .await
        .map_err(|error| format!("kill -0 failed to run: {error}"))?;
    Ok(output.status.success())
}

#[tokio::test]
async fn aborting_in_flight_execute_reaps_child_and_prevents_delayed_side_effect() {
    let (guard, script, sentinel_path) = fixture();
    let pid_path = guard.pid_path.clone();
    let config = ShellConfig {
        timeout_secs: 5,
        max_output_mb: 1,
        allowed_commands: vec![script.to_string_lossy().into_owned()],
    };
    let pid_arg = pid_path.to_string_lossy().into_owned();
    let sentinel_arg = sentinel_path.to_string_lossy().into_owned();
    let tool = ShellTool::new(
        script.to_string_lossy().into_owned(),
        vec![pid_arg, sentinel_arg],
    );
    let handle = tokio::spawn(tool.execute(config));

    let wait_deadline = Instant::now() + Duration::from_secs(2);
    let pid = loop {
        if let Some(pid) = read_pid(&pid_path).await {
            break pid;
        }
        assert!(Instant::now() < wait_deadline, "fixture did not publish a bounded numeric PID");
        tokio::time::sleep(Duration::from_millis(10)).await;
    };

    handle.abort();
    let _ = handle.await;

    let kill_deadline = Instant::now() + Duration::from_millis(500);
    loop {
        assert!(Instant::now() < kill_deadline, "child PID {pid} remained alive after cancellation");
        if !is_alive(pid).await.expect("kill -0 runner") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    tokio::time::timeout(Duration::from_secs(2), tokio::time::sleep(Duration::from_millis(1100)))
        .await
        .expect("bounded post-cancellation side-effect wait");
    assert!(
        !sentinel_path.exists(),
        "child PID {pid} created delayed sentinel after cancellation"
    );
}
