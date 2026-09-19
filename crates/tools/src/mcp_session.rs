//! Typed MCP stdio CLIENT SESSION state machine.
//!
//! Protocol-level concerns only: JSON-RPC id correlation, initialize handshake,
//! tool list cache with cap, tool call with timeout + cancellation, bounded
//! error taxonomy. Zero process spawning — the broker (`mcp_spawn.rs`) composes
//! this session over a real stdio transport later.
//!
//! Drive against an in-memory framed transport for testing: callers inject
//! [`JsonRpcFrame`] via [`Session::receive`] and drain outbound frames via
//! [`Session::drain_outbound`].

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tokio::sync::mpsc;

/// JSON-RPC 2.0 frame. Used for both directions (request, response,
/// notification). Serde-only — no raw string parsing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcFrame {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
}

/// Client session state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientState {
    Uninitialized,
    Initializing,
    Ready,
    CallingTool,
    Shutdown,
    Error,
}

/// Tool info cached from the server's `tools/list` response.
#[derive(Debug, Clone)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Bounded error taxonomy for MCP session failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    SpawnFailed(String),
    Protocol(String),
    Timeout,
    ServerExited,
    Cancelled,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::SpawnFailed(m) => write!(f, "spawn failed: {m}"),
            Error::Protocol(m) => write!(f, "protocol error: {m}"),
            Error::Timeout => write!(f, "timeout"),
            Error::ServerExited => write!(f, "server exited"),
            Error::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::error::Error for Error {}

/// Next ID generator.
struct IdGen(u64);

impl IdGen {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(1);
        self.0
    }
}

/// The MCP client session state machine.
pub struct Session {
    state: ClientState,
    id_gen: IdGen,
    queue_cap: usize,
    tool_cap: usize,
    outbound: Vec<JsonRpcFrame>,
    /// Pending request_id -> oneshot sender for response delivery.
    pending: HashMap<u64, mpsc::Sender<Result<Value, Error>>>,
    /// Response receivers for async wait_result: request_id -> receiver.
    response_rxs: HashMap<u64, mpsc::Receiver<Result<Value, Error>>>,
    /// Request IDs that are tools/list requests (for tool cache update).
    pending_tool_list: HashSet<u64>,
    tool_cache: Vec<ToolInfo>,
    last_error: Option<Error>,
}

impl Session {
    /// Create a new session. `queue_cap` bounds the outbound write queue;
    /// `tool_cap` bounds the tool list cache.
    pub fn new(queue_cap: usize, tool_cap: usize) -> Self {
        Self {
            state: ClientState::Uninitialized,
            id_gen: IdGen(0),
            queue_cap,
            tool_cap,
            outbound: Vec::new(),
            pending: HashMap::new(),
            response_rxs: HashMap::new(),
            pending_tool_list: HashSet::new(),
            tool_cache: Vec::new(),
            last_error: None,
        }
    }

    /// Current session state.
    #[must_use]
    pub fn state(&self) -> ClientState {
        self.state
    }

    /// Last error, if the session is in the `Error` state.
    #[must_use]
    pub fn last_error(&self) -> Option<&Error> {
        self.last_error.as_ref()
    }

