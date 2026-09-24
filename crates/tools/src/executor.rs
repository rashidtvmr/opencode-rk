//! Tool execution engine.
//!
//! Provides `ToolExecutor` for running tool calls with timeout support,
//! and `ToolCall`/`ToolResult` types for structured tool invocation.

use opencode_rk_security::app_policy::{ExpectedScope, Grant};
use opencode_rk_security::tool_authorize::{redact_secrets, ToolAuthorizer, ToolGate};
use opencode_rk_security::{Decision, OperationIntent, PermissionBroker};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::Instant as TokioInstant;
use tokio::time::timeout;

#[cfg(unix)]
use rustix::process::{kill_process_group, Pid, Signal};

// ---------------------------------------------------------------------------
// Authorized direct-process seam (TOOL-SHELL-CANCEL-CONTRACT accepted at
// c630f3f). These names and shapes are the normative public contract consumed
// by the later cancellation RED and its independent verification lane.
// ---------------------------------------------------------------------------

/// Hard upper bound for every process timeout phase.
const MAX_PROCESS_TIMEOUT: Duration = Duration::from_secs(300);
/// Hard upper bound for retained stdout/stderr, per stream.
const MAX_PROCESS_OUTPUT_BYTES: usize = 10 * 1024 * 1024;
/// Hard upper bounds for argv/env/cwd budgets.
const MAX_PROCESS_ARGV_BYTES: usize = 64 * 1024;
const MAX_PROCESS_ARGV_ELEMENTS: usize = 4096;
const MAX_PROCESS_ENV_BYTES: usize = 64 * 1024;
const MAX_PROCESS_ENV_ELEMENTS: usize = 1024;
const MAX_PROCESS_CWD_BYTES: usize = 4 * 1024;
/// Any redacted diagnostic reason is bounded to this many bytes.
const MAX_PROCESS_ERROR_BYTES: usize = 512;
/// Fixed marker appended exactly once when a stream exceeds its retained cap.
/// The marker is outside the retained-byte cap.
const PROCESS_TRUNCATED_MARKER: &str = "\n[truncated]";
/// Fixed minimal PATH used when the request does not explicitly supply one.
const PROCESS_MINIMAL_PATH: &str =
    "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";

/// A direct-argv process request. No shell parsing, interpolation, or `-c`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessRequest {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
}

/// Required, fully bounded limits for one authorized process request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessLimits {
    pub timeout: Duration,
    pub startup_timeout: Duration,
    pub cleanup_timeout: Duration,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
    pub max_argv_bytes: usize,
    pub max_argv_elements: usize,
    pub max_env_bytes: usize,
    pub max_env_elements: usize,
    pub max_cwd_bytes: usize,
}

/// Exactly one readiness event published after a successful spawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessReady {
    pub pid: u32,
    #[cfg(unix)]
    pub process_group: u32,
}

/// Terminal reason for a bounded request. Monotonic once latched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessTerminal {
    Exited { exit_code: Option<i32> },
    TimedOut,
    Cancelled,
    CancelledBeforeStart,
}

/// Result of one bounded cleanup operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CleanupStep {
    NotRequired,
    Succeeded,
    Failed,
}

/// Bounded cleanup outcome for one request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CleanupStatus {
    NotRequired,
    Reaped {
        group_kill: CleanupStep,
        child_kill: CleanupStep,
        wait: CleanupStep,
        readers: CleanupStep,
    },
    Failed {
        group_kill: CleanupStep,
        child_kill: CleanupStep,
        wait: CleanupStep,
        readers: CleanupStep,
    },
}

