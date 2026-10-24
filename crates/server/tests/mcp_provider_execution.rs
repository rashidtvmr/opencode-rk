//! Black-box MCP provider execution: stdio discovery, approval, and continuation.
#![forbid(unsafe_code)]

use axum::{body::{to_bytes, Body}, http::{Request, StatusCode}, routing::post, Json};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use std::{env, sync::{Arc, OnceLock}, time::Duration};
use tempfile::TempDir;
use tokio::sync::Mutex;
use tower::ServiceExt;

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
const TOOL_ARGUMENTS: &str = r#"{"value":"exact-mcp-arguments"}"#;

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
    fn drop(&mut self) { for (key, value) in &self.0 { match value { Some(v) => env::set_var(key, v), None => env::remove_var(key) } } }
}

struct AbortOnDrop(tokio::task::JoinHandle<()>);
impl Drop for AbortOnDrop { fn drop(&mut self) { self.0.abort(); } }

async fn call(app: &axum::Router, method: &str, path: &str, value: Value) -> (StatusCode, Value) {
    let response = app.clone().oneshot(Request::builder().method(method).uri(path)
        .header("content-type", "application/json").body(Body::from(value.to_string())).unwrap()).await.unwrap();
    let status = response.status();
    let bytes = tokio::time::timeout(Duration::from_secs(10), to_bytes(response.into_body(), 512 * 1024)).await.unwrap().unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned())))
}

fn write_fixture(dir: &TempDir) -> String {
    let path = dir.path().join("mcp-execution.sh");
    let source = format!(r###"#!/bin/sh
set -eu
while IFS= read -r body; do
  id=$(printf '%s' "$body" | sed -n 's/.*"id"[[:space:]]*:[[:space:]]*\([0-9][0-9]*\).*/\1/p')
  case "$body" in
    *'"method":"initialize"'*) out="{{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{{\"tools\":{{}}}},\"serverInfo\":{{\"name\":\"execution-fixture\",\"version\":\"1\"}}}}}}";;
    *'"method":"tools/list"'*) out="{{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{{\"tools\":[{{\"name\":\"execute_marker\",\"description\":\"writes the disposable marker\",\"inputSchema\":{{\"type\":\"object\",\"properties\":{{\"value\":{{\"type\":\"string\"}}}},\"required\":[\"value\"]}}}}]}}}}";;
    *'"method":"tools/call"'*) printf '%s' "$body" > "$1"; out="{{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{{\"content\":[{{\"type\":\"text\",\"text\":\"marker written\"}}],\"isError\":false}}}}";;
    *) continue;;
  esac
  printf '%s\n' "$out"
done
"###);
    std::fs::write(&path, source).unwrap();
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; let mut p = std::fs::metadata(&path).unwrap().permissions(); p.set_mode(0o700); std::fs::set_permissions(&path, p).unwrap(); }
    path.to_string_lossy().into_owned()
}

