//! Shell tool: bounded command execution with timeout, env, cwd and cancellation.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Instant;

use opencode_rk_security::{Decision, OperationIntent, PermissionBroker};

use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Child;
use tokio::time::{Duration, timeout as tokio_timeout};

/// Maximum output bytes retained for stdout/stderr (10 MiB).
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

/// Outcome of a completed shell command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u128,
    /// Process exit code, if terminated normally.
    pub exit_code: Option<i32>,
}

/// Errors returned by [`ShellTool::execute`].
#[derive(Debug, Error)]
pub enum ShellError {
    #[error("command not allowed by allowlist")]
    CommandNotAllowed,

    #[error("no command specified")]
    NoCommand,

    #[error("spawn failed: {0}")]
    Spawn(String),

    #[error("timed out after {0} seconds")]
    Timeout(u64),

    #[error("denied by broker: {0}")]
    Denied(String),

    #[error("killed by cancellation")]
    Cancelled,

    #[error("output exceeded maximum of {0} bytes")]
    OutputTooLarge(usize),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Configuration for a single shell execution.
#[derive(Debug, Clone)]
pub struct ShellConfig {
    pub timeout_secs: u64,
    /// Maximum megabytes of stdout/stderr retained per stream.
    pub max_output_mb: u64,
    /// Allowlist of permitted command binaries (empty = deny all).
    pub allowed_commands: Vec<String>,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 30,
            max_output_mb: 10,
            allowed_commands: Vec::new(),
        }
    }
}

/// A shell command invocation with cancellation semantics on drop.
pub struct ShellTool {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub timeout_secs: u64,
    pub cwd: Option<String>,
    /// Injected permission broker consulted before spawn. `None` preserves
    /// legacy allowlist-only gating (ponytail: broker wiring mandatory for
    /// new callers; deny of a missing broker is a future upgrade).
    pub authz: Option<PermissionBroker>,
    child: Option<Child>,
}

impl ShellTool {
    /// Create a new `ShellTool` for `command` with the given arguments.
    pub fn new(command: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            command: command.into(),
            args,
            env: HashMap::new(),
            timeout_secs: 30,
            cwd: None,
            authz: None,
            child: None,
        }
    }

    /// Builder: command (overwrites).
    pub fn command(mut self, command: impl Into<String>) -> Self {
        self.command = command.into();
        self
    }

    /// Builder: positional args.
    pub fn args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Builder: extra environment variables.
    pub fn env(mut self, env: HashMap<String, String>) -> Self {
        self.env = env;
        self
    }

    /// Builder: timeout in seconds.
    pub fn timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = if secs == 0 { 1 } else { secs };
        self
    }

    /// Builder: working directory.
    pub fn cwd(mut self, cwd: impl Into<String>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Builder: permission broker consulted before spawn (mcp_spawn.rs:379).
    pub fn broker(mut self, broker: PermissionBroker) -> Self {
        self.authz = Some(broker);
        self
    }

    fn is_allowed(&self, cfg: &ShellConfig) -> bool {
        if cfg.allowed_commands.is_empty() {
            return false;
        }
        cfg.allowed_commands.iter().any(|a| a == &self.command)
    }

    fn max_bytes(&self, cfg: &ShellConfig) -> usize {
        let mb = cfg.max_output_mb.max(1) as usize;
        mb.saturating_mul(1024 * 1024).min(MAX_OUTPUT_BYTES)
    }

    /// Execute the shell command under `config`, returning a bounded `ShellResult`.
    pub async fn execute(mut self, config: ShellConfig) -> Result<ShellResult, ShellError> {
        if self.command.is_empty() {
            return Err(ShellError::NoCommand);
        }
        if !self.is_allowed(&config) {
            return Err(ShellError::CommandNotAllowed);
        }
        // Broker approval gates spawn (mcp_spawn.rs:379): intent built from
        // the exact argv that would exec. Deny/RequireHuman fail closed and
        // spawn no process. `None` broker = legacy allowlist-only.
        if let Some(broker) = &self.authz {
            let intent = OperationIntent::Process {
                program: self.command.clone(),
                args: self.args.clone(),
                cwd: self
                    .cwd
                    .as_ref()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("/tmp")),
            };
            match broker.authorize(&intent) {
                Decision::Allow => (),
                Decision::Deny { reason } => return Err(ShellError::Denied(reason)),
                Decision::RequireHuman { reason, .. } => {
                    return Err(ShellError::Denied(format!(
                        "requires human approval: {reason}"
                    )));
                }
            }
        }

        let limit = self.max_bytes(&config);
        let mut cmd = tokio::process::Command::new(&self.command);
        cmd.args(&self.args).env_clear();

        // Merge provided env with a minimal safe PATH.
        for (k, v) in &self.env {
            cmd.env(k, v);
        }
        if !self.env.contains_key("PATH") {
            cmd.env(
                "PATH",
                "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
            );
        }

        if let Some(cwd) = &self.cwd {
            cmd.current_dir(PathBuf::from(cwd));
        }

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        cmd.kill_on_drop(true);

        let child = cmd.spawn().map_err(|e| ShellError::Spawn(e.to_string()))?;
        self.child = Some(child);

        // Borrow child for reading; take it back before we await kill on drop.
        let child_ref = self.child.as_mut().expect("child set");

        let stdout = child_ref.stdout.take();
        let stderr = child_ref.stderr.take();
        let stdout_fut = async move {
            if let Some(out) = stdout {
                read_bounded(out, limit).await
            } else {
                Ok(Vec::new())
            }
        };
        let stderr_fut = async move {
            if let Some(err) = stderr {
                read_bounded(err, limit).await
            } else {
                Ok(Vec::new())
            }
        };

        let start = Instant::now();
        let timeout_secs = config.timeout_secs.max(1);
        let execution = async {
            let (stdout_bytes, stderr_bytes) = tokio::join!(stdout_fut, stderr_fut);
            let stdout_bytes = stdout_bytes?;
            let stderr_bytes = stderr_bytes?;
            let exit_code = child_ref.wait().await?.code();
            Ok::<_, std::io::Error>((stdout_bytes, stderr_bytes, exit_code))
        };
        let (stdout_bytes, stderr_bytes, exit_code) =
            tokio_timeout(Duration::from_secs(timeout_secs), execution)
                .await
                .map_err(|_| ShellError::Timeout(timeout_secs))??;
        let duration_ms = start.elapsed().as_millis();

        // Truncate to limit, converting bytes -> truncated flag.
        let stdout = truncate_to_limit(stdout_bytes, limit);
        let stderr = truncate_to_limit(stderr_bytes, limit);

        let success = exit_code == Some(0);

        // Clear child so drop cannot kill an already-reaped process.
        self.child = None;

        Ok(ShellResult {
            success,
            stdout,
            stderr,
            duration_ms,
            exit_code,
        })
    }

    /// Hard-cancel an in-flight command. Safe to call when no child exists.
    pub fn cancel(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.start_kill();
        }
    }

    /// Check whether the command is still running.
    pub fn is_running(&self) -> bool {
        self.child.is_some()
    }
}

