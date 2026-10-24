//! Black-box MCP selection inheritance through a real delegated child turn.
//!
//! The parent excludes one MCP server, delegates to a writable built-in child,
//! and the provider captures both request payloads.  This is intentionally a
//! router/provider test rather than a unit test of the MCP projection helper.
#![forbid(unsafe_code)]

use std::{env, sync::{Arc, OnceLock}, time::Duration};

use axum::{body::{to_bytes, Body}, extract::State, http::{Request, StatusCode}, routing::post, Json, Router};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use tokio::sync::{mpsc, Mutex};
use tower::ServiceExt;
use tempfile::TempDir;

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct EnvGuard(Vec<(&'static str, Option<std::ffi::OsString>)>);
impl EnvGuard {
    fn set(values: &[(&'static str, String)]) -> Self {
        let old = values.iter().map(|(key, value)| {
            let previous = env::var_os(key);
            env::set_var(key, value);
            (*key, previous)
        }).collect();
        Self(old)
    }
}
impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in &self.0 {
            match value { Some(value) => env::set_var(key, value), None => env::remove_var(key) }
        }
    }
}

struct AbortOnDrop(tokio::task::JoinHandle<()>);
impl Drop for AbortOnDrop { fn drop(&mut self) { self.0.abort(); } }

fn request(method: &str, path: String, body: Value) -> Request<Body> {
    Request::builder().method(method).uri(path).header("content-type", "application/json")
        .body(Body::from(body.to_string())).unwrap()
}

async fn value(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
}

async fn poll(app: &Router, path: String, predicate: impl Fn(&Value) -> bool) -> Value {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let response = app.clone().oneshot(request("GET", path.clone(), json!({}))).await.unwrap();
            let body = value(response).await;
            if predicate(&body) { return body; }
            tokio::task::yield_now().await;
        }
    }).await.expect("bounded router state poll")
}

fn fixture(dir: &TempDir, server: &str) -> String {
    let path = dir.path().join(format!("mcp-{server}.sh"));
    let tool = format!("{server}_probe");
    let source = format!(r###"#!/bin/sh
set -eu
while IFS= read -r body; do
  id=$(printf '%s' "$body" | sed -n 's/.*"id"[[:space:]]*:[[:space:]]*\([0-9][0-9]*\).*/\1/p')
  case "$body" in
    *'"method":"initialize"'*) out="{{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{{\"tools\":{{}}}},\"serverInfo\":{{\"name\":\"{server}\",\"version\":\"1\"}}}}}}";;
    *'"method":"tools/list"'*) out="{{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{{\"tools\":[{{\"name\":\"{tool}\",\"description\":\"selection probe\",\"inputSchema\":{{\"type\":\"object\"}}}}]}}}}";;
    *) continue;;
  esac
  printf '%s\n' "$out"
done
"###);
    std::fs::write(&path, source).unwrap();
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&path, permissions).unwrap();
    }
    path.to_string_lossy().into_owned()
}