/// Typed, redacted failure for the authorized process seam.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProcessError {
    InvalidRequest { field: &'static str, reason: String },
    InvalidCwd { reason: String },
    InvalidLimits { reason: String },
    Denied { reason: String },
    HumanRequired { reason: String },
    InvalidGrant { reason: String },
    Spawn { reason: String },
    ReadinessReceiverClosed { pid: u32 },
    Io { operation: &'static str, reason: String },
    CleanupFailed { terminal: ProcessTerminal, status: CleanupStatus },
    UnsupportedPlatform { operation: &'static str },
}

/// Bounded successful result for an authorized process request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessResult {
    pub terminal: ProcessTerminal,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub cleanup: CleanupStatus,
    pub duration_ms: u64,
}

/// Caller-owned cancellation receiver; the only cancellation state read.
pub type ProcessCancellation = tokio::sync::watch::Receiver<bool>;

/// Retained prefix of one output stream plus an overflow flag.
#[cfg(unix)]
#[derive(Default)]
struct ReaderOutcome {
    bytes: Vec<u8>,
    truncated: bool,
}

/// Owned reader tasks for one request's stdout/stderr pipes.
#[cfg(unix)]
#[derive(Default)]
struct Readers {
    stdout: Option<JoinHandle<std::io::Result<ReaderOutcome>>>,
    stderr: Option<JoinHandle<std::io::Result<ReaderOutcome>>>,
}

/// Minimum bounded wait used when a phase has no measurable budget left, so a
/// zero-length timeout can never spuriously fail an already-complete step.
#[cfg(unix)]
const MIN_PROCESS_TICK: Duration = Duration::from_millis(1);

/// Redact secret-bearing substrings and cap the reason to the error byte bound.
fn bounded_process_reason(text: &str) -> String {
    let redacted = redact_secrets(text);
    if redacted.len() <= MAX_PROCESS_ERROR_BYTES {
        return redacted;
    }
    let mut end = MAX_PROCESS_ERROR_BYTES;
    while end > 0 && !redacted.is_char_boundary(end) {
        end -= 1;
    }
    redacted[..end].to_owned()
}

/// `u64` millisecond duration from one request-entry clock reading. Saturates
/// at `u64::MAX`; a sub-millisecond elapsed value is `0`.
fn process_duration_ms(start: TokioInstant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Start of one request, on the Tokio clock so paused-time tests advance it.
fn process_now() -> TokioInstant {
    TokioInstant::now()
}

/// Exact raw OS bytes of an `OsStr` for cwd budget/NUL accounting.
///
/// Unix counts the bytes actually handed to the kernel
/// (`std::os::unix::ffi::OsStrExt::as_bytes`), so a non-UTF-8 path is never
/// inflated by lossy U+FFFD expansion. Non-Unix only has to compile: the
/// process seam returns `UnsupportedPlatform` before preparation is reached,
/// so this path is never executed and makes no Windows support claim.
#[cfg(unix)]
fn raw_os_bytes(value: &std::ffi::OsStr) -> &[u8] {
    use std::os::unix::ffi::OsStrExt;
    value.as_bytes()
}

#[cfg(not(unix))]
fn raw_os_bytes(value: &std::ffi::OsStr) -> &[u8] {
    value.as_encoded_bytes()
}

/// Result of the one canonical-cwd preparation pass.
fn prepare_canonical_cwd(
    cwd: &Path,
    max_cwd_bytes: usize,
) -> Result<PathBuf, ProcessError> {
    if cwd.as_os_str().is_empty() {
        return Err(ProcessError::InvalidCwd {
            reason: "cwd must not be empty".to_owned(),
        });
    }
    if raw_os_bytes(cwd.as_os_str()).contains(&0) {
        return Err(ProcessError::InvalidCwd {
            reason: "cwd must not contain NUL".to_owned(),
        });
    }
    if raw_os_bytes(cwd.as_os_str()).len() > max_cwd_bytes {
        return Err(ProcessError::InvalidCwd {
            reason: "cwd exceeds max_cwd_bytes".to_owned(),
        });
    }
    let canonical = std::fs::canonicalize(cwd).map_err(|_| ProcessError::InvalidCwd {
        reason: "cwd could not be canonicalized".to_owned(),
    })?;
    if !canonical.is_absolute() {
        return Err(ProcessError::InvalidCwd {
            reason: "canonical cwd is not absolute".to_owned(),
        });
    }
    if raw_os_bytes(canonical.as_os_str()).len() > max_cwd_bytes {
        return Err(ProcessError::InvalidCwd {
            reason: "canonical cwd exceeds max_cwd_bytes".to_owned(),
        });
    }
    match std::fs::metadata(&canonical) {
        Ok(meta) if meta.is_dir() => Ok(canonical),
        _ => Err(ProcessError::InvalidCwd {
            reason: "canonical cwd is not a directory".to_owned(),
        }),
    }
}

/// Validate every request/limit invariant before authorization or spawn.
fn validate_process_limits(limits: &ProcessLimits) -> Result<(), ProcessError> {
    let invalid = |reason: &str| ProcessError::InvalidLimits {
        reason: reason.to_owned(),
    };
    if limits.timeout.is_zero() || limits.timeout > MAX_PROCESS_TIMEOUT {
        return Err(invalid("timeout must be positive and at most 300s"));
    }
    if limits.startup_timeout.is_zero() || limits.startup_timeout > MAX_PROCESS_TIMEOUT {
        return Err(invalid("startup_timeout must be positive and at most 300s"));
    }
    if limits.startup_timeout > limits.timeout {
        return Err(invalid("startup_timeout must not exceed timeout"));
    }
    if limits.cleanup_timeout.is_zero() || limits.cleanup_timeout > MAX_PROCESS_TIMEOUT {
        return Err(invalid("cleanup_timeout must be positive and at most 300s"));
    }
    if limits.max_stdout_bytes == 0 || limits.max_stdout_bytes > MAX_PROCESS_OUTPUT_BYTES {
        return Err(invalid("max_stdout_bytes must be 1..=10 MiB"));
    }
    if limits.max_stderr_bytes == 0 || limits.max_stderr_bytes > MAX_PROCESS_OUTPUT_BYTES {
        return Err(invalid("max_stderr_bytes must be 1..=10 MiB"));
    }
    if limits.max_argv_bytes == 0 || limits.max_argv_bytes > MAX_PROCESS_ARGV_BYTES {
        return Err(invalid("max_argv_bytes must be 1..=64 KiB"));
    }
    if limits.max_argv_elements == 0 || limits.max_argv_elements > MAX_PROCESS_ARGV_ELEMENTS {
        return Err(invalid("max_argv_elements must be 1..=4096"));
    }
    if limits.max_env_bytes == 0 || limits.max_env_bytes > MAX_PROCESS_ENV_BYTES {
        return Err(invalid("max_env_bytes must be 1..=64 KiB"));
    }
    if limits.max_env_elements == 0 || limits.max_env_elements > MAX_PROCESS_ENV_ELEMENTS {
        return Err(invalid("max_env_elements must be 1..=1024"));
    }
    if limits.max_cwd_bytes == 0 || limits.max_cwd_bytes > MAX_PROCESS_CWD_BYTES {
        return Err(invalid("max_cwd_bytes must be 1..=4 KiB"));
    }
    Ok(())
}

/// Validate the direct-argv request shape. Never echoes raw values.
fn validate_process_request(
    request: &ProcessRequest,
    limits: &ProcessLimits,
) -> Result<(), ProcessError> {
    if request.program.is_empty() {
        return Err(ProcessError::InvalidRequest {
            field: "program",
            reason: "program must not be empty".to_owned(),
        });
    }
    if request.program.contains('\0') {
        return Err(ProcessError::InvalidRequest {
            field: "program",
            reason: "program must not contain NUL".to_owned(),
        });
    }
    if request.args.len() > limits.max_argv_elements {
        return Err(ProcessError::InvalidRequest {
            field: "args",
            reason: "argv element count exceeds max_argv_elements".to_owned(),
        });
    }
    let mut argv_bytes = 0usize;
    for arg in &request.args {
        if arg.contains('\0') {
            return Err(ProcessError::InvalidRequest {
                field: "args",
                reason: "argv element must not contain NUL".to_owned(),
            });
        }
        argv_bytes = argv_bytes.saturating_add(arg.len());
    }
    if argv_bytes > limits.max_argv_bytes {
        return Err(ProcessError::InvalidRequest {
            field: "args",
            reason: "argv byte total exceeds max_argv_bytes".to_owned(),
        });
    }
    if request.env.len() > limits.max_env_elements {
        return Err(ProcessError::InvalidRequest {
            field: "env",
            reason: "environment entry count exceeds max_env_elements".to_owned(),
        });
    }
    let mut env_bytes = 0usize;
    for (key, value) in &request.env {
        if key.is_empty() {
            return Err(ProcessError::InvalidRequest {
                field: "env",
                reason: "environment key must not be empty".to_owned(),
            });
        }
        if key.contains('\0') || key.contains('=') {
            return Err(ProcessError::InvalidRequest {
                field: "env",
                reason: "environment key must be NUL-free and '='-free".to_owned(),
            });
        }
        if value.contains('\0') {
            return Err(ProcessError::InvalidRequest {
                field: "env",
                reason: "environment value must not contain NUL".to_owned(),
            });
        }
        env_bytes = env_bytes
            .saturating_add(key.len())
            .saturating_add(value.len());
    }
    if env_bytes > limits.max_env_bytes {
        return Err(ProcessError::InvalidRequest {
            field: "env",
            reason: "environment byte total exceeds max_env_bytes".to_owned(),
        });
    }
    Ok(())
}

/// Bounded result with no child: `CancelledBeforeStart`/pre-spawn `TimedOut`.
fn no_child_result(terminal: ProcessTerminal, start: TokioInstant) -> ProcessResult {
    ProcessResult {
        terminal,
        stdout: String::new(),
        stderr: String::new(),
        stdout_truncated: false,
        stderr_truncated: false,
        cleanup: CleanupStatus::NotRequired,
        duration_ms: process_duration_ms(start),
    }
}

/// Read the retained prefix of one pipe, draining the remainder so the child
/// never blocks on a full pipe and the retained buffer never exceeds `cap`.
#[cfg(unix)]
async fn read_capped<R>(mut reader: R, cap: usize) -> std::io::Result<ReaderOutcome>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut outcome = ReaderOutcome::default();
    let mut chunk = [0u8; 8 * 1024];
    loop {
        let read = reader.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        if outcome.bytes.len() < cap {
            let room = cap - outcome.bytes.len();
            outcome.bytes.extend_from_slice(&chunk[..read.min(room)]);
            if read > room {
                outcome.truncated = true;
            }
        } else {
            outcome.truncated = true;
        }
    }
    Ok(outcome)
}

/// Join one reader task within the remaining cleanup budget. A timed-out or
/// failed task is aborted so no detached reader remains.
#[cfg(unix)]
async fn join_one_reader(
    handle: JoinHandle<std::io::Result<ReaderOutcome>>,
    deadline: TokioInstant,
) -> (bool, Option<ReaderOutcome>) {
    let remaining = deadline
        .saturating_duration_since(TokioInstant::now())
        .max(MIN_PROCESS_TICK);
    let mut handle = handle;
    match timeout(remaining, &mut handle).await {
        Ok(Ok(Ok(outcome))) => (true, Some(outcome)),
        _ => {
            handle.abort();
            (false, None)
        }
    }
}

/// Join both owned readers. Returns the cleanup step and any retained output.
#[cfg(unix)]
async fn join_readers(
    readers: Readers,
    deadline: TokioInstant,
) -> (CleanupStep, Option<ReaderOutcome>, Option<ReaderOutcome>) {
    let mut required = false;
    let mut all_ok = true;
    let (out_ok, out_outcome) = match readers.stdout {
        Some(handle) => {
            required = true;
            join_one_reader(handle, deadline).await
        }
        None => (true, None),
    };
    let (err_ok, err_outcome) = match readers.stderr {
        Some(handle) => {
            required = true;
            join_one_reader(handle, deadline).await
        }
        None => (true, None),
    };
    all_ok &= out_ok && err_ok;
    let step = if !required {
        CleanupStep::NotRequired
    } else if all_ok {
        CleanupStep::Succeeded
    } else {
        CleanupStep::Failed
    };
    (step, out_outcome, err_outcome)
}

/// Owned cleanup: group kill, direct child-kill fallback, explicit wait, then
/// reader join, all inside one `cleanup_timeout` budget.
#[cfg(unix)]
async fn cleanup_process(
    child: &mut tokio::process::Child,
    group: Option<u32>,
    readers: Readers,
    budget: Duration,
) -> (
    CleanupStatus,
    Option<ReaderOutcome>,
    Option<ReaderOutcome>,
) {
    let deadline = TokioInstant::now() + budget;

    // 1. Group kill first.
    let (group_step, group_done) = match group
        .and_then(|raw| i32::try_from(raw).ok())
        .and_then(Pid::from_raw)
    {
        Some(pid) => match kill_process_group(pid, Signal::KILL) {
            Ok(()) => (CleanupStep::Succeeded, true),
            // ESRCH: the group is already gone, so termination is established.
            Err(rustix::io::Errno::SRCH) => (CleanupStep::NotRequired, true),
            Err(_) => (CleanupStep::Failed, false),
        },
        None => (CleanupStep::NotRequired, false),
    };

    // 2. Direct child kill only when the group kill did not establish death.
    let child_step = if group_done {
        CleanupStep::NotRequired
    } else {
        match child.start_kill() {
            Ok(()) => CleanupStep::Succeeded,
            Err(_) => CleanupStep::Failed,
        }
    };

    // 3. Explicit bounded wait/reap.
    let remaining = deadline
        .saturating_duration_since(TokioInstant::now())
        .max(MIN_PROCESS_TICK);
    let wait_step = match timeout(remaining, child.wait()).await {
        Ok(Ok(_)) => CleanupStep::Succeeded,
        _ => CleanupStep::Failed,
    };

    // 4. Join and drain the owned readers.
    let (readers_step, stdout_outcome, stderr_outcome) = join_readers(readers, deadline).await;

    let required_ok = !matches!(
        group_step,
        CleanupStep::Failed
    ) && !matches!(child_step, CleanupStep::Failed)
        && wait_step == CleanupStep::Succeeded
        && !matches!(readers_step, CleanupStep::Failed);

    let status = if required_ok {
        CleanupStatus::Reaped {
            group_kill: group_step,
            child_kill: child_step,
            wait: wait_step,
            readers: readers_step,
        }
    } else {
        CleanupStatus::Failed {
            group_kill: group_step,
            child_kill: child_step,
            wait: wait_step,
            readers: readers_step,
        }
    };
    (status, stdout_outcome, stderr_outcome)
}

/// Render one retained stream plus its single truncation marker.
#[cfg(unix)]
fn render_process_stream(
    outcome: Option<ReaderOutcome>,
) -> (String, bool) {
    match outcome {
        Some(outcome) => {
            let mut text = String::from_utf8_lossy(&outcome.bytes).into_owned();
            if outcome.truncated {
                text.push_str(PROCESS_TRUNCATED_MARKER);
            }
            (text, outcome.truncated)
        }
        None => (String::new(), false),
    }
}

fn elapsed_ms(start: Instant) -> u64 {
    start.elapsed().as_millis().max(1) as u64
}

fn parse_optional_usize(input: &Value, field: &'static str) -> Result<Option<usize>, String> {
    let Some(value) = input.get(field) else {
        return Ok(None);
    };
    let parsed = value
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("Invalid '{field}' field in input"))?;
    Ok(Some(parsed))
}