    /// Start the MCP initialize handshake. Returns the outbound JSON-RPC
    /// request frame. The session transitions to `Initializing` and waits
    /// for the server's `initialize` response via [`Session::receive`].
    pub fn begin_init(&mut self) -> Result<JsonRpcFrame, Error> {
        if self.state != ClientState::Uninitialized {
            return Err(Error::Protocol(format!(
                "cannot init in state {:?}",
                self.state
            )));
        }
        let id = self.id_gen.next();
        let frame = JsonRpcFrame {
            jsonrpc: "2.0".into(),
            id: Some(id),
            method: Some("initialize".into()),
            params: Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "opencode-rk", "version": "0.1.0"}
            })),
            result: None,
            error: None,
        };
        self.push_outbound(frame.clone());

        // Register a response channel so wait_result(id, ..) works for init.
        let (tx, rx) = mpsc::channel(1);
        self.pending.insert(id, tx);
        self.response_rxs.insert(id, rx);

        self.state = ClientState::Initializing;
        Ok(frame)
    }

    /// Receive an inbound JSON-RPC frame from the server. Dispatches
    /// responses to pending requests by id correlation, handles
    /// notifications, and advances state machine transitions.
    pub fn receive(&mut self, frame: JsonRpcFrame) -> Result<(), Error> {
        // Server cancel notification is advisory — no state change.
        if frame.id.is_none() {
            if let Some(method) = &frame.method {
                if method == "notifications/cancelled" {
                    return Ok(());
                }
            }
            return Ok(());
        }

        let id = frame.id.unwrap();

        // Response to initialize handshake.
        if self.state == ClientState::Initializing {
            if let Some(err) = &frame.error {
                self.state = ClientState::Error;
                let msg = err
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                self.last_error = Some(Error::Protocol(msg.to_owned()));
                // Deliver error to pending waiter.
                if let Some(tx) = self.pending.remove(&id) {
                    let _ = tx.try_send(Err(Error::Protocol(msg.to_owned())));
                }
                return Ok(());
            }
            // Successful init response — transition to Ready.
            if let Some(tx) = self.pending.remove(&id) {
                if let Some(result) = frame.result.clone() {
                    let _ = tx.try_send(Ok(result));
                }
            }
            self.state = ClientState::Ready;
            return Ok(());
        }

        // Correlate response to pending request.
        if let Some(tx) = self.pending.remove(&id) {
            // Update tool cache if this was a tools/list response.
            let is_tool_list = self.pending_tool_list.remove(&id);

            if let Some(err) = &frame.error {
                let msg = err
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let _ = tx.try_send(Err(Error::Protocol(msg.to_owned())));
            } else if let Some(result) = &frame.result {
                if is_tool_list {
                    self.update_tool_cache_from_response(result);
                }
                let _ = tx.try_send(Ok(result.clone()));
            } else {
                let _ = tx.try_send(Err(Error::Protocol("empty response".into())));
            }
            return Ok(());
        }

        // Unknown response id — ignore.
        Ok(())
    }

    /// Send a `tools/list` request. Returns the outbound frame. Must be
    /// called in the `Ready` state.
    pub fn list_tools(&mut self) -> Result<JsonRpcFrame, Error> {
        self.require_ready()?;
        let id = self.id_gen.next();
        let frame = JsonRpcFrame {
            jsonrpc: "2.0".into(),
            id: Some(id),
            method: Some("tools/list".into()),
            params: Some(serde_json::json!({})),
            result: None,
            error: None,
        };
        self.push_outbound(frame.clone());

        let (tx, rx) = mpsc::channel(1);
        self.pending.insert(id, tx);
        self.response_rxs.insert(id, rx);
        self.pending_tool_list.insert(id);

        Ok(frame)
    }

    /// Invoke a tool. Returns the outbound JSON-RPC request frame. Must be
    /// called in the `Ready` state.
    pub fn call_tool(&mut self, name: &str, args: Value) -> Result<JsonRpcFrame, Error> {
        self.require_ready()?;
        let id = self.id_gen.next();
        let frame = JsonRpcFrame {
            jsonrpc: "2.0".into(),
            id: Some(id),
            method: Some("tools/call".into()),
            params: Some(serde_json::json!({
                "name": name,
                "arguments": args,
            })),
            result: None,
            error: None,
        };
        self.push_outbound(frame.clone());

        let (tx, rx) = mpsc::channel(1);
        self.pending.insert(id, tx);
        self.response_rxs.insert(id, rx);

        Ok(frame)
    }

    /// Cached tool list from the server.
    #[must_use]
    pub fn tools(&self) -> &[ToolInfo] {
        &self.tool_cache
    }

    /// Cancel all pending requests. Pending waiters will receive
    /// `Error::Cancelled`.
    pub fn cancel_pending(&mut self) {
        for (_id, tx) in self.pending.drain() {
            let _ = tx.try_send(Err(Error::Cancelled));
        }
        self.pending_tool_list.clear();
        // Do NOT clear response_rxs — wait_result() needs them to detect
        // channel close and return Error::Cancelled.
    }

    /// Mark the server as exited (e.g., child process died).
    pub fn mark_server_exited(&mut self) {
        self.state = ClientState::Error;
        self.last_error = Some(Error::ServerExited);
        self.cancel_pending();
    }

    /// Record an error directly (e.g., spawn failure from the broker).
    pub fn record_error(&mut self, err: Error) {
        self.state = ClientState::Error;
        self.last_error = Some(err);
        self.cancel_pending();
    }

    /// Wait for the response to a specific request id with a timeout.
    /// Returns the result value on success, or a bounded error.
    pub async fn wait_result(
        &mut self,
        request_id: u64,
        timeout: Duration,
    ) -> Result<Value, Error> {
        let mut rx = match self.response_rxs.remove(&request_id) {
            Some(r) => r,
            None => return Err(Error::Protocol(format!("unknown request id {request_id}"))),
        };

        let result = tokio::time::timeout(timeout, rx.recv()).await;

        match result {
            Ok(Some(Ok(val))) => Ok(val),
            Ok(Some(Err(e))) => Err(e),
            Ok(None) => Err(Error::Cancelled),
            Err(_) => {
                // Restore the receiver so it can be cancelled later.
                self.response_rxs.insert(request_id, rx);
                Err(Error::Timeout)
            }
        }
    }

    /// Drain all queued outbound frames. The transport layer (broker) calls
    /// this to write frames to the server's stdin.
    pub fn drain_outbound(&mut self) -> Vec<JsonRpcFrame> {
        std::mem::take(&mut self.outbound)
    }

    // ── private ──

    fn push_outbound(&mut self, frame: JsonRpcFrame) {
        if self.outbound.len() >= self.queue_cap {
            self.outbound.remove(0);
        }
        self.outbound.push(frame);
    }

    fn require_ready(&self) -> Result<(), Error> {
        if self.state != ClientState::Ready {
            return Err(Error::Protocol(format!(
                "session not ready (state: {:?})",
                self.state
            )));
        }
        Ok(())
    }

    /// Update tool cache from a tools/list response, enforcing the cap
    /// with FIFO eviction.
    fn update_tool_cache_from_response(&mut self, result: &Value) {
        let arr = match result.get("tools").and_then(|v| v.as_array()) {
            Some(a) => a,
            None => return,
        };
        let new_tools: Vec<ToolInfo> = arr
            .iter()
            .filter_map(|t| {
                Some(ToolInfo {
                    name: t.get("name")?.as_str()?.to_owned(),
                    description: t
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_owned(),
                    input_schema: t.get("inputSchema").cloned().unwrap_or(Value::Null),
                })
            })
            .collect();
        self.tool_cache.extend(new_tools);
        if self.tool_cache.len() > self.tool_cap {
            self.tool_cache.drain(0..self.tool_cache.len() - self.tool_cap);
        }
    }
}