async fn provider(State(seen): State<mpsc::Sender<Value>>, Json(body): Json<Value>) -> ([(&'static str, &'static str); 1], String) {
    let _ = seen.send(body.clone()).await;
    let input = body["input"].as_array().unwrap();
    let last_user = input.iter().rposition(|item| item["role"] == "user").unwrap();
    let user_text = input[last_user]["content"].to_string();
    let continuation = input[last_user + 1..].iter().any(|item| item["type"] == "function_call_output");
    let event = if user_text.contains("CHILD_MCP_SELECTION") {
        json!({"type":"response.output_text.delta","delta":"child completed"})
    } else if !continuation {
        json!({"type":"response.output_item.done","item":{"type":"function_call","id":"fc_task","call_id":"parent_task","name":"task","arguments":json!({"description":"MCP child selection","prompt":"CHILD_MCP_SELECTION: complete the child probe","subagent_type":"build"}).to_string()}})
    } else {
        assert!(input.iter().any(|item| item["type"] == "function_call_output" && item["call_id"] == "parent_task"));
        json!({"type":"response.output_text.delta","delta":"parent completed"})
    };
    ([ ("content-type", "text/event-stream") ], format!("data: {event}\n\ndata: {{\"type\":\"response.completed\",\"response\":{{\"status\":\"completed\"}}}}\n\n"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn child_provider_inherits_parent_mcp_exclusions() {
    let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().await;
    let root = tempfile::tempdir().unwrap();
    let alpha_dir = tempfile::tempdir().unwrap();
    let beta_dir = tempfile::tempdir().unwrap();
    let (seen_tx, mut seen_rx) = mpsc::channel::<Value>(8);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let provider_task = tokio::spawn(async move {
        axum::serve(listener, Router::new().fallback(post(provider)).with_state(seen_tx)).await.unwrap();
    });
    let _provider_guard = AbortOnDrop(provider_task);
    let rules = root.path().join("permission-rules.json");
    std::fs::write(&rules, r#"{"rules":[]}"#).unwrap();
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&rules, std::fs::Permissions::from_mode(0o600)).unwrap(); }
    let _env = EnvGuard::set(&[
        ("OPENAI_BASE_URL", base), ("OPENAI_API_KEY", "mcp-child-selection".into()),
        ("OPENCODE_RK_HOME", root.path().to_string_lossy().into_owned()),
        ("OPENCODE_RK_TURN_TOOLS", "task,mcp".into()),
        ("OPENCODE_RK_PERMISSION_RULES", rules.to_string_lossy().into_owned()),
    ]);
    let sessions = SessionService::new(Arc::new(Storage::open_in_memory(root.path().join("blobs")).unwrap()));
    let app = router(AppState { sessions: sessions.clone(), catalog: Arc::new(Catalog::default()) });
    for (name, dir, server) in [("alpha", &alpha_dir, "alpha"), ("beta", &beta_dir, "beta")] {
        let response = app.clone().oneshot(request("POST", "/api/mcp".into(), json!({"name":name,"config":{"type":"local","command":["/bin/sh",fixture(dir, server)],"environment":{},"enabled":true}}))).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(app.clone().oneshot(request("POST", format!("/api/mcp/{name}/connect"), json!({}))).await.unwrap().status(), StatusCode::OK);
    }
    let parent = sessions.create_in_directory("MCP parent", Some(root.path().into())).await.unwrap();
    // Exclude only alpha. Beta remains available, proving the child payload is
    // filtered by the inherited selection rather than by globally omitting MCP.
    assert_eq!(app.clone().oneshot(request("PUT", format!("/api/sessions/{}/mcp/alpha", parent.id), json!({"included":false}))).await.unwrap().status(), StatusCode::OK);
    let parent_turn = app.clone().oneshot(request("POST", format!("/api/sessions/{}/turns/stream", parent.id), json!({"model":"openai/fixture","text":"delegate MCP selection"}))).await.unwrap();
    assert_eq!(parent_turn.status(), StatusCode::CREATED);
    let parent_body = tokio::spawn(async move { to_bytes(parent_turn.into_body(), 1024 * 1024).await.unwrap() });
    let parent_request = tokio::time::timeout(Duration::from_secs(10), seen_rx.recv()).await.unwrap().unwrap();
    let parent_tools = parent_request["tools"].as_array().unwrap();
    assert!(parent_tools.iter().any(|tool| tool["name"] == "task"));
    assert!(!parent_tools.iter().any(|tool| tool["name"].as_str().is_some_and(|name| name.ends_with("_alpha_probe"))));
    assert!(parent_tools.iter().any(|tool| tool["name"].as_str().is_some_and(|name| name.ends_with("_beta_probe"))));
    let permission = poll(&app, format!("/api/session/{}/permission", parent.id), |v| v["data"].as_array().is_some_and(|rows| rows.iter().any(|row| row["source"]["callID"] == "parent_task"))).await;
    let permission_id = permission["data"].as_array().unwrap().iter().find(|row| row["source"]["callID"] == "parent_task").unwrap()["id"].as_str().unwrap();
    assert_eq!(app.clone().oneshot(request("POST", format!("/api/session/{}/permission/{permission_id}/reply", parent.id), json!({"reply":"once"}))).await.unwrap().status(), StatusCode::NO_CONTENT);
    let child_request = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let body = seen_rx.recv().await.unwrap();
            if body["input"].to_string().contains("CHILD_MCP_SELECTION") { break body; }
        }
    }).await.unwrap();
    let child_tools = child_request["tools"].as_array().unwrap();
    assert!(child_tools.iter().any(|tool| tool["name"].as_str().is_some_and(|name| name.ends_with("_beta_probe"))), "writable child must retain included beta MCP tool");
    assert!(!child_tools.iter().any(|tool| tool["name"].as_str().is_some_and(|name| name.ends_with("_alpha_probe"))), "child must inherit excluded alpha MCP server");
    let parent_continuation = tokio::time::timeout(Duration::from_secs(10), async { loop { let body = seen_rx.recv().await.unwrap(); if body["input"].as_array().is_some_and(|items| items.iter().any(|item| item["type"] == "function_call_output")) { break body; } } }).await.unwrap();
    assert!(parent_continuation["input"].to_string().contains("child completed"));
    let _ = tokio::time::timeout(Duration::from_secs(10), parent_body).await.unwrap().unwrap();
}