fn shell_cwd(input: &Value) -> Result<PathBuf, String> {
    let Some(value) = input.get("cwd") else {
        return std::env::current_dir().map_err(|_| "Invalid 'cwd' field in input".to_owned());
    };
    let Some(cwd) = value.as_str() else {
        return Err("Invalid 'cwd' field in input".to_owned());
    };
    if cwd.trim().is_empty() || cwd.contains('\0') {
        return Err("Invalid 'cwd' field in input".to_owned());
    }
    Ok(PathBuf::from(cwd))
}

/// Configuration for tool execution timeouts.
#[derive(Clone, Copy, Debug)]
pub struct TimeoutConfig {
    /// Default timeout in milliseconds for tool calls without explicit timeout.
    pub default_timeout_ms: u64,
    /// Maximum allowed timeout in milliseconds.
    pub max_timeout_ms: u64,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            default_timeout_ms: 30_000,
            max_timeout_ms: 300_000,
        }
    }
}

/// A tool call to be executed.
#[derive(Clone, Debug)]
pub struct ToolCall {
    /// Unique identifier for this tool call.
    pub tool_id: String,
    /// Name of the tool to execute.
    pub name: String,
    /// Input parameters for the tool.
    pub input: Value,
    /// Optional timeout in milliseconds.
    pub timeout_ms: Option<u64>,
}

