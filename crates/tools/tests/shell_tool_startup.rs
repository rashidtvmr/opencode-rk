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

const STARTUP_FIXTURE: &str = "#!/bin/bash\nread -r < \"$3\"\ntmp=\"$1.tmp.$$\"\nprintf '%s\\n' \"$$\" > \"$tmp\"\nmv -f \"$tmp\" \"$1\"\nsleep 30 &\ndescendant=$!\nprintf '%s\\n' \"$descendant\" > \"$2\"\nwait \"$descendant\"\nprintf '%s\\n' sentinel > \"$4\"\n";
const OUTPUT_FIXTURE: &str = "#!/bin/sh\ntmp=\"$1.tmp.$$\"\nprintf '%s\\n' \"$$\" > \"$tmp\"\nmv -f \"$tmp\" \"$1\"\nprintf '%s\\n' normal-output\nprintf '%s\\n' normal-error >&2\n";

/// Compile-compatible fallback for the pre-readiness public API.
///
/// The fallback deliberately invokes only `execute`. It cannot publish a valid
/// startup PID before completion, so the readiness assertions remain semantic
/// RED until the inherent production method exists.
trait StartupAdapter {
    fn execute_with_startup(
        self,
        config: ShellConfig,
        readiness_path: PathBuf,
        startup: tokio::sync::mpsc::Sender<u32>,
    ) -> Pin<Box<dyn Future<Output = Result<ShellResult, ShellError>> + Send>>;
}

impl StartupAdapter for ShellTool {
    fn execute_with_startup(
        self,
        config: ShellConfig,
        readiness_path: PathBuf,
        startup: tokio::sync::mpsc::Sender<u32>,
    ) -> Pin<Box<dyn Future<Output = Result<ShellResult, ShellError>> + Send>> {
        Box::pin(async move {
            let _ = (readiness_path, startup);
            self.execute(config).await
        })
    }
}

struct FixtureGuard {
    _dir: TempDir,
    readiness_path: PathBuf,
    descendant_pid_path: PathBuf,
}