async fn scenario(mode: &'static str, app_root: &std::path::Path) {
    let mcp_dir = tempfile::tempdir().unwrap();
    let marker = app_root.join(format!("marker-{mode}"));
    let fixture = write_fixture(&mcp_dir);
    let rules = app_root.join(format!("rules-{mode}.json"));
    std::fs::write(&rules, r#"{"rules":[]}"#).unwrap();
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&rules, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    let (requests, mut seen) = tokio::sync::mpsc::channel::<Value>(4);
    let provider = axum::Router::new().route("/responses", post(move |Json(body): Json<Value>| {
        let requests = requests.clone();
        async move {
            let _ = requests.send(body.clone()).await;
            let continuation = body.get("input").and_then(Value::as_array).is_some_and(|items| items.iter().any(|item| item["type"] == "function_call_output"));
            if continuation {
                let output = if body["input"].as_array().unwrap().iter().any(|item| item["type"] == "function_call_output" && item["output"].as_str().is_some_and(|text| text.contains("permission rejected") || text.contains("disabled"))) { "failure output observed" } else { "continuation complete" };
                ([ ("content-type", "text/event-stream") ], format!("data: {{\"type\":\"response.output_text.delta\",\"delta\":\"{output}\"}}\n\ndata: {{\"type\":\"response.completed\",\"response\":{{\"status\":\"completed\"}}}}\n\n"))
            } else {
                let name = body["tools"].as_array().and_then(|tools| tools.iter().find_map(|tool| tool["name"].as_str().filter(|name| name.starts_with("mcp_")))).unwrap_or("mcp_missing");
                let response = format!("data: {{\"type\":\"response.output_item.done\",\"item\":{{\"type\":\"function_call\",\"call_id\":\"call_execution\",\"name\":\"{name}\",\"arguments\":{TOOL_ARGUMENTS:?}}}}}\n\ndata: {{\"type\":\"response.completed\",\"response\":{{\"status\":\"completed\"}}}}\n\n");
                ([ ("content-type", "text/event-stream") ], response)
            }
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let _provider_task = AbortOnDrop(tokio::spawn(async move { axum::serve(listener, provider).await.unwrap(); }));
    let _env = EnvGuard::set(&[("OPENAI_BASE_URL", format!("http://{address}")), ("OPENAI_API_KEY", "mcp-execution-fixture".into()), ("OPENCODE_RK_TURN_TOOLS", "mcp".into()), ("OPENCODE_RK_PERMISSION_RULES", rules.to_string_lossy().into_owned())]);

    let storage = Storage::open_in_memory(app_root.join(format!("blobs-{mode}"))).unwrap();
    let app = router(AppState { sessions: SessionService::new(Arc::new(storage)), catalog: Arc::new(Catalog::default()) });
    assert_eq!(call(&app, "POST", "/api/mcp", json!({"name":"fixture","config":{"type":"local","command":["/bin/sh",fixture,marker],"environment":{},"enabled":true}})).await.0, StatusCode::OK);
    assert_eq!(call(&app, "POST", "/api/mcp/fixture/connect", json!({})).await.0, StatusCode::OK);
    let (_, session) = call(&app, "POST", "/api/sessions", json!({"title":"MCP execution","directory":app_root})).await;
    let id = session["session"]["id"].as_str().unwrap().to_owned();
    let mut turn = tokio::spawn({ let app = app.clone(); let id = id.clone(); async move { call(&app, "POST", &format!("/api/sessions/{id}/turns/stream"), json!({"model":"openai/fixture","text":"execute the marker tool"})).await } });
    let initial = tokio::time::timeout(Duration::from_secs(10), seen.recv()).await.unwrap().unwrap();
    let name = initial["tools"].as_array().unwrap().iter().find_map(|t| t["name"].as_str().filter(|n| n.starts_with("mcp_")).map(str::to_owned)).expect("qualified MCP tool advertised");
    assert_eq!(initial["tools"].as_array().unwrap().iter().find(|t| t["name"] == name).expect("qualified MCP tool present")["name"], name);
    let pending = tokio::time::timeout(Duration::from_secs(10), async { loop {
        let (status, value) = call(&app, "GET", &format!("/api/session/{id}/permission"), Value::Null).await;
        assert_eq!(status, StatusCode::OK, "permission listing failed: {value}");
        if let Some(item) = value["data"].as_array().and_then(|a| a.first()).filter(|v| v["source"]["callID"] == "call_execution") { break item.clone(); }
        if turn.is_finished() { panic!("turn ended before requesting MCP approval: {:?}", (&mut turn).await.unwrap()); }
        tokio::task::yield_now().await;
    } }).await.expect("MCP execution must request real human approval");
    let request_id = pending["id"].as_str().unwrap();
    if mode == "saved-deny" {
        opencode_rk_server::permission_rules::upsert(opencode_rk_server::permission_rules::Rule {
            tool: name.clone(), decision:"deny".into(), scope:"always".into(),
            session_id:None, resource:None, context:None,
        }).expect("human deny preference must be persisted while approval is pending");
    }
    if mode == "toggle" { assert_eq!(call(&app, "PUT", &format!("/api/sessions/{id}/mcp/fixture"), json!({"included":false})).await.0, StatusCode::OK); }
    let reply = if mode == "reject" { "reject" } else { "once" };
    assert_eq!(call(&app, "POST", &format!("/api/session/{id}/permission/{request_id}/reply"), json!({"reply":reply})).await.0, StatusCode::NO_CONTENT);
    let _ = tokio::time::timeout(Duration::from_secs(10), turn).await.unwrap().unwrap();
    let continuation = tokio::time::timeout(Duration::from_secs(10), seen.recv()).await.unwrap().unwrap();
    assert!(continuation["input"].as_array().unwrap().iter().any(|v| v["type"] == "function_call_output" && v["call_id"] == "call_execution"));
    let output = continuation["input"].as_array().unwrap().iter().find(|v| v["type"] == "function_call_output" && v["call_id"] == "call_execution").unwrap()["output"].as_str().unwrap();
    if mode == "approve" { assert!(output.contains("marker written"), "approved MCP output must be successful: {output}"); }
    else if mode == "reject" { assert!(output.contains("permission rejected"), "reject must produce failure output: {output}"); }
    else if mode == "saved-deny" { assert!(output.contains("denied"), "a persisted deny must remain terminal after approval: {output}"); }
    else { assert!(output.contains("disabled"), "pending exclusion must produce failure output: {output}"); }
    assert_eq!(call(&app, "POST", "/api/mcp/fixture/disconnect", Value::Null).await.0, StatusCode::OK);
    if mode == "approve" {
        let captured: Value = serde_json::from_str(&std::fs::read_to_string(&marker).unwrap()).expect("captured tools/call body");
        assert_eq!(captured["params"]["name"], "execute_marker");
        assert_eq!(captured["params"]["arguments"], json!({"value":"exact-mcp-arguments"}));
    } else { assert!(!marker.exists()); }
}

#[tokio::test]
async fn mcp_provider_execution_approval_rejection_and_pending_toggle() {
    let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().await;
    let root = tempfile::tempdir().unwrap();
    scenario("approve", root.path()).await;
    scenario("reject", root.path()).await;
    scenario("toggle", root.path()).await;
}

#[tokio::test]
async fn a_persisted_deny_added_during_mcp_approval_prevents_external_execution() {
    let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().await;
    let root = tempfile::tempdir().unwrap();
    scenario("saved-deny", root.path()).await;
}