impl ToolCall {
    pub fn new(tool_id: impl Into<String>, name: impl Into<String>, input: Value) -> Self {
        Self {
            tool_id: tool_id.into(),
            name: name.into(),
            input,
            timeout_ms: None,
        }
    }

    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }
}

/// Result of a tool execution.
#[derive(Debug)]
pub struct ToolResult {
    /// Tool identifier from the call.
    pub tool_id: String,
    /// Output from the tool execution (stdout).
    pub output: String,
    /// Whether the execution succeeded.
    pub success: bool,
    /// Duration of execution in milliseconds.
    pub duration_ms: u64,
    /// Error message if execution failed or timed out.
    pub error: Option<String>,
}

/// Executor for running tool calls with configurable timeouts.
#[derive(Debug)]
pub struct ToolExecutor {
    timeout_config: TimeoutConfig,
}

impl ToolExecutor {
    pub fn new() -> Self {
        Self {
            timeout_config: TimeoutConfig::default(),
        }
    }

    pub fn with_timeout_config(config: TimeoutConfig) -> Self {
        Self {
            timeout_config: config,
        }
    }

    /// Execute a single tool call with timeout handling.
    pub async fn execute(&self, call: ToolCall) -> ToolResult {
        let start = Instant::now();

        let effective_timeout = call
            .timeout_ms
            .map(|t| Duration::from_millis(t.min(self.timeout_config.max_timeout_ms)))
            .unwrap_or_else(|| Duration::from_millis(self.timeout_config.default_timeout_ms));

        // Shell execution is never available through the generic path. The
        // brokered entrypoint below is the only path allowed to construct a
        // process command.
        let result = if call.name == "bash" || call.name == "shell" {
            ToolResult {
                tool_id: call.tool_id,
                output: String::new(),
                success: false,
                duration_ms: elapsed_ms(start),
                error: Some("shell execution denied: broker authorization required".to_owned()),
            }
        } else if call.name == "echo" {
            self.execute_echo(&call, effective_timeout).await
        } else {
            let duration_ms = elapsed_ms(start);
            ToolResult {
                tool_id: call.tool_id,
                output: String::new(),
                success: false,
                duration_ms,
                error: Some(format!("Unknown tool: {}", call.name)),
            }
        };

        result
    }