impl Drop for FixtureGuard {
    fn drop(&mut self) {
        // Failure containment only: kill explicitly recorded disposable PIDs.
        for path in [&self.readiness_path, &self.descendant_pid_path] {
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

fn startup_fixture() -> (FixtureGuard, PathBuf, PathBuf, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let script = executable(&dir, "startup.sh", STARTUP_FIXTURE);
    let readiness_path = dir.path().join("ready.pid");
    let descendant_pid_path = dir.path().join("descendant.pid");
    let gate_path = dir.path().join("startup-gate");
    let status = StdCommand::new("/usr/bin/mkfifo")
        .arg(&gate_path)
        .status()
        .expect("create disposable startup FIFO");
    assert!(status.success(), "mkfifo failed: {status}");
    let guard = FixtureGuard {
        _dir: dir,
        readiness_path: readiness_path.clone(),
        descendant_pid_path: descendant_pid_path.clone(),
    };
    (
        guard,
        script,
        readiness_path,
        descendant_pid_path,
        gate_path,
    )
    // The sentinel is derived from the guard directory by the caller.
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
    let pid = trimmed.parse().expect("fixture PID");
    assert!(pid > 0);
    pid
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
    let (guard, script, readiness_path, descendant_path, gate_path) = startup_fixture();
    let sentinel_path = guard._dir.path().join("sentinel");
    let args = vec![
        readiness_path.to_string_lossy().into_owned(),
        descendant_path.to_string_lossy().into_owned(),
        gate_path.to_string_lossy().into_owned(),
        sentinel_path.to_string_lossy().into_owned(),
    ];
    let (startup_tx, mut startup_rx) = tokio::sync::mpsc::channel::<u32>(1);
    let handle = tokio::spawn(
        ShellTool::new(script.to_string_lossy().into_owned(), args).execute_with_startup(
            config(&script, 10),
            readiness_path.clone(),
            startup_tx,
        ),
    );

    let before_ready =
        tokio::time::timeout(Duration::from_millis(250), startup_rx.recv()).await;
    assert!(
        before_ready.is_err() || !matches!(before_ready, Ok(Some(_))),
        "startup notification arrived before child wrapper readiness: {before_ready:?}"
    );

    release_gate(gate_path).await;
    let ready_pid = numeric_pid(&wait_for_text(&readiness_path, Duration::from_secs(2)).await);
    let descendant_pid = numeric_pid(&wait_for_text(&descendant_path, Duration::from_secs(2)).await);
    let observed = tokio::time::timeout(Duration::from_secs(1), startup_rx.recv()).await;
    let observed_pid = observed.ok().flatten();
    let duplicate = tokio::time::timeout(Duration::from_millis(250), startup_rx.recv()).await;

    // Abort after the readiness window. The final event assertion keeps this a
    // semantic RED when the fallback never publishes startup.
    handle.abort();
    let _ = handle.await;
    wait_dead(ready_pid, Duration::from_millis(750)).await;
    wait_dead(descendant_pid, Duration::from_millis(750)).await;
    tokio::time::sleep(Duration::from_millis(250)).await;

    assert_eq!(
        observed_pid,
        Some(ready_pid),
        "readiness must publish exactly the wrapper PID"
    );
    assert!(
        !matches!(duplicate, Ok(Some(_))),
        "startup publisher emitted a duplicate event: {duplicate:?}"
    );
    assert!(!sentinel_path.exists(), "canceled fixture created delayed sentinel");
    drop(guard);
}

#[tokio::test]
async fn startup_instrumentation_is_absent_and_normal_output_is_faithful() {
    let dir = tempfile::tempdir().expect("tempdir");
    let script = executable(&dir, "output.sh", OUTPUT_FIXTURE);
    let readiness_path = dir.path().join("ready.pid");
    let (startup_tx, mut startup_rx) = tokio::sync::mpsc::channel::<u32>(1);
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        ShellTool::new(
            script.to_string_lossy().into_owned(),
            vec![readiness_path.to_string_lossy().into_owned()],
        )
        .execute_with_startup(config(&script, 2), readiness_path.clone(), startup_tx),
    )
    .await
    .expect("output fixture deadline elapsed")
    .expect("output fixture should succeed");

    let ready_pid = numeric_pid(&wait_for_text(&readiness_path, Duration::from_secs(1)).await);
    let observed = tokio::time::timeout(Duration::from_secs(1), startup_rx.recv())
        .await
        .expect("startup event deadline elapsed")
        .expect("startup event missing");
    assert_eq!(observed, ready_pid);
    let duplicate = tokio::time::timeout(Duration::from_millis(100), startup_rx.recv()).await;
    assert!(!matches!(duplicate, Ok(Some(_))), "duplicate startup event: {duplicate:?}");
    assert_eq!(result.stdout, "normal-output\n");
    assert_eq!(result.stderr, "normal-error\n");
    assert!(!result.stdout.contains("child-wrapper-ready"));
    assert!(!result.stderr.contains("child-wrapper-ready"));
}

#[tokio::test]
async fn startup_error_is_bounded_and_explicit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let readiness_path = dir.path().join("never-created-ready.pid");
    let command = "/definitely-not-a-real-shell-tool-startup-command";
    let (startup_tx, mut startup_rx) = tokio::sync::mpsc::channel::<u32>(1);
    let result = tokio::time::timeout(
        Duration::from_millis(500),
        ShellTool::new(command, Vec::new()).execute_with_startup(
            ShellConfig {
                timeout_secs: 1,
                max_output_mb: 1,
                allowed_commands: vec![command.to_owned()],
            },
            readiness_path,
            startup_tx,
        ),
    )
    .await
    .expect("startup error deadline elapsed");

    assert!(matches!(result, Err(ShellError::Spawn(_))), "result: {result:?}");
    let no_event = tokio::time::timeout(Duration::from_millis(100), startup_rx.recv()).await;
    assert!(!matches!(no_event, Ok(Some(_))), "spawn failure emitted startup: {no_event:?}");
}

#[tokio::test]
async fn startup_sender_is_bounded() {
    let dir = tempfile::tempdir().expect("tempdir");
    let script = executable(&dir, "bounded.sh", OUTPUT_FIXTURE);
    let readiness_path = dir.path().join("ready.pid");
    let (startup_tx, _startup_rx) = tokio::sync::mpsc::channel::<u32>(1);
    assert_eq!(startup_tx.max_capacity(), 1);
    let result = ShellTool::new(
        script.to_string_lossy().into_owned(),
        vec![readiness_path.to_string_lossy().into_owned()],
    )
    .execute_with_startup(config(&script, 2), readiness_path, startup_tx)
    .await
    .expect("bounded fixture should succeed");
    assert!(result.success);
}
