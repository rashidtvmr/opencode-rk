// LANE-MCP-LIVE: typed MCP client session state machine tests.
// Standalone harness: #[path] includes the module under test.
// No process spawning; drives against in-memory framed transport.

#[path = "../src/mcp_session.rs"]
mod mcp_session;

use mcp_session::{ClientState, Error, JsonRpcFrame, Session};
use serde_json::{Value, json};

/// Helper: build a JSON-RPC response frame.
fn rpc_response(id: u64, result: Value) -> JsonRpcFrame {
    JsonRpcFrame {
        jsonrpc: "2.0".into(),
        id: Some(id),
        method: None,
        params: None,
        result: Some(result),
        error: None,
    }
}

/// Helper: build a JSON-RPC error frame.
fn rpc_error(id: u64, code: i32, message: &str) -> JsonRpcFrame {
    JsonRpcFrame {
        jsonrpc: "2.0".into(),
        id: Some(id),
        method: None,
        params: None,
        result: None,
        error: Some(serde_json::json!({
            "code": code,
            "message": message,
        })),
    }
}

/// Helper: build a JSON-RPC notification frame.
fn rpc_notification(method: &str, params: Value) -> JsonRpcFrame {
    JsonRpcFrame {
        jsonrpc: "2.0".into(),
        id: None,
        method: Some(method.into()),
        params: Some(params),
        result: None,
        error: None,
    }
}

// ─── T01: State starts uninitialized; init handshake completes to Ready ────

#[test]
fn t01_initial_state_is_uninitialized() {
    let session = Session::new(8, 10);
    assert_eq!(session.state(), ClientState::Uninitialized);
}

#[test]
fn t02_init_handshake_transitions_to_ready() {
    let mut session = Session::new(8, 10);
    let frame = session.begin_init().expect("begin_init ok");
    assert_eq!(frame.method.as_deref(), Some("initialize"));
    assert_eq!(session.state(), ClientState::Initializing);
    let response = rpc_response(frame.id.unwrap(), json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {"tools": {"listChanged": true}},
        "serverInfo": {"name": "test-server", "version": "1.0"}
    }));
    session.receive(response).expect("handshake ok");
    assert_eq!(session.state(), ClientState::Ready);
}

#[test]
fn t03_init_error_transitions_to_error() {
    let mut session = Session::new(8, 10);
    let frame = session.begin_init().expect("begin_init ok");
    let response = rpc_error(frame.id.unwrap(), -32600, "invalid request");
    session.receive(response).expect("receive ok");
    assert_eq!(session.state(), ClientState::Error);
    let err = session.last_error().expect("has error");
    assert!(matches!(err, Error::Protocol(_)));
}

// ─── T04: ID correlation — sequential requests ─────────────────────────────

#[test]
fn t04_sequential_request_correlation() {
    let mut session = Session::new(8, 10);
    let init = session.begin_init().expect("init");
    let r = rpc_response(init.id.unwrap(), json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "serverInfo": {"name": "s", "version": "1"}
    }));
    session.receive(r).expect("init ok");
    assert_eq!(session.state(), ClientState::Ready);

    let list_frame = session.list_tools().expect("list_tools");
    let resp = rpc_response(list_frame.id.unwrap(), json!({
        "tools": []
    }));
    session.receive(resp).expect("list response ok");
    assert_eq!(session.tools().len(), 0);
}

// ─── T05: ID correlation — interleaved requests ─────────────────────────────

#[test]
fn t05_interleaved_request_correlation() {
    let mut session = Session::new(8, 10);
    let init = session.begin_init().expect("init");
    session
        .receive(rpc_response(
            init.id.unwrap(),
            json!({"protocolVersion":"2024-11-05","capabilities":{},"serverInfo":{"name":"s","version":"1"}}),
        ))
        .expect("init ok");

    let list_frame = session.list_tools().expect("list");
    let call_frame = session.call_tool("foo", json!({"x": 1})).expect("call");
    assert_ne!(list_frame.id, call_frame.id);

    let call_resp = rpc_response(call_frame.id.unwrap(), json!({"content":[{"type":"text","text":"ok"}]}));
    let list_resp = rpc_response(list_frame.id.unwrap(), json!({"tools":[{"name":"foo","description":"d","inputSchema":{}}]}));
    session.receive(call_resp).expect("call resp");
    session.receive(list_resp).expect("list resp");

    assert_eq!(session.tools().len(), 1);
}

// ─── T06: Timeout on tool call ──────────────────────────────────────────────

