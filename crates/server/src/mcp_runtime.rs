//! Real bounded local MCP stdio lifecycle.
//!
//! This module owns one argv-direct child per configured server.  It performs
//! the MCP initialize/tools/list handshake before exposing `connected` and
//! keeps the discovered schemas private to the daemon.  The HTTP projection
//! may expose bounded tool metadata, but never command arguments or env.
#![forbid(unsafe_code)]

use opencode_rk_security::{Decision, OperationIntent, PermissionBroker, SecurityPolicy};
use serde_json::{Value, json};
use std::{collections::HashMap, fmt, path::PathBuf, process::Stdio, sync::{Arc, atomic::{AtomicBool, Ordering}}};
use tokio::{io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader}, process::{Child, ChildStdin, ChildStdout, Command}, sync::{Mutex, Semaphore}, time::{Duration, timeout}};

pub const MAX_FRAME_BYTES: usize = 64 * 1024;
pub const MAX_TOOLS: usize = 128;
pub const IO_TIMEOUT: Duration = Duration::from_secs(5);
pub const MAX_NOTIFICATIONS_SKIPPED: usize = 32;
pub const MAX_TOOL_NAME_CHARS: usize = 128;
/// Cap on captured, still-unanswered server→client requests.
pub const MAX_PENDING_REQUESTS: usize = 8;

#[derive(Clone, Debug)]
pub struct LocalConfig {
    pub command: String,
    pub args: Vec<String>,
    pub environment: HashMap<String, String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DiscoveredTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// One server→client JSON-RPC request captured while waiting for an
/// outbound response (e.g. an MCP `elicitation/create` ask). Keeping the
/// id and params bounded makes the request answerable instead of fatal.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingRequest {
    pub id: u64,
    pub method: String,
    pub params: Value,
}

#[derive(Debug)]
pub enum RuntimeError {
    Denied(String),
    Process(String),
    Protocol(String),
    Timeout,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Denied(message) => write!(f, "MCP process denied: {message}"),
            Self::Process(message) => write!(f, "MCP process failed: {message}"),
            Self::Protocol(message) => write!(f, "MCP protocol failed: {message}"),
            Self::Timeout => f.write_str("MCP operation timed out"),
        }
    }
}

impl std::error::Error for RuntimeError {}

struct Connection {
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    stdout: Mutex<BufReader<ChildStdout>>,
    serial: Semaphore,
    next_id: Mutex<u64>,
    pending: Mutex<Vec<PendingRequest>>,
    active: AtomicBool,
}

impl Drop for Connection {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.try_lock() {
            let _ = child.start_kill();
        }
    }
}

#[derive(Clone)]
pub struct McpRuntime {
    connection: Arc<Connection>,
    tools: Arc<Vec<DiscoveredTool>>,
}