    /// Execute a call whose concrete resource must be authorized by the
    /// supplied broker. File reads never use the generic unbrokered path.
    pub async fn execute_authorized(
        &self,
        call: ToolCall,
        broker: &PermissionBroker,
    ) -> ToolResult {
        let effective_timeout = call
            .timeout_ms
            .map(|value| Duration::from_millis(value.min(self.timeout_config.max_timeout_ms)))
            .unwrap_or_else(|| Duration::from_millis(self.timeout_config.default_timeout_ms));
        if call.name == "bash" || call.name == "shell" {
            return self
                .execute_shell_authorized(&call, effective_timeout, broker)
                .await;
        }
        if call.name != "read" {
            return self.execute(call).await;
        }
        self.execute_read(call, broker.clone()).await
    }

    /// Execute one direct-argv process under an explicit capability decision.
    ///
    /// This is the only public entrypoint allowed to construct a process
    /// command. It validates and canonicalizes once, authorizes before spawn
    /// through [`ToolAuthorizer`], binds the exact canonical cwd/argv/program to
    /// both the broker intent and the `Command`, clears the environment, and
    /// owns cancellation, readiness, deadlines, bounded output and explicit
    /// cleanup for the whole request lifetime.
    pub async fn execute_authorized_process(
        &self,
        request: ProcessRequest,
        authorizer: &mut ToolAuthorizer<'_>,
        grant: Option<&Grant>,
        expected: &ExpectedScope,
        limits: ProcessLimits,
        mut cancellation: ProcessCancellation,
        ready: oneshot::Sender<ProcessReady>,
    ) -> Result<ProcessResult, ProcessError> {
        #[cfg(not(unix))]
        {
            let _ = (
                request,
                authorizer,
                grant,
                expected,
                limits,
                cancellation,
                ready,
            );
            return Err(ProcessError::UnsupportedPlatform {
                operation: "execute_authorized_process",
            });
        }

        #[cfg(unix)]
        {
            let start = process_now();
            let deadline = start + limits.timeout;

            // 1. Validate limits and request shape before touching the broker.
            validate_process_limits(&limits)?;
            validate_process_request(&request, &limits)?;
            let canonical_cwd = prepare_canonical_cwd(&request.cwd, limits.max_cwd_bytes)?;

            // 2. Cancellation before authorization: no spawn, no grant burn.
            if *cancellation.borrow() {
                return Ok(no_child_result(
                    ProcessTerminal::CancelledBeforeStart,
                    start,
                ));
            }

            // 3. Authorize the exact direct-argv intent. The baseline decision
            //    is authoritative: a mandatory `Deny` is terminal and a grant
            //    can never lift it. Only a baseline human gate may consult the
            //    single-use grant; any rejected grant is `InvalidGrant`.
            let intent = OperationIntent::Process {
                program: request.program.clone(),
                args: request.args.clone(),
                cwd: canonical_cwd.clone(),
            };
            match authorizer.authorize(&intent) {
                ToolGate::Allow => {}
                ToolGate::Deny { .. } => {
                    return Err(ProcessError::Denied {
                        reason: "denied by policy".to_owned(),
                    });
                }
                ToolGate::HumanGate { .. } => match grant {
                    None => {
                        return Err(ProcessError::HumanRequired {
                            reason: "requires human approval".to_owned(),
                        });
                    }
                    Some(grant) => {
                        match authorizer.authorize_with_grant(&intent, grant, expected) {
                            ToolGate::Allow => {}
                            ToolGate::Deny { reason } | ToolGate::HumanGate { reason } => {
                                let reason = if reason.trim().is_empty() {
                                    "grant rejected".to_owned()
                                } else {
                                    bounded_process_reason(&reason)
                                };
                                return Err(ProcessError::InvalidGrant { reason });
                            }
                        }
                    }
                },
            }

            // 4. Recheck cancellation then deadline before spawn. Cancellation
            //    wins over timeout per the fixed precedence.
            if *cancellation.borrow() {
                return Ok(no_child_result(
                    ProcessTerminal::CancelledBeforeStart,
                    start,
                ));
            }
            if process_now() >= deadline {
                return Ok(no_child_result(ProcessTerminal::TimedOut, start));
            }

            // 5. Spawn with the exact canonical cwd, exact argv, and a cleared
            //    explicit environment. Unix: own a fresh process group.
            let mut command = Command::new(&request.program);
            command
                .args(&request.args)
                .current_dir(&canonical_cwd)
                .env_clear();
            for (key, value) in &request.env {
                command.env(key, value);
            }
            if !request.env.contains_key("PATH") {
                command.env("PATH", PROCESS_MINIMAL_PATH);
            }
            command
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            command.process_group(0);

            let spawn_attempt = process_now();
            let mut child = match command.spawn() {
                Ok(child) => child,
                Err(_) => {
                    return Err(ProcessError::Spawn {
                        reason: "process spawn failed".to_owned(),
                    });
                }
            };
            let pid = child.id().unwrap_or(0);
            let group_raw = pid;

            // 6. Own both readers before publishing readiness.
            let mut readers = Readers::default();
            if let Some(stdout) = child.stdout.take() {
                readers.stdout = Some(tokio::spawn(read_capped(stdout, limits.max_stdout_bytes)));
            }
            if let Some(stderr) = child.stderr.take() {
                readers.stderr = Some(tokio::spawn(read_capped(stderr, limits.max_stderr_bytes)));
            }

            // 6b. The startup window runs from the spawn attempt until
            //     readiness publication; a breach latches a timeout and runs
            //     the owned cleanup without publishing readiness.
            if process_now() >= spawn_attempt + limits.startup_timeout {
                let (status, stdout_outcome, stderr_outcome) = cleanup_process(
                    &mut child,
                    Some(group_raw),
                    readers,
                    limits.cleanup_timeout,
                )
                .await;
                if matches!(status, CleanupStatus::Failed { .. }) {
                    return Err(ProcessError::CleanupFailed {
                        terminal: ProcessTerminal::TimedOut,
                        status,
                    });
                }
                let (stdout, stdout_truncated) = render_process_stream(stdout_outcome);
                let (stderr, stderr_truncated) = render_process_stream(stderr_outcome);
                return Ok(ProcessResult {
                    terminal: ProcessTerminal::TimedOut,
                    stdout,
                    stderr,
                    stdout_truncated,
                    stderr_truncated,
                    cleanup: status,
                    duration_ms: process_duration_ms(start),
                });
            }

            // 7. Reject an invalid PID: bounded cleanup first, typed error after.
            if pid == 0 {
                let (status, _, _) =
                    cleanup_process(&mut child, Some(group_raw), readers, limits.cleanup_timeout)
                        .await;
                return Err(if matches!(status, CleanupStatus::Reaped { .. }) {
                    ProcessError::Io {
                        operation: "readiness_pid",
                        reason: "spawned child reported an invalid PID".to_owned(),
                    }
                } else {
                    ProcessError::CleanupFailed {
                        terminal: ProcessTerminal::Cancelled,
                        status,
                    }
                });
            }

            // 8. Publish exactly one readiness event.
            let ready_event = ProcessReady {
                pid,
                process_group: group_raw,
            };
            if ready.send(ready_event).is_err() {
                let (status, _, _) = cleanup_process(
                    &mut child,
                    Some(group_raw),
                    readers,
                    limits.cleanup_timeout,
                )
                .await;
                return Err(if matches!(status, CleanupStatus::Reaped { .. }) {
                    ProcessError::ReadinessReceiverClosed { pid }
                } else {
                    ProcessError::CleanupFailed {
                        terminal: ProcessTerminal::Cancelled,
                        status,
                    }
                });
            }

            // 9. Bounded running observation with deterministic precedence:
            //    child completion, then cancellation, then deadline.
            let terminal;
            let mut wait_error = false;
            let mut cancel_closed = false;
            // Latch a cancellation already observable at readiness (it must not
            // race the loop's first poll), after exactly one readiness event.
            if *cancellation.borrow() {
                let (status, stdout_outcome, stderr_outcome) = cleanup_process(
                    &mut child,
                    Some(group_raw),
                    readers,
                    limits.cleanup_timeout,
                )
                .await;
                if matches!(status, CleanupStatus::Failed { .. }) {
                    return Err(ProcessError::CleanupFailed {
                        terminal: ProcessTerminal::Cancelled,
                        status,
                    });
                }
                let (stdout, stdout_truncated) = render_process_stream(stdout_outcome);
                let (stderr, stderr_truncated) = render_process_stream(stderr_outcome);
                return Ok(ProcessResult {
                    terminal: ProcessTerminal::Cancelled,
                    stdout,
                    stderr,
                    stdout_truncated,
                    stderr_truncated,
                    cleanup: status,
                    duration_ms: process_duration_ms(start),
                });
            }
            loop {
                tokio::select! {
                    biased;
                    status = child.wait() => {
                        match status {
                            Ok(status) => {
                                terminal = ProcessTerminal::Exited {
                                    exit_code: status.code(),
                                };
                            }
                            Err(_) => {
                                // Placeholder never observed: the wait_error
                                // guard below returns before `terminal` is used.
                                terminal = ProcessTerminal::Cancelled;
                                wait_error = true;
                            }
                        }
                        break;
                    }
                    changed = cancellation.changed(), if !cancel_closed => {
                        match changed {
                            Ok(()) => {
                                if *cancellation.borrow() {
                                    terminal = ProcessTerminal::Cancelled;
                                    break;
                                }
                            }
                            Err(_) => {
                                cancel_closed = true;
                            }
                        }
                    }
                    _ = tokio::time::sleep_until(deadline) => {
                        terminal = ProcessTerminal::TimedOut;
                        break;
                    }
                }
            }

            // 9b. A wait error is a bounded `Io` failure, but it must not leak
            //     the child or readers: run owned cleanup first.
            if wait_error {
                let (status, _, _) = cleanup_process(
                    &mut child,
                    Some(group_raw),
                    readers,
                    limits.cleanup_timeout,
                )
                .await;
                return Err(if matches!(status, CleanupStatus::Reaped { .. }) {
                    ProcessError::Io {
                        operation: "child_wait",
                        reason: "child wait failed".to_owned(),
                    }
                } else {
                    ProcessError::CleanupFailed {
                        terminal: ProcessTerminal::Cancelled,
                        status,
                    }
                });
            }

            // 10. Terminal cleanup. Normal exit reaps via the successful wait and
            //     only needs the readers joined; cancellation/timeout run the
            //     full group-kill/child-kill/wait/readers sequence.
            match terminal {
                ProcessTerminal::Exited { .. } => {
                    let cleanup_deadline = TokioInstant::now() + limits.cleanup_timeout;
                    let (readers_step, stdout_outcome, stderr_outcome) =
                        join_readers(readers, cleanup_deadline).await;
                    let status = if readers_step == CleanupStep::Succeeded {
                        CleanupStatus::Reaped {
                            group_kill: CleanupStep::NotRequired,
                            child_kill: CleanupStep::NotRequired,
                            wait: CleanupStep::Succeeded,
                            readers: readers_step,
                        }
                    } else {
                        CleanupStatus::Failed {
                            group_kill: CleanupStep::NotRequired,
                            child_kill: CleanupStep::NotRequired,
                            wait: CleanupStep::Succeeded,
                            readers: readers_step,
                        }
                    };
                    if matches!(status, CleanupStatus::Failed { .. }) {
                        return Err(ProcessError::CleanupFailed { terminal, status });
                    }
                    let (stdout, stdout_truncated) = render_process_stream(stdout_outcome);
                    let (stderr, stderr_truncated) = render_process_stream(stderr_outcome);
                    Ok(ProcessResult {
                        terminal,
                        stdout,
                        stderr,
                        stdout_truncated,
                        stderr_truncated,
                        cleanup: status,
                        duration_ms: process_duration_ms(start),
                    })
                }
                ProcessTerminal::TimedOut | ProcessTerminal::Cancelled => {
                    let (status, stdout_outcome, stderr_outcome) = cleanup_process(
                        &mut child,
                        Some(group_raw),
                        readers,
                        limits.cleanup_timeout,
                    )
                    .await;
                    if matches!(status, CleanupStatus::Failed { .. }) {
                        return Err(ProcessError::CleanupFailed { terminal, status });
                    }
                    let (stdout, stdout_truncated) = render_process_stream(stdout_outcome);
                    let (stderr, stderr_truncated) = render_process_stream(stderr_outcome);
                    Ok(ProcessResult {
                        terminal,
                        stdout,
                        stderr,
                        stdout_truncated,
                        stderr_truncated,
                        cleanup: status,
                        duration_ms: process_duration_ms(start),
                    })
                }
                ProcessTerminal::CancelledBeforeStart => {
                    Ok(no_child_result(ProcessTerminal::CancelledBeforeStart, start))
                }
            }
        }
    }

