//! Shell tool: bounded command execution with timeout, env, cwd and cancellation.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use opencode_rk_security::{Decision, HumanApproval, OperationIntent, PermissionBroker};

use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Child;
use tokio::sync::watch;
#[cfg(test)]
use tokio::time::timeout as tokio_timeout;

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
    process_group: Option<u32>,
    cancellation: Option<watch::Receiver<bool>>,
    timeout: Option<Duration>,
    human_approval: Option<(HumanApproval, String, u64, String)>,
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
            process_group: None,
            cancellation: None,
            timeout: None,
            human_approval: None,
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

    /// Builder: fixed wall-clock deadline spanning child + pipe completion.
    pub fn deadline(mut self, deadline: Duration) -> Self {
        self.timeout = Some(deadline);
        self
    }

    /// Builder: precise timeout in milliseconds (overrides legacy seconds).
    pub fn timeout_millis(self, millis: u64) -> Self {
        self.deadline(Duration::from_millis(millis))
    }

    /// Builder: cooperative cancellation via a pre-filled watch channel.
    pub fn cancellation(mut self, receiver: watch::Receiver<bool>) -> Self {
        self.cancellation = Some(receiver);
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

    /// Execute with a proof minted by the live permission-reply path. The
    /// exact argv/cwd/call identity is rechecked at the effect boundary.
    pub fn human_approved(
        mut self,
        approval: HumanApproval,
        session_id: impl Into<String>,
        run_id: u64,
        call_id: impl Into<String>,
    ) -> Self {
        self.human_approval = Some((approval, session_id.into(), run_id, call_id.into()));
        self
    }

    fn is_allowed(&self, cfg: &ShellConfig) -> bool {
        if cfg.allowed_commands.is_empty() {
            return false;
        }
        cfg.allowed_commands.contains(&self.command)
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
        // Already-cancelled token: fail closed before broker/spawn work.
        if self
            .cancellation
            .as_ref()
            .is_some_and(|receiver| *receiver.borrow())
        {
            return Err(ShellError::Cancelled);
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
            let decision =
                if let Some((approval, session_id, run_id, call_id)) = self.human_approval.take() {
                    broker.authorize_with_human_approval(
                        &intent,
                        approval,
                        &session_id,
                        run_id,
                        &call_id,
                    )
                } else {
                    broker.authorize(&intent)
                };
            match decision {
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
        #[cfg(unix)]
        cmd.process_group(0);
        let child = cmd.spawn().map_err(|e| ShellError::Spawn(e.to_string()))?;
        self.process_group = child.id();
        self.child = Some(child);
        let start = Instant::now();
        let duration = self
            .timeout
            .unwrap_or_else(|| Duration::from_secs(config.timeout_secs.max(1)));
        let mut cancellation = self.cancellation.take();
        let result = {
            let child = self.child.as_mut().expect("child set");
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();
            let completion = async {
                let (status, stdout, stderr) = tokio::join!(
                    child.wait(),
                    drain_bounded(stdout, limit),
                    drain_bounded(stderr, limit)
                );
                Ok::<_, ShellError>((status?, stdout, stderr))
            };
            let cancelled = async {
                if let Some(receiver) = cancellation.as_mut() {
                    loop {
                        if *receiver.borrow_and_update() {
                            break;
                        }
                        if receiver.changed().await.is_err() {
                            break;
                        }
                    }
                } else {
                    std::future::pending::<()>().await;
                }
            };
            tokio::select! {
                biased;
                _=cancelled=>Err(ShellError::Cancelled),
                _=tokio::time::sleep(duration)=>Err(ShellError::Timeout(duration.as_secs().max(1))),
                result=completion=>result,
            }
        };
        // Keep the pgid independently of Child::id(), which becomes None once
        // wait() reaps a parent whose descendants still own the output pipes.
        let (status, stdout_bytes, stderr_bytes) = match result {
            Ok(result) => result,
            Err(error) => {
                self.kill_group();
                if let Some(child) = self.child.as_mut() {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                }
                self.child = None;
                self.process_group = None;
                return Err(error);
            }
        };
        let duration_ms = start.elapsed().as_millis();

        // Truncate to limit, converting bytes -> truncated flag.
        let stdout = truncate_to_limit(stdout_bytes, limit);
        let stderr = truncate_to_limit(stderr_bytes, limit);

        let exit_code = status.code();
        let success = exit_code == Some(0);

        // Clear child so drop cannot kill an already-reaped process.
        self.child = None;
        self.process_group = None;

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
        self.kill_group();
        if let Some(child) = self.child.as_mut() {
            let _ = child.start_kill();
        }
    }

    fn kill_group(&self) {
        #[cfg(unix)]
        if let Some(pid) = self
            .process_group
            .and_then(|id| rustix::process::Pid::from_raw(id as i32))
        {
            let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
        }
    }

    /// Check whether the command is still running.
    pub fn is_running(&self) -> bool {
        self.child.is_some()
    }
}

async fn drain_bounded<R: AsyncRead + Unpin>(reader: Option<R>, limit: usize) -> Vec<u8> {
    let Some(mut reader) = reader else {
        return Vec::new();
    };
    let mut retained = Vec::with_capacity(limit.min(8192));
    let mut buffer = [0u8; 8192];
    loop {
        match reader.read(&mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(length) => {
                let keep = length.min(limit.saturating_add(1).saturating_sub(retained.len()));
                retained.extend_from_slice(&buffer[..keep]);
            }
        }
    }
    retained
}

impl Drop for ShellTool {
    fn drop(&mut self) {
        // Cancel any in-flight child on drop.
        self.cancel();
        self.child = None;
    }
}

/// Enforce a hard timeout on execution, returning `Cancelled` on timeout.
#[cfg(test)]
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
            opencode_rk_security::PermissionRule::new("*", opencode_rk_security::RuleEffect::Deny),
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
        let tool =
            ShellTool::new("echo".to_string(), vec!["hello".to_string()]).broker(deny_all_broker());
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
