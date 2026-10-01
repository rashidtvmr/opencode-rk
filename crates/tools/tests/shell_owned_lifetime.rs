//! Source-only contract for ownership of a live shell process.
//!
//! These tests deliberately use a fixed, authored script and direct argv.  They
//! are not a shell-parser, timeout, PTY, or process-tree contract.  In
//! particular, aborting the Tokio owner must not orphan the direct child.
#![cfg(unix)]

use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use opencode_rk_security::{
    PermissionBroker, PermissionRule, PermissionSet, RuleEffect, SecurityPolicy,
};
use opencode_rk_tools::shell_tool::{ShellConfig, ShellError, ShellTool};
use tempfile::TempDir;

const WAIT_LIMIT: Duration = Duration::from_secs(2);
const MAX_PROBE_BYTES: usize = 4096;

struct ProbeError(String);

impl std::fmt::Debug for ProbeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug)]
struct ProcessRow {
    pid: u32,
    ppid: u32,
    state: String,
}

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
            "#!/bin/sh\nprintf '%s\\n' \"$$\" > \"$1\"\nif [ \"$2\" = complete ]; then exit 0; fi\nexec /bin/sleep 30\n",
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

    fn tool(&self, mode: &'static str) -> ShellTool {
        ShellTool::new(
            "/bin/sh",
            vec![
                self.script.to_string_lossy().into_owned(),
                self.pid_file.to_string_lossy().into_owned(),
                mode.to_owned(),
            ],
        )
        .cwd(self.dir.path().to_string_lossy())
    }

    fn pid(&self) -> Result<Option<u32>, ProbeError> {
        let file = match fs::File::open(&self.pid_file) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(ProbeError(format!("PID marker open failed: {error}"))),
        };
        let mut bytes = Vec::with_capacity(65);
        file.take(65)
            .read_to_end(&mut bytes)
            .map_err(|error| ProbeError(format!("PID marker read failed: {error}")))?;
        if bytes.len() > 64 {
            return Err(ProbeError("fixture PID marker too large".to_owned()));
        }
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| ProbeError("fixture PID is not UTF-8".to_owned()))?
            .trim();
        if text.is_empty() || text.len() > 64 {
            return Err(ProbeError("fixture PID marker is invalid".to_owned()));
        }
        let pid = text
            .parse()
            .map_err(|_| ProbeError("fixture PID is not numeric".to_owned()))?;
        validate_pid(pid)?;
        Ok(Some(pid))
    }
}

impl Drop for OwnedFixture {
    fn drop(&mut self) {
        // This is failure-only cleanup.  It is intentionally conservative:
        // only a PID from our authored marker is considered, and ownership is
        // checked against the current test process before signalling it.
        let Ok(Some(pid)) = self.pid() else { return };
        let Ok(Some(row)) = process_row(pid) else {
            return;
        };
        if row.ppid != std::process::id() || !is_live_state(&row.state) {
            return;
        }
        let result = Command::new("/bin/kill")
            .env_clear()
            .args(["-KILL", &pid.to_string()])
            .status();
        eprintln!("shell lifetime fixture forced cleanup pid={pid}: {result:?}");
        let _ = wait_until_sync(|| matches!(process_row(pid), Ok(None)), WAIT_LIMIT);
    }
}

fn validate_pid(pid: u32) -> Result<(), ProbeError> {
    if pid <= 1 || pid == std::process::id() {
        return Err(ProbeError("unsafe or self PID".to_owned()));
    }
    Ok(())
}

fn is_live_state(state: &str) -> bool {
    !state.starts_with('Z') && !state.starts_with('z')
}

