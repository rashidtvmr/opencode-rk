#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::time::{Duration, Instant};

use opencode_rk_tools::shell_tool::{ShellConfig, ShellError, ShellTool};
use tempfile::TempDir;
use tokio::process::Command;

const TIMEOUT_FIXTURE: &str = "#!/bin/sh\nprintf '%s\\n' \"$$\" > \"$1\"\n\"$3\" \"$4\" &\nhelper=$!\nwait \"$helper\"\nprintf '%s\\n' sentinel > \"$2\"\n";
const TIMEOUT_HELPER: &str = "#!/bin/sh\nprintf '%s\\n' \"$$\" > \"$1\"\nsleep 3\nprintf '%s\\n' complete > \"$2\"\n";
const OUTPUT_FIXTURE: &str = "#!/bin/sh\ndd if=/dev/zero bs=1048576 count=2 2>/dev/null\ndd if=/dev/zero bs=1048576 count=2 1>&2 2>/dev/null\n";
const LIMIT: usize = 1024 * 1024;
const MARKER: &str = "\n[truncated]";

struct FixtureGuard {
    dir: TempDir,
    pid_path: PathBuf,
    helper_pid_path: PathBuf,
}

impl Drop for FixtureGuard {
    fn drop(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(4);
        while fixture_process_alive(&self.pid_path, self.dir.path())
            || fixture_process_alive(&self.helper_pid_path, self.dir.path())
        {
            if Instant::now() >= deadline {
                eprintln!("timeout fixture cleanup did not finish; bounded fixture process may remain");
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

fn read_pid_sync(path: &Path) -> Option<u32> {
    let text = fs::read_to_string(path).ok()?;
    let text = text.trim();
    if text.is_empty() || text.len() > 20 || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

fn fixture_process_alive(pid_path: &Path, dir: &Path) -> bool {
    read_pid_sync(pid_path).is_some_and(|pid| {
        let Ok(output) = StdCommand::new("/bin/ps")
            .env_clear()
            .args(["-p", &pid.to_string(), "-o", "state=", "-o", "command="])
            .output()
        else {
            return true;
        };
        if output.stdout.len() > 4096 {
            return true;
        }
        if !output.status.success() && output.stdout.is_empty() {
            return false;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let mut fields = text.split_whitespace();
        let Some(state) = fields.next() else {
            return true;
        };
        !state.to_ascii_lowercase().starts_with('z')
            && fields.collect::<Vec<_>>().join(" ").contains(&dir.to_string_lossy().to_string())
    })
}

fn executable(dir: &TempDir, name: &str, contents: &str) -> PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, contents).expect("write fixture");
    let mut permissions = fs::metadata(&path).expect("fixture metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&path, permissions).expect("fixture executable");
    path
}

async fn read_pid(path: &Path) -> Option<u32> {
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
async fn public_execute_enforces_timeout_and_cleans_up_child() {
    let dir = tempfile::tempdir().expect("tempdir");
    let script = executable(&dir, "timeout.sh", TIMEOUT_FIXTURE);
    let helper = executable(&dir, "timeout-helper.sh", TIMEOUT_HELPER);
    let pid_path = dir.path().join("child.pid");
    let helper_pid_path = dir.path().join("helper.pid");
    let sentinel_path = dir.path().join("sentinel");
    let guard = FixtureGuard {
        dir,
        pid_path: pid_path.clone(),
        helper_pid_path: helper_pid_path.clone(),
    };
    let pid_arg = pid_path.to_string_lossy().into_owned();
    let sentinel_arg = sentinel_path.to_string_lossy().into_owned();
    let helper_path_arg = helper_pid_path.to_string_lossy().into_owned();
    let helper_arg = helper.to_string_lossy().into_owned();
    let config = ShellConfig {
        timeout_secs: 1,
        max_output_mb: 1,
        allowed_commands: vec![script.to_string_lossy().into_owned()],
    };
    let tool = ShellTool::new(
        script.to_string_lossy().into_owned(),
        vec![pid_arg, sentinel_arg, helper_path_arg, helper_arg],
    )
    .timeout(1);

    let started = Instant::now();
    let result = tokio::time::timeout(Duration::from_secs(2), tool.execute(config))
        .await
        .expect("public execute watchdog elapsed");
    assert!(started.elapsed() < Duration::from_millis(1500));
    assert!(matches!(result, Err(ShellError::Timeout(1))), "result: {result:?}");

    let pid_deadline = Instant::now() + Duration::from_millis(500);
    let pid = loop {
        if let Some(pid) = read_pid(&pid_path).await {
            if !is_alive(pid).await {
                break pid;
            }
        }
        assert!(Instant::now() < pid_deadline, "child was not reaped");
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    tokio::time::sleep(Duration::from_millis(2200)).await;
    assert!(!sentinel_path.exists(), "child PID {pid} created delayed sentinel");
    drop(guard);
}

#[tokio::test]
async fn execute_bounds_stdout_and_stderr_results() {
    let dir = tempfile::tempdir().expect("tempdir");
    let script = executable(&dir, "output.sh", OUTPUT_FIXTURE);
    let config = ShellConfig {
        timeout_secs: 2,
        max_output_mb: 1,
        allowed_commands: vec![script.to_string_lossy().into_owned()],
    };
    let tool = ShellTool::new(script.to_string_lossy().into_owned(), Vec::new());
    let result = tokio::time::timeout(Duration::from_secs(2), tool.execute(config))
        .await
        .expect("bounded output watchdog elapsed")
        .expect("output fixture should succeed");

    assert!(result.success);
    assert!(result.stdout.len() <= LIMIT + MARKER.len());
    assert!(result.stderr.len() <= LIMIT + MARKER.len());
    assert!(result.stdout.contains(MARKER));
    assert!(result.stderr.contains(MARKER));
}