#[tokio::test]
async fn t06_tool_call_timeout() {
    let mut session = Session::new(8, 10);
    let init = session.begin_init().expect("init");
    session
        .receive(rpc_response(
            init.id.unwrap(),
            json!({"protocolVersion":"2024-11-05","capabilities":{},"serverInfo":{"name":"s","version":"1"}}),
        ))
        .expect("init ok");

    let frame = session.call_tool("foo", json!({})).expect("call");
    let result = session.wait_result(frame.id.unwrap(), std::time::Duration::from_millis(10)).await;
    assert!(matches!(result, Err(Error::Timeout)));
}

// ─── T07: Cancel clears pending calls ───────────────────────────────────────

#[tokio::test]
async fn t07_cancel_pending_call() {
    let mut session = Session::new(8, 10);
    let init = session.begin_init().expect("init");
    session
        .receive(rpc_response(
            init.id.unwrap(),
            json!({"protocolVersion":"2024-11-05","capabilities":{},"serverInfo":{"name":"s","version":"1"}}),
        ))
        .expect("init ok");

    let frame = session.call_tool("bar", json!({})).expect("call");
    session.cancel_pending();
    let result = session.wait_result(frame.id.unwrap(), std::time::Duration::from_secs(5)).await;
    assert!(matches!(result, Err(Error::Cancelled)));
}

// ─── T08: Tool list cache with cap eviction ─────────────────────────────────

#[test]
fn t08_tool_list_cache_eviction() {
    let mut session = Session::new(8, 2); // cap = 2
    let init = session.begin_init().expect("init");
    session
        .receive(rpc_response(
            init.id.unwrap(),
            json!({"protocolVersion":"2024-11-05","capabilities":{},"serverInfo":{"name":"s","version":"1"}}),
        ))
        .expect("init ok");

    let f1 = session.list_tools().expect("list1");
    let tools: Vec<Value> = (0..5)
        .map(|i| json!({"name": format!("t{i}"), "description": "d", "inputSchema": {}}))
        .collect();
    session.receive(rpc_response(f1.id.unwrap(), json!({"tools": tools}))).expect("ok");
    assert_eq!(session.tools().len(), 2, "should evict to cap");
}

// ─── T09: Server exited error ───────────────────────────────────────────────

#[test]
fn t09_server_exited_error() {
    let mut session = Session::new(8, 10);
    let init = session.begin_init().expect("init");
    session
        .receive(rpc_response(
            init.id.unwrap(),
            json!({"protocolVersion":"2024-11-05","capabilities":{},"serverInfo":{"name":"s","version":"1"}}),
        ))
        .expect("init ok");

    session.mark_server_exited();
    assert_eq!(session.state(), ClientState::Error);
    assert!(matches!(session.last_error(), Some(Error::ServerExited)));
}

// ─── T10: Spawn failed error (from broker composition) ──────────────────────

#[test]
fn t10_spawn_failed_error() {
    let mut session = Session::new(8, 10);
    session.record_error(Error::SpawnFailed("not found".into()));
    assert_eq!(session.state(), ClientState::Error);
    assert!(matches!(session.last_error(), Some(Error::SpawnFailed(_))));
}

// ─── T11: Timeout error ─────────────────────────────────────────────────────

#[tokio::test]
async fn t11_init_timeout() {
    let mut session = Session::new(8, 10);
    let frame = session.begin_init().expect("init");
    let result = session.wait_result(frame.id.unwrap(), std::time::Duration::from_millis(10)).await;
    assert!(matches!(result, Err(Error::Timeout)));
}

// ─── T12: Cancel notification from server ────────────────────────────────────

#[test]
fn t12_server_cancel_notification() {
    let mut session = Session::new(8, 10);
    let init = session.begin_init().expect("init");
    session
        .receive(rpc_response(
            init.id.unwrap(),
            json!({"protocolVersion":"2024-11-05","capabilities":{},"serverInfo":{"name":"s","version":"1"}}),
        ))
        .expect("init ok");

    let notif = rpc_notification("notifications/cancelled", json!({"requestId": 999}));
    session.receive(notif).expect("notification ok");
    // Should remain in Ready — cancel notification is advisory, not fatal.
    assert_eq!(session.state(), ClientState::Ready);
}

// ─── T13: Rejected state transitions ────────────────────────────────────────

#[test]
fn t13_list_tools_before_init_rejected() {
    let mut session = Session::new(8, 10);
    let result = session.list_tools();
    assert!(matches!(result, Err(Error::Protocol(_))));
}

#[test]
fn t14_call_tool_before_init_rejected() {
    let mut session = Session::new(8, 10);
    let result = session.call_tool("x", json!({}));
    assert!(matches!(result, Err(Error::Protocol(_))));
}

// ─── T15: Drain outbound frames ─────────────────────────────────────────────

#[test]
fn t15_drain_outbound() {
    let mut session = Session::new(8, 10);
    let init = session.begin_init().expect("init");
    let frames = session.drain_outbound();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].method.as_deref(), Some("initialize"));
}
