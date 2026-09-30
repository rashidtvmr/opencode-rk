//! Source-only contract for ownership of a live shell process.
//!
//! These tests deliberately use a fixed, authored script and direct argv.  They
//! are not a shell-parser, timeout, PTY, or process-tree contract.  In
//! particular, aborting the Tokio owner must not orphan the direct child.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use opencode_rk_security::{
    PermissionBroker, PermissionRule, PermissionSet, RuleEffect, SecurityPolicy,
};
use opencode_rk_tools::shell_tool::{ShellConfig, ShellError, ShellTool};
use tempfile::TempDir;

const WAIT_LIMIT: Duration = Duration::from_secs(2);

struct OwnedFixture {
    dir: TempDir,
    script: PathBuf,
    pid_file: PathBuf,
}

impl OwnedFixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("disposable fixture directory");
        let script = dir.path().join("owned-child.sh");
        let pid_file = dir.path().join("owned-child.pid");
        // Fixed test-authored content: the command under test receives this
        // path as argv and never receives an interpolated shell command.
        fs::write(
            &script,
            "#!/bin/sh\nprintf '%s\\n' \"$$\" > \"$1\"\nexec /bin/sleep 30\n",
        )
        .expect("write authored fixture");
        Self {
            dir,
            script,
            pid_file,
        }
    }

    fn config(&self) -> ShellConfig {
        ShellConfig {
            timeout_secs: 30,
            max_output_mb: 1,
            allowed_commands: vec!["/bin/sh".to_owned()],
        }
    }

    fn tool(&self) -> ShellTool {
        ShellTool::new(
            "/bin/sh",
            vec![
                self.script.to_string_lossy().into_owned(),
                self.pid_file.to_string_lossy().into_owned(),
            ],
        )
        .cwd(self.dir.path().to_string_lossy())
    }

    fn pid(&self) -> Option<u32> {
        let bytes = fs::read(&self.pid_file).ok()?;
        let text = std::str::from_utf8(&bytes).ok()?.trim();
        if text.is_empty() || text.len() > 64 {
            return None;
        }
        text.parse().ok()
    }
}

impl Drop for OwnedFixture {
    fn drop(&mut self) {
        // This is failure-only cleanup.  It is intentionally conservative:
        // only a PID from our authored marker is considered, and ownership is
        // checked against the current test process before signalling it.
        let Some(pid) = self.pid() else { return };
        if !is_owned_child(pid) {
            return;
        }
        let _ = Command::new("/bin/kill")
            .args(["-KILL", &pid.to_string()])
            .status();
        let _ = wait_until(|| !is_alive(pid), WAIT_LIMIT);
    }
}

fn is_alive(pid: u32) -> bool {
    process_row(pid).is_some()
}

fn is_owned_child(pid: u32) -> bool {
    process_row(pid)
        .is_some_and(|(observed_pid, parent)| observed_pid == pid && parent == std::process::id())
}

fn process_row(pid: u32) -> Option<(u32, u32)> {
    let output = Command::new("/bin/ps")
        .args(["-p", &pid.to_string(), "-o", "pid=", "-o", "ppid="])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let fields: Vec<_> = text.split_whitespace().collect();
    if fields.len() != 2 {
        return None;
    }
    Some((fields[0].parse().ok()?, fields[1].parse().ok()?))
}

fn wait_until(mut condition: impl FnMut() -> bool, limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    while !condition() {
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    true
}

async fn wait_for_started(fixture: &OwnedFixture) -> u32 {
    let deadline = tokio::time::Instant::now() + WAIT_LIMIT;
    loop {
        if let Some(pid) = fixture.pid() {
            assert_ne!(
                pid,
                std::process::id(),
                "fixture must not claim the test process"
            );
            assert!(
                is_owned_child(pid),
                "marker PID must identify our live direct child"
            );
            return pid;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "fixture child did not become live"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn normal_completion_has_no_live_process_or_marker_requirement() {
    let fixture = OwnedFixture::new();
    let tool = ShellTool::new("/bin/true", vec![]);
    let config = ShellConfig {
        allowed_commands: vec!["/bin/true".to_owned()],
        ..fixture.config()
    };
    let result = tool
        .execute(config)
        .await
        .expect("normal command completes");
    assert!(result.success);
    assert!(
        fixture.pid().is_none(),
        "unrelated normal command cannot create fixture marker"
    );
}

#[tokio::test]
async fn broker_denial_has_no_process_or_marker_side_effect() {
    let fixture = OwnedFixture::new();
    let broker =
        PermissionBroker::new(SecurityPolicy::lean_default(fixture.dir.path())).with_permissions(
            PermissionSet::new(vec![PermissionRule::new("*", RuleEffect::Deny)]),
        );
    let result = fixture
        .tool()
        .broker(broker)
        .execute(fixture.config())
        .await;
    assert!(matches!(result, Err(ShellError::Denied(_))));
    assert!(
        fixture.pid().is_none(),
        "denied spawn must not write the PID marker"
    );
}

#[tokio::test]
async fn unpolled_execute_future_does_not_spawn_or_create_marker() {
    let fixture = OwnedFixture::new();
    let future = fixture.tool().execute(fixture.config());
    drop(future);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        fixture.pid().is_none(),
        "an unpolled future must have no side effects"
    );
}

#[tokio::test]
async fn aborting_owned_task_reclaims_live_direct_child() {
    let fixture = OwnedFixture::new();
    let handle = tokio::spawn(fixture.tool().execute(fixture.config()));
    let pid = wait_for_started(&fixture).await;
    handle.abort();
    let join = handle.await;
    assert!(join.is_err_and(|error| error.is_cancelled()));
    assert!(
        wait_until(|| !is_alive(pid), WAIT_LIMIT),
        "aborted owner left child PID {pid} alive"
    );
}

#[tokio::test]
async fn dropping_timeout_owner_reclaims_live_direct_child() {
    let fixture = OwnedFixture::new();
    let handle = tokio::spawn(fixture.tool().execute(fixture.config()));
    let pid = wait_for_started(&fixture).await;
    let timed = tokio::time::timeout(Duration::from_millis(50), handle);
    let timeout_result = timed.await;
    assert!(
        timeout_result.is_err(),
        "fixture must still be running at timeout"
    );
    // timeout drops its JoinHandle, which is detach semantics, not ownership;
    // this test intentionally freezes the stronger contract for the product
    // implementation: its execution owner must still reclaim the child.
    assert!(
        wait_until(|| !is_alive(pid), WAIT_LIMIT),
        "timeout owner left child PID {pid} alive"
    );
}