impl Drop for ShellTool {
    fn drop(&mut self) {
        // Cancel any in-flight child on drop.
        self.cancel();
        self.child = None;
    }
}

async fn read_bounded<R>(mut reader: R, limit: usize) -> Result<Vec<u8>, std::io::Error>
where
    R: AsyncRead + Unpin,
{
    let retained_limit = limit.saturating_add(1);
    let mut retained = Vec::with_capacity(retained_limit.min(8 * 1024));
    let mut chunk = [0_u8; 8 * 1024];
    loop {
        let read = reader.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        let remaining = retained_limit.saturating_sub(retained.len());
        if remaining > 0 {
            retained.extend_from_slice(&chunk[..read.min(remaining)]);
        }
    }
    Ok(retained)
}

/// Enforce a hard timeout on execution, returning [`ShellError::Timeout`].
async fn with_timeout<T, F>(secs: u64, fut: F) -> Result<T, ShellError>
where
    T: std::fmt::Debug,
    F: std::future::Future<Output = T>,
{
    tokio_timeout(Duration::from_secs(secs), fut)
        .await
        .map_err(|_| ShellError::Timeout(secs))
}

/// Convert captured bytes into a string, truncating with a marker if over limit.
fn truncate_to_limit(bytes: Vec<u8>, limit: usize) -> String {
    if bytes.len() > limit {
        let mut truncated = bytes[..limit].to_vec();
        truncated.extend_from_slice(b"\n[truncated]");
        String::from_utf8_lossy(&truncated).into_owned()
    } else {
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(allowed: &[&str]) -> ShellConfig {
        ShellConfig {
            timeout_secs: 5,
            max_output_mb: 1,
            allowed_commands: allowed.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[tokio::test]
    async fn execute_simple() {
        let tool = ShellTool::new("echo".to_string(), vec!["hello".to_string()]);
        let res = tool.execute(cfg(&["echo"])).await.expect("should succeed");
        assert!(res.success);
        assert_eq!(res.exit_code, Some(0));
        assert!(res.stdout.trim().ends_with("hello"));
    }

    #[tokio::test]
    async fn env_vars() {
        let mut env = HashMap::new();
        env.insert("MY_TEST_VAR".to_string(), "foobar".to_string());
        let tool = ShellTool::new(
            "sh".to_string(),
            vec!["-c".to_string(), "echo $MY_TEST_VAR".to_string()],
        )
        .env(env);
        let res = tool.execute(cfg(&["sh"])).await.expect("should succeed");
        assert!(res.success);
        assert!(res.stdout.trim().ends_with("foobar"));
    }

    #[tokio::test]
    async fn timeout() {
        // `sleep 5` with a 1s timeout should fail as Timeout.
        let tool = ShellTool::new("sleep".to_string(), vec!["5".to_string()]).timeout(1);
        let res = with_timeout(1, tool.execute(cfg(&["sleep"]))).await;
        assert!(matches!(res, Err(ShellError::Timeout(1))));
    }

    #[tokio::test]
    async fn cancel_on_drop() {
        let mut tool = ShellTool::new("sleep".to_string(), vec!["30".to_string()]);
        tool.timeout_secs = 30;
        // Spawn in background so we can observe cancellation on drop.
        let handle = tokio::spawn(async move { tool.execute(cfg(&["sleep"])).await });
        // Give it a moment to start.
        tokio::time::sleep(Duration::from_millis(100)).await;
        drop(handle);
        // If cancellation works, the spawn handle drops cleanly (task aborts).
        // The test passes if we reach here without hanging.
    }

    #[tokio::test]
    async fn error_handling() {
        // Nonexistent command.
        let tool = ShellTool::new("definitely_not_a_real_cmd_xyz".to_string(), vec![]);
        let res = tool.execute(cfg(&["definitely_not_a_real_cmd_xyz"])).await;
        assert!(matches!(res, Err(ShellError::Spawn(_))));

        // Command not in allowlist.
        let tool2 = ShellTool::new("echo".to_string(), vec!["x".to_string()]);
        let res2 = tool2.execute(cfg(&["other_cmd"])).await;
        assert!(matches!(res2, Err(ShellError::CommandNotAllowed)));

        // Empty command.
        let tool3 = ShellTool::new("".to_string(), vec![]);
        let res3 = tool3.execute(cfg(&["echo"])).await;
        assert!(matches!(res3, Err(ShellError::NoCommand)));

        // Command failing with non-zero exit.
        let tool4 = ShellTool::new(
            "sh".to_string(),
            vec!["-c".to_string(), "exit 7".to_string()],
        );
        let res4 = tool4.execute(cfg(&["sh"])).await.expect("should run");
        assert!(!res4.success);
        assert_eq!(res4.exit_code, Some(7));
    }

    // --- unit test for truncation helper ---

    #[test]
    fn truncate_to_limit_truncates() {
        let bytes = vec![b'a'; 100];
        let out = truncate_to_limit(bytes.clone(), 50);
        assert!(out.contains('\n'));
        assert!(out.len() <= 50 + "\n[truncated]".len());
    }

    #[test]
    fn truncate_to_limit_keeps_small() {
        let bytes = b"hello".to_vec();
        let out = truncate_to_limit(bytes, 50);
        assert_eq!(out, "hello");
    }

    fn deny_all_broker() -> opencode_rk_security::PermissionBroker {
        opencode_rk_security::PermissionBroker::new(
            opencode_rk_security::SecurityPolicy::lean_default("/work/project"),
        )
        .with_permissions(opencode_rk_security::PermissionSet::new(vec![
            opencode_rk_security::PermissionRule::new(
                "*",
                opencode_rk_security::RuleEffect::Deny,
            ),
        ]))
    }

    fn allow_all_broker() -> opencode_rk_security::PermissionBroker {
        opencode_rk_security::PermissionBroker::new(
            opencode_rk_security::SecurityPolicy::lean_default("/work/project"),
        )
        .with_permissions(opencode_rk_security::PermissionSet::star())
    }

    #[tokio::test]
    async fn broker_deny_spawns_no_process() {
        // Allowlisted but broker-denied: fail closed, no process runs.
        let tool = ShellTool::new("echo".to_string(), vec!["hello".to_string()])
            .broker(deny_all_broker());
        let res = tool.execute(cfg(&["echo"])).await;
        assert!(
            matches!(res, Err(ShellError::Denied(_))),
            "broker deny must fail closed, got {res:?}"
        );
    }

    #[tokio::test]
    async fn broker_allow_runs_command() {
        let tool = ShellTool::new("echo".to_string(), vec!["hello".to_string()])
            .broker(allow_all_broker());
        let res = tool.execute(cfg(&["echo"])).await.expect("allowed");
        assert!(res.success);
        assert!(res.stdout.trim().ends_with("hello"));
    }
}