impl McpRuntime {
    pub async fn connect(config: LocalConfig, cwd: PathBuf) -> Result<Self, RuntimeError> {
        if config.command.is_empty() || config.args.len() > 32 {
            return Err(RuntimeError::Protocol("invalid command bounds".into()));
        }
        let broker = PermissionBroker::new(SecurityPolicy::lean_default(cwd.clone()));
        let intent = OperationIntent::Process { program: config.command.clone(), args: config.args.clone(), cwd: cwd.clone() };
        match broker.authorize(&intent) {
            Decision::Allow => {}
            Decision::Deny { reason } | Decision::RequireHuman { reason, .. } => return Err(RuntimeError::Denied(reason)),
        }
        let mut command = Command::new(&config.command);
        command.args(&config.args).env_clear().envs(config.environment).current_dir(cwd)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true);
        let mut child = command.spawn().map_err(|e| RuntimeError::Process(e.to_string()))?;
        let stdin = child.stdin.take().ok_or_else(|| RuntimeError::Process("missing MCP stdin".into()))?;
        let stdout = child.stdout.take().ok_or_else(|| RuntimeError::Process("missing MCP stdout".into()))?;
        let runtime = Self { connection: Arc::new(Connection { child: Mutex::new(child), stdin: Mutex::new(stdin), stdout: Mutex::new(BufReader::new(stdout)), serial: Semaphore::new(1), next_id: Mutex::new(0), pending: Mutex::new(Vec::new()), active: AtomicBool::new(true) }), tools: Arc::new(Vec::new()) };
        runtime.handshake().await
    }

    async fn handshake(mut self) -> Result<Self, RuntimeError> {
        let _permit = self.connection.serial.acquire().await.map_err(|_| RuntimeError::Process("connection closed".into()))?;
        let init_id = self.id().await;
        let init = self.request(json!({"jsonrpc":"2.0","id":init_id,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"opencode-rk","version":"0.1.0"}}}), init_id).await?;
        let init_result = init.get("result").ok_or_else(|| RuntimeError::Protocol("initialize rejected".into()))?;
        if init_result.get("protocolVersion").and_then(Value::as_str).is_none() || init_result.get("capabilities").and_then(Value::as_object).is_none() { return Err(RuntimeError::Protocol("invalid initialize result".into())); }
        self.notify(json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}})).await?;
        let mut cursor = None;
        let mut rows_all = Vec::new();
        loop {
            let list_id = self.id().await;
            let mut params = json!({});
            if let Some(cursor) = cursor.take() { params["cursor"] = Value::String(cursor); }
            let listed = self.request(json!({"jsonrpc":"2.0","id":list_id,"method":"tools/list","params":params}), list_id).await?;
            let rows = listed.get("result").and_then(|v| v.get("tools")).and_then(Value::as_array).ok_or_else(|| RuntimeError::Protocol("tools/list missing tools".into()))?;
            rows_all.extend(rows.iter().cloned());
            if rows_all.len() > MAX_TOOLS { return Err(RuntimeError::Protocol("too many MCP tools".into())); }
            cursor = listed.get("result").and_then(|v| v.get("nextCursor")).and_then(Value::as_str).map(str::to_owned);
            if cursor.is_none() { break; }
        }
        let tools = rows_all.iter().map(|row| {
            let name = row.get("name").and_then(Value::as_str).filter(|v| !v.is_empty()).ok_or_else(|| RuntimeError::Protocol("tool name missing".into()))?;
            if name.chars().count() > MAX_TOOL_NAME_CHARS { return Err(RuntimeError::Protocol("tool name too long".into())); }
            let schema = row.get("inputSchema").cloned().ok_or_else(|| RuntimeError::Protocol("tool schema missing".into()))?;
            if !schema.is_object() { return Err(RuntimeError::Protocol("tool schema must be an object".into())); }
            Ok(DiscoveredTool { name: name.to_owned(), description: row.get("description").and_then(Value::as_str).unwrap_or("").chars().take(2048).collect(), input_schema: schema })
        }).collect::<Result<Vec<_>, RuntimeError>>()?;
        if tools.windows(2).any(|pair| pair[0].name == pair[1].name) || { let mut names = tools.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(); names.sort_unstable(); names.windows(2).any(|p| p[0] == p[1]) } { return Err(RuntimeError::Protocol("duplicate MCP tool name".into())); }
        self.tools = Arc::new(tools);
        drop(_permit);
        Ok(self)
    }

    async fn id(&self) -> u64 { let mut id = self.connection.next_id.lock().await; *id = id.saturating_add(1); *id }
    async fn notify(&self, value: Value) -> Result<(), RuntimeError> { timeout(IO_TIMEOUT, self.write(value)).await.map_err(|_| RuntimeError::Timeout)??; Ok(()) }
    async fn request(&self, value: Value, expected_id: u64) -> Result<Value, RuntimeError> { timeout(IO_TIMEOUT, async { self.write(value).await?; self.read_response(expected_id).await }).await.map_err(|_| RuntimeError::Timeout)? }
    async fn write(&self, value: Value) -> Result<(), RuntimeError> {
        if !self.connection.active.load(Ordering::Acquire) { return Err(RuntimeError::Process("MCP connection is closed".into())); }
        let body = serde_json::to_vec(&value).map_err(|e| RuntimeError::Protocol(e.to_string()))?;
        if body.len() > MAX_FRAME_BYTES { return Err(RuntimeError::Protocol("frame too large".into())); }
        let mut stdin = self.connection.stdin.lock().await;
        // MCP stdio uses newline-delimited JSON. Content-Length is accepted
        // only on input for compatibility with older local fixtures.
        stdin.write_all(&body).await.map_err(|e| RuntimeError::Process(e.to_string()))?;
        stdin.write_all(b"\n").await.map_err(|e| RuntimeError::Process(e.to_string()))?;
        stdin.flush().await.map_err(|e| RuntimeError::Process(e.to_string()))
    }
    async fn read_response(&self, expected_id: u64) -> Result<Value, RuntimeError> {
        let mut stdout = self.connection.stdout.lock().await;
        for _ in 0..MAX_NOTIFICATIONS_SKIPPED {
            let value = read_frame(&mut *stdout).await?;
            match route_frame(&value, expected_id)? {
                FrameRoute::Expected => return Ok(value),
                FrameRoute::Notification => {}
                FrameRoute::ServerRequest(request) => {
                    let mut pending = self.connection.pending.lock().await;
                    queue_request(&mut pending, request)?;
                }
            }
        }
        Err(RuntimeError::Protocol("too many MCP notifications".into()))
    }
    pub fn tools(&self) -> &[DiscoveredTool] { &self.tools }
    /// Snapshot of captured, still-unanswered server→client requests so a
    /// form/answer broker can surface them and respond while an outbound
    /// request is in flight.
    pub async fn pending_requests(&self) -> Vec<PendingRequest> { self.connection.pending.lock().await.clone() }
    /// Answer one captured server→client request with a JSON-RPC `result`
    /// (e.g. `{"action":"accept","content":{...}}` for an elicitation ask).
    /// Transport plumbing only: the caller owns the answer decision. Takes
    /// no `serial` permit on purpose — the in-flight outbound request holds
    /// it while the child waits for this answer.
    pub async fn answer_request(&self, id: u64, result: Value) -> Result<(), RuntimeError> {
        self.respond(id, answer_frame(id, result)).await
    }
    /// Answer one captured request with a JSON-RPC error (decline/cancel).
    pub async fn reject_request(&self, id: u64, code: i64, message: &str) -> Result<(), RuntimeError> {
        self.respond(id, reject_frame(id, code, message)).await
    }
    async fn respond(&self, id: u64, frame: Value) -> Result<(), RuntimeError> {
        {
            let mut pending = self.connection.pending.lock().await;
            let at = pending.iter().position(|request| request.id == id).ok_or_else(|| RuntimeError::Protocol("unknown MCP request id".into()))?;
            pending.remove(at);
        }
        timeout(IO_TIMEOUT, self.write(frame)).await.map_err(|_| RuntimeError::Timeout)??;
        Ok(())
    }
    /// Invoke one previously discovered tool through the owned MCP child.
    /// This is only transport plumbing: the caller must perform the normal
    /// server permission-broker decision before calling it.
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<Value, RuntimeError> {
        if !self.connection.active.load(Ordering::Acquire) { return Err(RuntimeError::Process("MCP connection is closed".into())); }
        if !self.tools.iter().any(|tool| tool.name == name) {
            return Err(RuntimeError::Protocol("unknown MCP tool".into()));
        }
        let encoded = serde_json::to_vec(&arguments).map_err(|e| RuntimeError::Protocol(e.to_string()))?;
        if encoded.len() > MAX_FRAME_BYTES { return Err(RuntimeError::Protocol("tool arguments too large".into())); }
        let permit = timeout(IO_TIMEOUT, self.connection.serial.acquire()).await.map_err(|_| RuntimeError::Timeout)?.map_err(|_| RuntimeError::Process("connection closed".into()))?;
        let id = self.id().await;
        let response = self.request(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":arguments}}), id).await?;
        drop(permit);
        let bytes = serde_json::to_vec(&response).map_err(|e| RuntimeError::Protocol(e.to_string()))?;
        if bytes.len() > MAX_FRAME_BYTES { return Err(RuntimeError::Protocol("tool result too large".into())); }
        Ok(response)
    }
    pub async fn disconnect(&self) -> Result<(), RuntimeError> {
        self.connection.active.store(false, Ordering::Release);
        let mut child = self.connection.child.lock().await;
        child.start_kill().map_err(|e| RuntimeError::Process(e.to_string()))?;
        timeout(IO_TIMEOUT, child.wait()).await.map_err(|_| RuntimeError::Timeout)?.map_err(|e| RuntimeError::Process(e.to_string()))?;
        Ok(())
    }
}

