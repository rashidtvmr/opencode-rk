//! Bounded native provider-tool executor.
use crate::file_ops::{FileOperation, FileTool};
use crate::shell_tool::{ShellConfig, ShellTool};
use opencode_rk_security::{
    Decision, FileAction, HumanApproval, OperationIntent, PermissionBroker,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::time::timeout;

const MAX_TEXT: usize = 1_048_576;
fn elapsed(start: Instant) -> u64 {
    start.elapsed().as_millis().max(1) as u64
}
fn fail(call: &ToolCall, start: Instant, error: impl Into<String>) -> ToolResult {
    ToolResult {
        tool_id: call.tool_id.clone(),
        output: String::new(),
        success: false,
        duration_ms: elapsed(start),
        error: Some(error.into()),
    }
}
fn cap(text: String) -> String {
    if text.len() <= MAX_TEXT {
        text
    } else {
        let mut end = MAX_TEXT;
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}\n[truncated]", &text[..end])
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TimeoutConfig {
    pub default_timeout_ms: u64,
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

#[derive(Clone, Debug)]
pub struct ToolCall {
    pub tool_id: String,
    pub name: String,
    pub input: Value,
    pub timeout_ms: Option<u64>,
}
impl ToolCall {
    pub fn new(id: impl Into<String>, name: impl Into<String>, input: Value) -> Self {
        Self {
            tool_id: id.into(),
            name: name.into(),
            input,
            timeout_ms: None,
        }
    }
    pub fn with_timeout(mut self, ms: u64) -> Self {
        self.timeout_ms = Some(ms);
        self
    }
}

#[derive(Debug)]
pub struct ToolResult {
    pub tool_id: String,
    pub output: String,
    pub success: bool,
    pub duration_ms: u64,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct ToolExecutor {
    timeout_config: TimeoutConfig,
    cwd: PathBuf,
    broker: Option<PermissionBroker>,
    cancellation: Option<tokio::sync::watch::Receiver<bool>>,
}
impl Default for ToolExecutor {
    fn default() -> Self {
        Self::new()
    }
}
impl ToolExecutor {
    pub fn new() -> Self {
        Self {
            timeout_config: TimeoutConfig::default(),
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            broker: None,
            cancellation: None,
        }
    }
    pub fn with_timeout_config(config: TimeoutConfig) -> Self {
        Self {
            timeout_config: config,
            ..Self::new()
        }
    }
    pub fn with_timeout(mut self, c: TimeoutConfig) -> Self {
        self.timeout_config = c;
        self
    }
    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = cwd.into();
        self
    }
    pub fn with_broker(mut self, b: PermissionBroker) -> Self {
        self.broker = Some(b);
        self
    }
    pub fn with_cancellation(mut self, receiver: tokio::sync::watch::Receiver<bool>) -> Self {
        self.cancellation = Some(receiver);
        self
    }
    pub(crate) fn clone_for_dispatch(&self) -> Self {
        self.clone()
    }
    fn call_timeout(&self, call: &ToolCall) -> Duration {
        Duration::from_millis(
            call.timeout_ms
                .or_else(|| call.input.get("timeout").and_then(Value::as_u64))
                .unwrap_or(self.timeout_config.default_timeout_ms)
                .clamp(1, self.timeout_config.max_timeout_ms.max(1)),
        )
    }
    fn authorize(&self, intent: OperationIntent) -> Result<(), String> {
        if let Some(b) = &self.broker {
            match b.authorize(&intent) {
                Decision::Allow => Ok(()),
                Decision::Deny { reason } | Decision::RequireHuman { reason, .. } => Err(reason),
            }
        } else {
            Ok(())
        }
    }
    fn write_content(
        &self,
        call: &ToolCall,
        path: &Path,
    ) -> Result<(String, Option<Value>), String> {
        if call.name == "write" {
            let content = call
                .input
                .get("content")
                .and_then(Value::as_str)
                .ok_or("content is required")?;
            if content.len() > MAX_TEXT {
                return Err("content exceeds limit".to_owned());
            }
            return Ok((content.to_owned(), None));
        }
        let old = call
            .input
            .get("oldString")
            .and_then(Value::as_str)
            .ok_or("oldString is required")?;
        let new = call
            .input
            .get("newString")
            .and_then(Value::as_str)
            .ok_or("newString is required")?;
        if old.is_empty() || old == new {
            return Err("oldString must be nonempty and differ from newString".to_owned());
        }
        if old.len() > MAX_TEXT || new.len() > MAX_TEXT {
            return Err("edit arguments exceed limit".to_owned());
        }
        self.authorize(OperationIntent::File {
            action: FileAction::Read,
            path: path.to_owned(),
        })?;
        let result = FileTool::with_project_root(&self.cwd)
            .execute(FileOperation::Read {
                path: path.to_owned(),
                offset: None,
                limit: None,
            })
            .map_err(|error| error.to_string())?;
        if !result.success {
            return Err(result
                .error
                .unwrap_or_else(|| "edit source unavailable".to_owned()));
        }
        let source = result.content;
        if source.len() > MAX_TEXT {
            return Err("edit source exceeds limit".to_owned());
        }
        let count = source.matches(old).count();
        let replace_all = call
            .input
            .get("replaceAll")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if count == 0 || (count != 1 && !replace_all) {
            return Err("edit match count is not exactly one".to_owned());
        }
        let size = source
            .len()
            .checked_sub(
                old.len()
                    .saturating_mul(if replace_all { count } else { 1 }),
            )
            .and_then(|size| {
                size.checked_add(
                    new.len()
                        .saturating_mul(if replace_all { count } else { 1 }),
                )
            })
            .ok_or("edited content exceeds limit")?;
        if size > MAX_TEXT {
            return Err("edited content exceeds limit".to_owned());
        }
        let content = if replace_all {
            source.replace(old, new)
        } else {
            source.replacen(old, new, 1)
        };
        let mut diff = format!(
            "--- {}\n+++ {}\n@@ -1,{} +1,{} @@\n",
            path.display(),
            path.display(),
            source.lines().count(),
            content.lines().count()
        );
        let mut truncated = false;
        for (prefix, text) in [("-", source.as_str()), ("+", content.as_str())] {
            for line in text.lines() {
                if diff.len().saturating_add(line.len() + 2) > 64 * 1024 {
                    truncated = true;
                    break;
                }
                diff.push_str(prefix);
                diff.push_str(line);
                diff.push('\n');
            }
            if truncated {
                break;
            }
            if !text.is_empty() && !text.ends_with('\n') {
                diff.push_str("\\ No newline at end of file\n");
            }
        }
        let metadata = serde_json::json!({"format":"opencode.file-edit.v1","path":path,"diff":diff,"diff_truncated":truncated});
        Ok((content, Some(metadata)))
    }
    /// Build the exact effect intent without touching the filesystem.
    pub fn intent(&self, call: &ToolCall) -> Result<OperationIntent, String> {
        match call.name.as_str() {
            "bash" | "shell" => {
                let command = call
                    .input
                    .get("command")
                    .and_then(Value::as_str)
                    .filter(|v| !v.is_empty() && v.len() <= 32 * 1024 && !v.contains('\0'))
                    .ok_or("command is required")?;
                let cwd = call
                    .input
                    .get("workdir")
                    .and_then(Value::as_str)
                    .map(PathBuf::from)
                    .unwrap_or_else(|| self.cwd.clone());
                let cwd = if cwd.is_absolute() {
                    cwd
                } else {
                    self.cwd.join(cwd)
                };
                if call.input.get("background").and_then(Value::as_bool) == Some(true) {
                    return Err("background execution is not supported".to_owned());
                }
                Ok(OperationIntent::Process {
                    program: "bash".into(),
                    args: vec!["-c".into(), command.into()],
                    cwd,
                })
            }
            "read" | "grep" | "glob" => {
                let path = call
                    .input
                    .get("path")
                    .and_then(Value::as_str)
                    .map(PathBuf::from)
                    .unwrap_or_else(|| self.cwd.clone());
                Ok(OperationIntent::File {
                    action: FileAction::Read,
                    path: if path.is_absolute() {
                        path
                    } else {
                        self.cwd.join(path)
                    },
                })
            }
            "write" | "edit" => {
                let path = call
                    .input
                    .get("path")
                    .and_then(Value::as_str)
                    .filter(|v| !v.is_empty())
                    .ok_or("path is required")?;
                let path = PathBuf::from(path);
                Ok(OperationIntent::File {
                    action: FileAction::Write,
                    path: if path.is_absolute() {
                        path
                    } else {
                        self.cwd.join(path)
                    },
                })
            }
            "websearch" | "webfetch" => crate::native_web::intent(&call.name, &call.input),
            other => Err(format!("unsupported tool: {other}")),
        }
    }

    /// Execute after a trusted live approval. The proof is consumed by the
    /// broker against the freshly reconstructed exact intent before effects.
    pub async fn execute_human_approved(
        &self,
        call: ToolCall,
        approval: HumanApproval,
        session_id: &str,
        run_id: u64,
        call_id: &str,
    ) -> ToolResult {
        let start = Instant::now();
        let Some(broker) = &self.broker else {
            return fail(&call, start, "tool broker is required");
        };
        let intent = match self.intent(&call) {
            Ok(v) => v,
            Err(e) => return fail(&call, start, e),
        };
        match call.name.as_str() {
            "bash" | "shell" => {
                self.shell_approved(&call, start, approval, session_id, run_id, call_id)
                    .await
            }
            "read" | "write" | "edit" => {
                let executor = self.clone();
                let session_id = session_id.to_owned();
                let call_id = call_id.to_owned();
                let retained_call = call.clone();
                match tokio::task::spawn_blocking(move || {
                    executor
                        .execute_file_operation(call, Some((approval, session_id, run_id, call_id)))
                })
                .await
                {
                    Ok(result) => result,
                    Err(error) => fail(&retained_call, start, error.to_string()),
                }
            }
            "grep" | "glob" => {
                match broker
                    .authorize_with_human_approval(&intent, approval, session_id, run_id, call_id)
                {
                    Decision::Allow => {
                        if call.name == "grep" {
                            self.grep_unchecked(&call, start).await
                        } else {
                            self.glob_unchecked(&call, start).await
                        }
                    }
                    Decision::Deny { reason } | Decision::RequireHuman { reason, .. } => {
                        fail(&call, start, reason)
                    }
                }
            }
            "websearch" | "webfetch" => {
                match broker.authorize_with_human_approval(&intent, approval, session_id, run_id, call_id) {
                    Decision::Allow => self.web_unchecked(&call, start).await,
                    Decision::Deny { reason } | Decision::RequireHuman { reason, .. } => fail(&call, start, reason),
                }
            }
            _ => fail(&call, start, "unsupported approved tool"),
        }
    }
    async fn shell_approved(
        &self,
        call: &ToolCall,
        start: Instant,
        approval: HumanApproval,
        session_id: &str,
        run_id: u64,
        call_id: &str,
    ) -> ToolResult {
        let Some(command) = call.input.get("command").and_then(Value::as_str) else {
            return fail(call, start, "command is required");
        };
        let cwd = match self.intent(call) {
            Ok(OperationIntent::Process { cwd, .. }) => cwd,
            Ok(_) => unreachable!(),
            Err(error) => return fail(call, start, error),
        };
        let Some(b) = &self.broker else {
            return fail(call, start, "tool broker is required");
        };
        let mut tool = ShellTool::new("bash", vec!["-c".into(), command.into()])
            .cwd(cwd.to_string_lossy())
            .broker(b.clone())
            .deadline(self.call_timeout(call))
            .human_approved(approval, session_id, run_id, call_id);
        if let Some(receiver) = self.cancellation.clone() {
            tool = tool.cancellation(receiver);
        }
        match tool
            .execute(ShellConfig {
                timeout_secs: self.call_timeout(call).as_millis().div_ceil(1000).max(1) as u64,
                max_output_mb: 1,
                allowed_commands: vec!["bash".into()],
            })
            .await
        {
            Ok(r) if r.success => ToolResult {
                tool_id: call.tool_id.clone(),
                output: cap(r.stdout),
                success: true,
                duration_ms: elapsed(start),
                error: None,
            },
            Ok(r) => fail(call, start, r.stderr),
            Err(e) => fail(call, start, e.to_string()),
        }
    }
    pub fn execute_file_operation(
        &self,
        call: ToolCall,
        approval: Option<(HumanApproval, String, u64, String)>,
    ) -> ToolResult {
        if !matches!(call.name.as_str(), "read" | "write" | "edit") {
            return fail(&call, Instant::now(), "not a file operation");
        }
        match approval {
            Some((approval, session, run, call_id)) => {
                self.file_approved(&call, Instant::now(), approval, &session, run, &call_id)
            }
            None => self.file(&call),
        }
    }
    fn file_approved(
        &self,
        call: &ToolCall,
        start: Instant,
        approval: HumanApproval,
        session_id: &str,
        run_id: u64,
        call_id: &str,
    ) -> ToolResult {
        let Some(b) = &self.broker else {
            return fail(call, start, "tool broker is required");
        };
        let path = match self.intent(call) {
            Ok(OperationIntent::File { path, .. }) => path,
            Ok(_) => return fail(call, start, "invalid file intent"),
            Err(e) => return fail(call, start, e),
        };
        if call.name == "read" {
            match b.authorize_with_human_approval(
                &OperationIntent::File {
                    action: FileAction::Read,
                    path: path.clone(),
                },
                approval,
                session_id,
                run_id,
                call_id,
            ) {
                Decision::Allow => {}
                Decision::Deny { reason } | Decision::RequireHuman { reason, .. } => {
                    return fail(call, start, reason);
                }
            };
            let op = FileOperation::Read {
                path,
                offset: call
                    .input
                    .get("offset")
                    .and_then(Value::as_u64)
                    .map(|v| v as usize),
                limit: call
                    .input
                    .get("limit")
                    .and_then(Value::as_u64)
                    .map(|v| v.min(MAX_TEXT as u64) as usize),
            };
            return match FileTool::with_project_root(&self.cwd).execute(op) {
                Ok(r) if r.success => ToolResult {
                    tool_id: call.tool_id.clone(),
                    output: cap(r.content),
                    success: true,
                    duration_ms: elapsed(start),
                    error: None,
                },
                Ok(r) => fail(
                    call,
                    start,
                    r.error.unwrap_or_else(|| "file operation failed".into()),
                ),
                Err(e) => fail(call, start, e.to_string()),
            };
        }
        let (content, mut metadata) = match self.write_content(call, &path) {
            Ok(content) => content,
            Err(error) => return fail(call, start, error),
        };
        if content.len() > MAX_TEXT {
            return fail(call, start, "content exceeds limit");
        };
        match FileTool::with_project_root(&self.cwd).execute_human_approved(
            FileOperation::Write {
                path,
                content,
                append: false,
            },
            b,
            approval,
            session_id,
            run_id,
            call_id,
        ) {
            Ok(r) if r.success => ToolResult {
                tool_id: call.tool_id.clone(),
                output: match metadata.as_mut() {
                    Some(metadata) => {
                        metadata["output"] = Value::String(r.content);
                        metadata.to_string()
                    }
                    None => cap(r.content),
                },
                success: true,
                duration_ms: elapsed(start),
                error: None,
            },
            Ok(r) => fail(
                call,
                start,
                r.error.unwrap_or_else(|| "file operation failed".into()),
            ),
            Err(e) => fail(call, start, e.to_string()),
        }
    }
    async fn grep_unchecked(&self, call: &ToolCall, _start: Instant) -> ToolResult {
        self.search(call).await
    }
    async fn glob_unchecked(&self, call: &ToolCall, _start: Instant) -> ToolResult {
        self.search(call).await
    }
    pub async fn execute(&self, call: ToolCall) -> ToolResult {
        let start = Instant::now();
        let d = self.call_timeout(&call);
        match call.name.as_str() {
            "bash" | "shell" => self.shell(&call, d).await,
            "read" | "write" | "edit" => {
                let executor = self.clone();
                let retained_call = call.clone();
                match tokio::task::spawn_blocking(move || {
                    executor.execute_file_operation(call, None)
                })
                .await
                {
                    Ok(result) => result,
                    Err(error) => fail(&retained_call, start, error.to_string()),
                }
            }
            "grep" | "glob" => self.search(&call).await,
            "websearch" | "webfetch" => {
                match self.intent(&call).and_then(|intent| self.authorize(intent)) {
                    Ok(()) => self.web_unchecked(&call, start).await,
                    Err(error) => fail(&call, start, error),
                }
            }
            "echo" => self.echo(&call, d).await,
            _ => fail(&call, start, format!("Unknown tool: {}", call.name)),
        }
    }
    async fn web_unchecked(&self, call: &ToolCall, start: Instant) -> ToolResult {
        let request = crate::native_web::execute(&call.name, &call.input);
        let mut cancellation = self.cancellation.clone();
        let result = if let Some(cancelled) = cancellation.as_mut() {
            if *cancelled.borrow() { return fail(call, start, "web request cancelled"); }
            tokio::select! {
                biased;
                _ = cancelled.changed() => return fail(call, start, "web request cancelled"),
                result = request => result,
            }
        } else { request.await };
        match result {
            Ok(output) => ToolResult { tool_id: call.tool_id.clone(), output, success: true, duration_ms: elapsed(start), error: None },
            Err(error) => fail(call, start, error),
        }
    }
    async fn echo(&self, call: &ToolCall, d: Duration) -> ToolResult {
        let start = Instant::now();
        let msg = call
            .input
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("");
        match timeout(d, tokio::process::Command::new("echo").arg(msg).output()).await {
            Ok(Ok(out)) => ToolResult {
                tool_id: call.tool_id.clone(),
                output: String::from_utf8_lossy(&out.stdout).into_owned(),
                success: out.status.success(),
                duration_ms: elapsed(start),
                error: None,
            },
            Ok(Err(e)) => fail(call, start, e.to_string()),
            Err(_) => fail(call, start, "Execution timed out"),
        }
    }
    async fn shell(&self, call: &ToolCall, d: Duration) -> ToolResult {
        let start = Instant::now();
        let Some(command) = call.input.get("command").and_then(Value::as_str) else {
            return fail(call, start, "Missing 'command' field");
        };
        let cwd = match self.intent(call) {
            Ok(OperationIntent::Process { cwd, .. }) => cwd,
            Ok(_) => unreachable!(),
            Err(error) => return fail(call, start, error),
        };
        if let Err(e) = self.authorize(OperationIntent::Process {
            program: "bash".into(),
            args: vec!["-c".into(), command.into()],
            cwd: cwd.clone(),
        }) {
            return fail(call, start, e);
        }
        let mut tool =
            ShellTool::new("bash", vec!["-c".into(), command.into()]).cwd(cwd.to_string_lossy());
        if let Some(b) = &self.broker {
            tool = tool.broker(b.clone())
        };
        tool = tool.deadline(d);
        if let Some(receiver) = self.cancellation.clone() {
            tool = tool.cancellation(receiver);
        }
        match tool
            .execute(ShellConfig {
                timeout_secs: d.as_secs().max(1),
                max_output_mb: 1,
                allowed_commands: vec!["bash".into()],
            })
            .await
        {
            Ok(r) if r.success => ToolResult {
                tool_id: call.tool_id.clone(),
                output: cap(r.stdout),
                success: true,
                duration_ms: elapsed(start),
                error: None,
            },
            Ok(r) => fail(call, start, cap(r.stderr)),
            Err(e) => fail(call, start, e.to_string()),
        }
    }
    fn file(&self, call: &ToolCall) -> ToolResult {
        let start = Instant::now();
        let Some(raw) = call.input.get("path").and_then(Value::as_str) else {
            return fail(call, start, "path is required");
        };
        let path = PathBuf::from(raw);
        let path = if path.is_absolute() {
            path
        } else {
            self.cwd.join(path)
        };
        let action = if call.name == "read" {
            FileAction::Read
        } else {
            FileAction::Write
        };
        if let Err(e) = self.authorize(OperationIntent::File {
            action,
            path: path.clone(),
        }) {
            return fail(call, start, e);
        }
        let mut metadata = None;
        let op = match call.name.as_str() {
            "read" => FileOperation::Read {
                path,
                offset: call
                    .input
                    .get("offset")
                    .and_then(Value::as_u64)
                    .map(|v| v as usize),
                limit: call
                    .input
                    .get("limit")
                    .and_then(Value::as_u64)
                    .map(|v| v.min(MAX_TEXT as u64) as usize),
            },
            "write" => FileOperation::Write {
                content: match self.write_content(call, &path) {
                    Ok((content, preview)) => {
                        metadata = preview;
                        content
                    }
                    Err(error) => return fail(call, start, error),
                },
                path,
                append: false,
            },
            "edit" => {
                let content = match self.write_content(call, &path) {
                    Ok((content, preview)) => {
                        metadata = preview;
                        content
                    }
                    Err(error) => return fail(call, start, error),
                };
                FileOperation::Write {
                    path,
                    content,
                    append: false,
                }
            }
            _ => unreachable!(),
        };
        let r = match self.broker.as_ref() {
            Some(b) => FileTool::with_project_root(&self.cwd).execute_authorized(op, b),
            None => return fail(call, start, "tool broker is required"),
        };
        match r {
            Ok(v) if v.success => ToolResult {
                tool_id: call.tool_id.clone(),
                output: match metadata.as_mut() {
                    Some(metadata) => {
                        metadata["output"] = Value::String(v.content);
                        metadata.to_string()
                    }
                    None => cap(v.content),
                },
                success: true,
                duration_ms: elapsed(start),
                error: None,
            },
            Ok(v) => fail(
                call,
                start,
                v.error.unwrap_or_else(|| "file operation failed".into()),
            ),
            Err(e) => fail(call, start, e.to_string()),
        }
    }
    async fn search(&self, call: &ToolCall) -> ToolResult {
        let start = Instant::now();
        let input = call.input.clone();
        let cwd = self.cwd.clone();
        let kind = call.name.clone();
        let Some(broker) = self.broker.clone() else {
            return fail(call, start, "search requires a broker");
        };
        match tokio::task::spawn_blocking(move || {
            crate::native_search::search(&input, &cwd, &broker, &kind)
        })
        .await
        {
            Ok(Ok(output)) => ToolResult {
                tool_id: call.tool_id.clone(),
                output,
                success: true,
                duration_ms: elapsed(start),
                error: None,
            },
            Ok(Err(error)) => fail(call, start, error),
            Err(error) => fail(call, start, error.to_string()),
        }
    }
    pub async fn execute_batch(&self, calls: Vec<ToolCall>) -> Vec<ToolResult> {
        let mut out = Vec::with_capacity(calls.len());
        for c in calls {
            out.push(self.execute(c).await)
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_shell_call(id: &str, cmd: &str) -> ToolCall {
        ToolCall::new(id, "bash", json!({"command":cmd}))
    }
    fn make_echo_call(id: &str, msg: &str) -> ToolCall {
        ToolCall::new(id, "echo", json!({"message":msg}))
    }

    #[tokio::test]
    async fn execute_success() {
        let result = ToolExecutor::new()
            .execute(make_shell_call("test-1", "echo hello"))
            .await;
        assert!(result.success);
        assert!(result.output.contains("hello"));
        assert!(result.error.is_none());
        assert!(result.duration_ms > 0);
    }
    #[tokio::test]
    async fn execute_timeout() {
        let executor = ToolExecutor::with_timeout_config(TimeoutConfig {
            default_timeout_ms: 1,
            max_timeout_ms: 1000,
        });
        let start = Instant::now();
        let result = executor
            .execute(make_shell_call("timeout-test", "sleep 10").with_timeout(1))
            .await;
        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("timed out"));
        assert!(start.elapsed().as_millis() < 2000);
    }
    #[tokio::test]
    async fn execute_failure() {
        let result = ToolExecutor::new()
            .execute(make_shell_call("fail-test", "exit 1"))
            .await;
        assert!(!result.success);
        assert!(result.error.is_some());
    }
    #[tokio::test]
    async fn batch_executes_all() {
        let results = ToolExecutor::new()
            .execute_batch(vec![
                make_echo_call("batch-1", "first"),
                make_echo_call("batch-2", "second"),
                make_echo_call("batch-3", "third"),
            ])
            .await;
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.success));
        assert!(results[0].output.contains("first"));
        assert!(results[1].output.contains("second"));
        assert!(results[2].output.contains("third"));
    }
    #[tokio::test]
    async fn result_has_duration() {
        let result = ToolResult {
            tool_id: "duration-test".into(),
            output: "test output".into(),
            success: true,
            duration_ms: 123,
            error: None,
        };
        assert_eq!(result.duration_ms, 123);
        assert!(result.success);
    }
}