    async fn execute_read(&self, call: ToolCall, broker: PermissionBroker) -> ToolResult {
        let start = Instant::now();
        let Some(path) = call.input.get("path").and_then(Value::as_str) else {
            return ToolResult {
                tool_id: call.tool_id,
                output: String::new(),
                success: false,
                duration_ms: elapsed_ms(start),
                error: Some("Missing 'path' field in input".to_owned()),
            };
        };
        if path.trim().is_empty() || path.contains('\0') {
            return ToolResult {
                tool_id: call.tool_id,
                output: String::new(),
                success: false,
                duration_ms: elapsed_ms(start),
                error: Some("Invalid 'path' field in input".to_owned()),
            };
        }
        let offset = match parse_optional_usize(&call.input, "offset") {
            Ok(value) => value.unwrap_or(0),
            Err(error) => {
                return ToolResult {
                    tool_id: call.tool_id,
                    output: String::new(),
                    success: false,
                    duration_ms: elapsed_ms(start),
                    error: Some(error),
                };
            }
        };
        let limit = match parse_optional_usize(&call.input, "limit") {
            Ok(value) => value
                .unwrap_or(crate::file_ops::MAX_READ_BYTES)
                .min(crate::file_ops::MAX_READ_BYTES),
            Err(error) => {
                return ToolResult {
                    tool_id: call.tool_id,
                    output: String::new(),
                    success: false,
                    duration_ms: elapsed_ms(start),
                    error: Some(error),
                };
            }
        };
        let op = crate::file_ops::FileOperation::read_with_range(path, offset, Some(limit));
        let tool_id = call.tool_id;
        let effective_timeout = call
            .timeout_ms
            .map(|value| Duration::from_millis(value.min(self.timeout_config.max_timeout_ms)))
            .unwrap_or_else(|| Duration::from_millis(self.timeout_config.default_timeout_ms));
        let task =
            tokio::task::spawn_blocking(move || crate::file_ops::execute_authorized(op, &broker));
        match timeout(effective_timeout, task).await {
            Ok(Ok(Ok(result))) => ToolResult {
                tool_id,
                output: result.content,
                success: result.success,
                duration_ms: elapsed_ms(start),
                error: result.error,
            },
            Ok(Ok(Err(_))) => ToolResult {
                tool_id,
                output: String::new(),
                success: false,
                duration_ms: elapsed_ms(start),
                error: Some("file read failed".to_owned()),
            },
            Ok(Err(_)) => ToolResult {
                tool_id,
                output: String::new(),
                success: false,
                duration_ms: elapsed_ms(start),
                error: Some("file read task failed".to_owned()),
            },
            // Dropping a JoinHandle after timeout stops awaiting the result but
            // cannot cancel blocking I/O already running on Tokio's pool. The
            // file operation has a fixed 64 KiB read cap, so its work is bounded;
            // this timeout is not hard cancellation.
            Err(_) => ToolResult {
                tool_id,
                output: String::new(),
                success: false,
                duration_ms: elapsed_ms(start),
                error: Some("Execution timed out".to_owned()),
            },
        }
    }

