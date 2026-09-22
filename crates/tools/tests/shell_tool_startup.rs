#![cfg(unix)]

use std::future::Future;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Command as StdCommand;
use std::time::Duration;

use opencode_rk_tools::shell_tool::{ShellConfig, ShellError, ShellResult, ShellTool};
use tempfile::TempDir;
use tokio::process::Command;
use tokio::sync::mpsc;

const STARTUP_FIXTURE: &str = "#!/bin/bash\nprintf '%s\\n' \"$$\" > \"$1\"\nsleep 30 &\ndescendant=$!\nprintf '%s\\n' \"$descendant\" > \"$2\"\nread -r < \"$5\"\nprintf '%s\\n' child-wrapper-ready > \"$3\"\nwait \"$descendant\"\nprintf '%s\\n' sentinel > \"$4\"\n";
const OUTPUT_FIXTURE: &str = "#!/bin/sh\nprintf '%s\\n' normal-output\nprintf '%s\\n' normal-error >&2\n";

#[derive(Debug, PartialEq, Eq)]
enum StartupEvent {
    ChildWrapperReady { pid: u32 },
    Completed,
}

/// Temporary compile-compatible adapter for the pre-readiness public API.
///
/// `ShellTool::execute` exposes completion only. Sending `Completed` after the
/// call returns is deliberately not a startup implementation: the blocking
/// fixture proves that this adapter cannot report child execution readiness.
trait LegacyStartupAdapter {
    fn execute_with_startup(
        self,
        config: ShellConfig,
        startup: mpsc::Sender<StartupEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<ShellResult, ShellError>> + Send>>;
}

impl LegacyStartupAdapter for ShellTool {
    fn execute_with_startup(
        self,
        config: ShellConfig,
        startup: mpsc::Sender<StartupEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<ShellResult, ShellError>> + Send>> {
        Box::pin(async move {
            let result = self.execute(config).await;
            let _ = startup.send(StartupEvent::Completed).await;
            result
        })
    }
}

struct FixtureGuard {
    _dir: TempDir,
    parent_pid_path: PathBuf,
    descendant_pid_path: PathBuf,
}

impl Drop for FixtureGuard {
    fn drop(&mut self) {
        // Failure containment only: kill explicitly recorded disposable PIDs.
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

fn executable(dir: &TempDir, name: &str, contents: &str) -> PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, contents).expect("write disposable fixture");
    let mut permissions = fs::metadata(&path)
        .expect("fixture metadata")
        .permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&path, permissions).expect("make fixture executable");
    path
}

fn startup_fixture() -> (FixtureGuard, PathBuf, PathBuf, PathBuf, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let script = executable(&dir, "startup.sh", STARTUP_FIXTURE);
    let parent_pid_path = dir.path().join("parent.pid");
    let descendant_pid_path = dir.path().join("descendant.pid");
    let ready_path = dir.path().join("ready");
    let gate_path = dir.path().join("startup-gate");
    let status = StdCommand::new("/usr/bin/mkfifo")
        .arg(&gate_path)
        .status()
        .expect("create disposable startup FIFO");
    assert!(status.success(), "mkfifo failed: {status}");
    let guard = FixtureGuard {
        _dir: dir,
        parent_pid_path: parent_pid_path.clone(),
        descendant_pid_path: descendant_pid_path.clone(),
    };
    (
        guard,
        script,
        parent_pid_path,
        descendant_pid_path,
        ready_path,
        gate_path,
    )
}

fn config(command: &Path, timeout_secs: u64) -> ShellConfig {
    ShellConfig {
        timeout_secs,
        max_output_mb: 1,
        allowed_commands: vec![command.to_string_lossy().into_owned()],
    }
}

async fn wait_for_text(path: &Path, timeout: Duration) -> String {
    tokio::time::timeout(timeout, async {
        loop {
            if let Ok(text) = fs::read_to_string(path) {
                if !text.trim().is_empty() {
                    return text;
                }
            }
            // Poll pacing only. The timeout is the correctness bound.
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("fixture readiness deadline elapsed")
}

fn numeric_pid(text: &str) -> u32 {
    let trimmed = text.trim();
    assert!(!trimmed.is_empty() && trimmed.len() <= 20);
    assert!(trimmed.bytes().all(|byte| byte.is_ascii_digit()));
    trimmed.parse().expect("fixture PID")
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

async fn wait_dead(pid: u32, timeout: Duration) {
    tokio::time::timeout(timeout, async {
        loop {
            if !is_alive(pid).await {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("recorded fixture PID remained alive");
}

async fn release_gate(path: PathBuf) {
    tokio::time::timeout(
        Duration::from_secs(1),
        tokio::task::spawn_blocking(move || {
            let mut gate = fs::OpenOptions::new()
                .write(true)
                .open(path)
                .expect("open disposable startup FIFO");
            gate.write_all(b"release\n")
                .expect("release disposable startup FIFO");
        }),
    )
    .await
    .expect("startup FIFO release deadline elapsed")
    .expect("startup FIFO release task failed");
}

#[tokio::test]
async fn startup_notification_requires_child_wrapper_readiness() {
    let (guard, script, parent_path, descendant_path, ready_path, gate_path) = startup_fixture();
    let sentinel_path = guard._dir.path().join("sentinel");
    let args = vec![
        parent_path.to_string_lossy().into_owned(),
        descendant_path.to_string_lossy().into_owned(),
        ready_path.to_string_lossy().into_owned(),
        sentinel_path.to_string_lossy().into_owned(),
        gate_path.to_string_lossy().into_owned(),
    ];
    let (startup_tx, mut startup_rx) = mpsc::channel(1);
    let handle = tokio::spawn(
        ShellTool::new(script.to_string_lossy().into_owned(), args)
            .execute_with_startup(config(&script, 10), startup_tx),
    );

    let _ = wait_for_text(&parent_path, Duration::from_secs(2)).await;
    let _ = wait_for_text(&descendant_path, Duration::from_secs(2)).await;
    let before_ready = tokio::time::timeout(Duration::from_millis(250), startup_rx.recv()).await;
    assert!(
        before_ready.is_err(),
        "startup notification arrived before child wrapper readiness: {before_ready:?}"
    );

    release_gate(gate_path.clone()).await;

    let ready = wait_for_text(&ready_path, Duration::from_secs(2)).await;
    assert_eq!(ready.trim(), "child-wrapper-ready");
    let observed = tokio::time::timeout(Duration::from_millis(250), startup_rx.recv()).await;

    handle.abort();
    let _ = handle.await;

    match observed {
        Ok(Some(StartupEvent::ChildWrapperReady { pid })) => {
            assert_eq!(pid, numeric_pid(&fs::read_to_string(&parent_path).expect("parent PID")));
        }
        other => panic!(
            "child wrapper became ready, but no distinct startup notification arrived: {other:?}"
        ),
    }
    drop(guard);
}

#[tokio::test]
async fn abort_after_readiness_kills_parent_and_descendant_and_prevents_sentinel() {
    let (guard, script, parent_path, descendant_path, ready_path, gate_path) = startup_fixture();
    let sentinel_path = guard._dir.path().join("sentinel");
    let tool = ShellTool::new(
        script.to_string_lossy().into_owned(),
        vec![
            parent_path.to_string_lossy().into_owned(),
            descendant_path.to_string_lossy().into_owned(),
            ready_path.to_string_lossy().into_owned(),
            sentinel_path.to_string_lossy().into_owned(),
            gate_path.to_string_lossy().into_owned(),
        ],
    );
    let handle = tokio::spawn(tool.execute(config(&script, 10)));

    let _ = wait_for_text(&parent_path, Duration::from_secs(2)).await;
    let _ = wait_for_text(&descendant_path, Duration::from_secs(2)).await;
    release_gate(gate_path.clone()).await;
    let _ = wait_for_text(&ready_path, Duration::from_secs(2)).await;
    let parent_pid = numeric_pid(&fs::read_to_string(&parent_path).expect("parent PID"));
    let descendant_pid = numeric_pid(
        &fs::read_to_string(&descendant_path).expect("descendant PID"),
    );
    handle.abort();
    let _ = handle.await;

    wait_dead(parent_pid, Duration::from_millis(750)).await;
    wait_dead(descendant_pid, Duration::from_millis(750)).await;
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert!(!sentinel_path.exists(), "canceled fixture created delayed sentinel");
    drop(guard);
}

#[tokio::test]
async fn startup_instrumentation_is_absent_and_normal_output_is_faithful() {
    let dir = tempfile::tempdir().expect("tempdir");
    let script = executable(&dir, "output.sh", OUTPUT_FIXTURE);
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        ShellTool::new(script.to_string_lossy().into_owned(), Vec::new())
            .execute(config(&script, 2)),
    )
    .await
    .expect("output fixture deadline elapsed")
    .expect("output fixture should succeed");

    assert_eq!(result.stdout, "normal-output\n");
    assert_eq!(result.stderr, "normal-error\n");
    assert!(!result.stdout.contains("child-wrapper-ready"));
    assert!(!result.stderr.contains("child-wrapper-ready"));
}

#[tokio::test]
async fn startup_error_is_bounded_and_explicit() {
    let command = "/definitely-not-a-real-shell-tool-startup-command";
    let result = tokio::time::timeout(
        Duration::from_millis(500),
        ShellTool::new(command, Vec::new()).execute(ShellConfig {
            timeout_secs: 1,
            max_output_mb: 1,
            allowed_commands: vec![command.to_owned()],
        }),
    )
    .await
    .expect("startup error deadline elapsed");

    assert!(matches!(result, Err(ShellError::Spawn(_))), "result: {result:?}");
}

#[tokio::test]
async fn startup_channel_is_bounded() {
    let (tx, mut rx) = mpsc::channel::<StartupEvent>(1);
    tx.try_send(StartupEvent::Completed).expect("first bounded event");
    assert!(tx.try_send(StartupEvent::Completed).is_err());
    assert_eq!(rx.recv().await, Some(StartupEvent::Completed));
}