/// Routing decision for one inbound frame. A frame with a non-expected id
/// and a `method` is a server→client request: it must be answerable, not
/// fatal. Only genuine foreign responses remain protocol errors.
#[derive(Debug)]
enum FrameRoute {
    Expected,
    Notification,
    ServerRequest(PendingRequest),
}

fn route_frame(value: &Value, expected_id: u64) -> Result<FrameRoute, RuntimeError> {
    if value.get("jsonrpc").and_then(Value::as_str) != Some("2.0") { return Err(RuntimeError::Protocol("invalid jsonrpc version".into())); }
    match value.get("id").and_then(Value::as_u64) {
        Some(id) if id == expected_id => Ok(FrameRoute::Expected),
        Some(id) if value.get("method").is_some() => {
            let method = value.get("method").and_then(Value::as_str).filter(|m| !m.is_empty()).ok_or_else(|| RuntimeError::Protocol("request method missing".into()))?;
            Ok(FrameRoute::ServerRequest(PendingRequest { id, method: method.to_owned(), params: value.get("params").cloned().unwrap_or(Value::Null) }))
        }
        Some(_) => Err(RuntimeError::Protocol("unexpected response id".into())),
        None => Ok(FrameRoute::Notification),
    }
}

fn queue_request(queue: &mut Vec<PendingRequest>, request: PendingRequest) -> Result<(), RuntimeError> {
    if queue.iter().any(|pending| pending.id == request.id) { return Err(RuntimeError::Protocol("duplicate MCP request id".into())); }
    if queue.len() >= MAX_PENDING_REQUESTS { return Err(RuntimeError::Protocol("too many pending MCP requests".into())); }
    queue.push(request);
    Ok(())
}