    async fn execute_shell_authorized(
        &self,
        call: &ToolCall,
        timeout_duration: Duration,
        broker: &PermissionBroker,
    ) -> ToolResult {
        let start = Instant::now();
        let Some(command) = call.input.get("command").and_then(Value::as_str) else {
            return ToolResult {
                tool_id: call.tool_id.clone(),
                output: String::new(),
                success: false,
                duration_ms: elapsed_ms(start),
                error: Some("Missing 'command' field in input".to_owned()),
            };
        };
        if command.is_empty() || command.contains('\0') {
            return ToolResult {
                tool_id: call.tool_id.clone(),
                output: String::new(),
                success: false,
                duration_ms: elapsed_ms(start),
                error: Some("Invalid 'command' field in input".to_owned()),
            };
        }

        let cwd = match shell_cwd(&call.input) {
            Ok(cwd) => cwd,
            Err(error) => {
                return ToolResult {
                    tool_id: call.tool_id.clone(),
                    output: String::new(),
                    success: false,
                    duration_ms: elapsed_ms(start),
                    error: Some(error),
                };
            }
        };
        let args = vec!["-c".to_owned(), command.to_owned()];
        let intent = OperationIntent::Process {
            program: "bash".to_owned(),
            args: args.clone(),
            cwd: cwd.clone(),
        };

        match broker.authorize(&intent) {
            Decision::Allow => {}
            Decision::Deny { .. } | Decision::RequireHuman { .. } => {
                return ToolResult {
                    tool_id: call.tool_id.clone(),
                    output: String::new(),
                    success: false,
                    duration_ms: elapsed_ms(start),
                    error: Some("shell execution denied by broker".to_owned()),
                };
            }
        }

        let mut process = Command::new("bash");
        process.args(&args).current_dir(&cwd).env_clear().env(
            "PATH",
            "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        );
        let cmd = process.output();

        match timeout(timeout_duration, cmd).await {
            Ok(output_result) => match output_result {
                Ok(output) => {
                    let duration_ms = elapsed_ms(start);
                    let success = output.status.success();
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

                    let error = if success {
                        None
                    } else {
                        Some(format!("Command failed with status: {}", output.status))
                    };

                    ToolResult {
                        tool_id: call.tool_id.clone(),
                        output: stdout,
                        success,
                        duration_ms,
                        error,
                    }
                }
                Err(_) => ToolResult {
                    tool_id: call.tool_id.clone(),
                    output: String::new(),
                    success: false,
                    duration_ms: elapsed_ms(start),
                    error: Some("Failed to execute command".to_owned()),
                },
            },
            Err(_) => ToolResult {
                tool_id: call.tool_id.clone(),
                output: String::new(),
                success: false,
                duration_ms: elapsed_ms(start),
                error: Some("Execution timed out".to_owned()),
            },
        }
    }

    async fn execute_echo(&self, call: &ToolCall, timeout_duration: Duration) -> ToolResult {
        let start = Instant::now();

        let message = call
            .input
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let cmd = Command::new("echo").arg(message).output();

        match timeout(timeout_duration, cmd).await {
            Ok(output_result) => match output_result {
                Ok(output) => {
                    let duration_ms = elapsed_ms(start);
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    ToolResult {
                        tool_id: call.tool_id.clone(),
                        output: stdout,
                        success: true,
                        duration_ms,
                        error: None,
                    }
                }
                Err(e) => ToolResult {
                    tool_id: call.tool_id.clone(),
                    output: String::new(),
                    success: false,
                    duration_ms: elapsed_ms(start),
                    error: Some(format!("Echo failed: {}", e)),
                },
            },
            Err(_) => ToolResult {
                tool_id: call.tool_id.clone(),
                output: String::new(),
                success: false,
                duration_ms: elapsed_ms(start),
                error: Some("Execution timed out".to_string()),
            },
        }
    }

    /// Execute multiple tool calls concurrently.
    pub async fn execute_batch(&self, calls: Vec<ToolCall>) -> Vec<ToolResult> {
        let config = self.timeout_config.clone();
        let mut set = tokio::task::JoinSet::new();
        let mut results: Vec<Option<ToolResult>> = (0..calls.len()).map(|_| None).collect();

        for (idx, call) in calls.into_iter().enumerate() {
            set.spawn(async move {
                let executor = ToolExecutor::with_timeout_config(config.clone());
                let result = executor.execute(call).await;
                (idx, result)
            });
        }

        while let Some(join_result) = set.join_next().await {
            match join_result {
                Ok((idx, tool_result)) => results[idx] = Some(tool_result),
                Err(e) => {
                    // Cannot recover idx on JoinError, find first empty slot
                    if let Some(pos) = results.iter().position(|r| r.is_none()) {
                        results[pos] = Some(ToolResult {
                            tool_id: String::new(),
                            output: String::new(),
                            success: false,
                            duration_ms: 0,
                            error: Some(format!("Task join error: {}", e)),
                        });
                    }
                }
            }
        }

        results
            .into_iter()
            .map(|r| {
                r.unwrap_or_else(|| ToolResult {
                    tool_id: String::new(),
                    output: String::new(),
                    success: false,
                    duration_ms: 0,
                    error: Some("missing result".to_string()),
                })
            })
            .collect()
    }
}

impl Default for ToolExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_shell_call(id: &str, cmd: &str) -> ToolCall {
        ToolCall::new(id, "bash", json!({ "command": cmd }))
    }