fn process_row(pid: u32) -> Result<Option<ProcessRow>, ProbeError> {
    validate_pid(pid)?;
    let output = Command::new("/bin/ps")
        .env_clear()
        .args([
            "-p",
            &pid.to_string(),
            "-o",
            "pid=",
            "-o",
            "ppid=",
            "-o",
            "state=",
        ])
        .output()
        .map_err(|error| ProbeError(format!("ps launch failed: {error}")))?;
    if output.stdout.len() > MAX_PROBE_BYTES || output.stderr.len() > MAX_PROBE_BYTES {
        return Err(ProbeError("ps output exceeded limit".to_owned()));
    }
    if !output.status.success() {
        if output.status.code() == Some(1) && output.stdout.is_empty() && output.stderr.is_empty() {
            return Ok(None);
        }
        return Err(ProbeError("ps returned unknown failure".to_owned()));
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|_| ProbeError("ps returned invalid UTF-8".to_owned()))?;
    if text.trim().is_empty() {
        return Err(ProbeError("ps returned empty successful output".to_owned()));
    }
    let fields: Vec<_> = text.split_whitespace().collect();
    if fields.len() != 3 {
        return Err(ProbeError("invalid process row".to_owned()));
    }
    let observed_pid = fields[0]
        .parse()
        .map_err(|_| ProbeError("invalid PID".to_owned()))?;
    let parent = fields[1]
        .parse()
        .map_err(|_| ProbeError("invalid PPID".to_owned()))?;
    validate_pid(observed_pid)?;
    if parent == 0 || fields[2].is_empty() {
        return Err(ProbeError("invalid ownership/state".to_owned()));
    }
    Ok(Some(ProcessRow {
        pid: observed_pid,
        ppid: parent,
        state: fields[2].to_owned(),
    }))
}

fn wait_until_sync(mut condition: impl FnMut() -> bool, limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    while !condition() {
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    true
}

async fn wait_until(mut condition: impl FnMut() -> bool, limit: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + limit;
    while !condition() {
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    true
}

async fn wait_for_started(fixture: &OwnedFixture) -> u32 {
    let deadline = tokio::time::Instant::now() + WAIT_LIMIT;
    loop {
        if let Some(pid) = fixture.pid().expect("authoritative PID marker read") {
            assert_ne!(
                pid,
                std::process::id(),
                "fixture must not claim the test process"
            );
            let row = process_row(pid)
                .expect("authoritative ps probe")
                .expect("live child row");
            assert_eq!(row.pid, pid);
            assert_eq!(row.ppid, std::process::id());
            assert!(
                is_live_state(&row.state),
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
    let result = fixture
        .tool("complete")
        .execute(fixture.config())
        .await
        .expect("normal command completes");
    assert!(result.success);
    let pid = fixture
        .pid()
        .expect("valid completion marker")
        .expect("completion announces PID");
    assert_ne!(pid, std::process::id());
    assert!(
        wait_until(|| matches!(process_row(pid), Ok(None)), WAIT_LIMIT).await,
        "completed PID remains present"
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
        .tool("sleep")
        .broker(broker)
        .execute(fixture.config())
        .await;
    assert!(matches!(result, Err(ShellError::Denied(_))));
    assert!(
        fixture.pid().expect("valid marker read").is_none(),
        "denied spawn must not write the PID marker"
    );
}

#[tokio::test]
async fn unpolled_execute_future_does_not_spawn_or_create_marker() {
    let fixture = OwnedFixture::new();
    let future = fixture.tool("sleep").execute(fixture.config());
    drop(future);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        fixture.pid().expect("valid marker read").is_none(),
        "an unpolled future must have no side effects"
    );
}

#[tokio::test]
async fn aborting_owned_task_reclaims_live_direct_child() {
    let fixture = OwnedFixture::new();
    let handle = tokio::spawn(fixture.tool("sleep").execute(fixture.config()));
    let pid = wait_for_started(&fixture).await;
    handle.abort();
    let join = handle.await;
    assert!(join.is_err_and(|error| error.is_cancelled()));
    assert!(
        wait_until(|| matches!(process_row(pid), Ok(None)), WAIT_LIMIT).await,
        "aborted owner left child PID {pid} alive"
    );
}

#[tokio::test]
async fn dropping_timeout_owner_reclaims_live_direct_child() {
    let fixture = OwnedFixture::new();
    let tool = fixture.tool("sleep");
    let config = fixture.config();
    let (start_timeout_tx, start_timeout_rx) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let mut future = Box::pin(tool.execute(config));
        tokio::select! {
            result = &mut future => return result,
            started = start_timeout_rx => started.expect("live child confirmed by test owner"),
        }
        tokio::time::timeout(Duration::from_millis(50), future)
            .await
            .map_err(|_| ShellError::Cancelled)
            .and_then(|result| result)
    });
    let pid = wait_for_started(&fixture).await;
    start_timeout_tx
        .send(())
        .expect("start timeout after live PID proof");
    let timeout_result = task.await.expect("timeout owner task must finish");
    assert!(
        matches!(timeout_result, Err(ShellError::Cancelled)),
        "owner must report the bounded timeout"
    );
    assert!(
        wait_until(|| matches!(process_row(pid), Ok(None)), WAIT_LIMIT).await,
        "timeout owner left child PID {pid} alive"
    );
}