fn answer_frame(id: u64, result: Value) -> Value { json!({"jsonrpc":"2.0","id":id,"result":result}) }
fn reject_frame(id: u64, code: i64, message: &str) -> Value { json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}}) }

async fn read_frame<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<Value, RuntimeError> {
    let mut line = Vec::new();
    loop {
        let chunk = reader.fill_buf().await.map_err(|e| RuntimeError::Process(e.to_string()))?;
        if chunk.is_empty() { return Err(RuntimeError::Process("MCP exited".into())); }
        let take = chunk.iter().position(|b| *b == b'\n').map_or(chunk.len(), |i| i + 1);
        if line.len().saturating_add(take) > MAX_FRAME_BYTES { return Err(RuntimeError::Protocol("frame header too large".into())); }
        line.extend_from_slice(&chunk[..take]); reader.consume(take);
        if take > 0 && line.last() == Some(&b'\n') { break; }
    }
    let text = String::from_utf8_lossy(&line).trim().to_owned();
    if text.starts_with('{') { return serde_json::from_str(&text).map_err(|e| RuntimeError::Protocol(e.to_string())); }
    let length = text.strip_prefix("Content-Length:").and_then(|v| v.trim().parse::<usize>().ok()).ok_or_else(|| RuntimeError::Protocol("invalid MCP frame".into()))?;
    if length > MAX_FRAME_BYTES { return Err(RuntimeError::Protocol("frame too large".into())); }
    let mut body = vec![0; length]; reader.read_exact(&mut body).await.map_err(|e| RuntimeError::Process(e.to_string()))?;
    serde_json::from_slice(&body).map_err(|e| RuntimeError::Protocol(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_request_frame_is_captured_not_fatal() {
        let frame = json!({"jsonrpc":"2.0","id":42,"method":"elicitation/create","params":{"message":"need input"}});
        match route_frame(&frame, 7).expect("request must route") {
            FrameRoute::ServerRequest(request) => {
                assert_eq!(request.id, 42);
                assert_eq!(request.method, "elicitation/create");
                assert_eq!(request.params, json!({"message":"need input"}));
            }
            other => panic!("expected server request, got {other:?}"),
        }
    }

    #[test]
    fn expected_notification_and_foreign_response_route_as_before() {
        assert!(matches!(route_frame(&json!({"jsonrpc":"2.0","id":7,"result":{}}), 7).unwrap(), FrameRoute::Expected));
        assert!(matches!(route_frame(&json!({"jsonrpc":"2.0","method":"notifications/message","params":{}}), 7).unwrap(), FrameRoute::Notification));
        assert!(route_frame(&json!({"jsonrpc":"2.0","id":8,"result":{}}), 7).is_err());
    }

    #[test]
    fn pending_queue_is_bounded_and_ids_stay_unique() {
        let mut queue = Vec::new();
        for id in 0..MAX_PENDING_REQUESTS as u64 {
            queue_request(&mut queue, PendingRequest { id, method: "elicitation/create".into(), params: Value::Null }).unwrap();
        }
        assert!(queue_request(&mut queue, PendingRequest { id: 99, method: "elicitation/create".into(), params: Value::Null }).is_err());
        queue.clear();
        queue_request(&mut queue, PendingRequest { id: 3, method: "elicitation/create".into(), params: Value::Null }).unwrap();
        assert!(queue_request(&mut queue, PendingRequest { id: 3, method: "elicitation/create".into(), params: Value::Null }).is_err());
    }

    #[test]
    fn answer_and_reject_frames_echo_the_captured_request_id() {
        assert_eq!(answer_frame(42, json!({"action":"accept"})), json!({"jsonrpc":"2.0","id":42,"result":{"action":"accept"}}));
        assert_eq!(reject_frame(42, -32800, "cancelled"), json!({"jsonrpc":"2.0","id":42,"error":{"code":-32800,"message":"cancelled"}}));
    }
}