    fn make_echo_call(id: &str, msg: &str) -> ToolCall {
        ToolCall::new(id, "echo", json!({ "message": msg }))
    }

    #[tokio::test]
    async fn execute_success() {
        let executor = ToolExecutor::new();
        let call = make_shell_call("test-1", "echo hello");

        let result = executor.execute(call).await;

        assert!(result.success);
        assert!(result.output.contains("hello"));
        assert!(result.error.is_none());
        assert!(result.duration_ms > 0);
    }

    #[tokio::test]
    async fn execute_timeout() {
        let config = TimeoutConfig {
            default_timeout_ms: 1,
            max_timeout_ms: 1000,
        };
        let executor = ToolExecutor::with_timeout_config(config);
        let call = make_shell_call("timeout-test", "sleep 10").with_timeout(1);

        let start = Instant::now();
        let result = executor.execute(call).await;
        let elapsed = start.elapsed().as_millis();

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("timed out"));
        assert!(elapsed < 2000, "Should timeout quickly");
    }

    #[tokio::test]
    async fn execute_failure() {
        let executor = ToolExecutor::new();
        let call = make_shell_call("fail-test", "exit 1");

        let result = executor.execute(call).await;

        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn batch_executes_all() {
        let executor = ToolExecutor::new();
        let calls = vec![
            make_echo_call("batch-1", "first"),
            make_echo_call("batch-2", "second"),
            make_echo_call("batch-3", "third"),
        ];

        let results = executor.execute_batch(calls).await;

        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.success));
        assert!(results[0].output.contains("first"));
        assert!(results[1].output.contains("second"));
        assert!(results[2].output.contains("third"));
    }

    #[tokio::test]
    async fn result_has_duration() {
        let result = ToolResult {
            tool_id: "duration-test".to_string(),
            output: "test output".to_string(),
            success: true,
            duration_ms: 123,
            error: None,
        };

        assert_eq!(result.duration_ms, 123);
        assert!(result.success);
    }
}
