//! Session MCP selection must change real outbound provider definitions.
use axum::{body::{to_bytes, Body}, http::{Request, StatusCode}, routing::post, Json};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use std::{env, sync::Arc, time::Duration};
use tower::ServiceExt;

struct Environment(Vec<(&'static str, Option<std::ffi::OsString>)>);
impl Environment {
    fn set(values: &[(&'static str, String)]) -> Self {
        Self(values.iter().map(|(key, value)| {
            let old = env::var_os(key); env::set_var(key, value); (*key, old)
        }).collect())
    }
}
impl Drop for Environment { fn drop(&mut self) {
    for (key, old) in &self.0 { if let Some(old) = old { env::set_var(key, old); } else { env::remove_var(key); } }
} }
struct Task(tokio::task::JoinHandle<()>);
impl Drop for Task { fn drop(&mut self) { self.0.abort(); } }

async fn request(app: &axum::Router, method: &str, path: &str, value: Value) -> (StatusCode, Value) {
    let response = app.clone().oneshot(Request::builder().method(method).uri(path)
        .header("content-type", "application/json").body(Body::from(value.to_string())).unwrap()).await.unwrap();
    let status = response.status();
    let bytes = tokio::time::timeout(Duration::from_secs(15), to_bytes(response.into_body(), 256 * 1024)).await.unwrap().unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

#[tokio::test]
async fn session_mcp_selection_filters_actual_tools_without_affecting_another_session() {
    let root = tempfile::tempdir().unwrap();
    let script = root.path().join("mcp-provider-fixture.sh");
    std::fs::write(&script, r###"#!/bin/sh
set -eu
while IFS= read -r body; do
  id=$(printf '%s' "$body" | sed -n 's/.*"id"[[:space:]]*:[[:space:]]*\([0-9][0-9]*\).*/\1/p')
  case "$body" in
    *'"method":"initialize"'*) out="{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{\"tools\":{}},\"serverInfo\":{\"name\":\"provider-fixture\",\"version\":\"1\"}}}";;
    *'"method":"tools/list"'*) out="{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"tools\":[{\"name\":\"${1}_probe\",\"description\":\"${1} selection probe\",\"inputSchema\":{\"type\":\"object\"}}]}}";;
    *) continue;;
  esac
  printf '%s\n' "$out"
done
"###).unwrap();
    let sessions = SessionService::new(Arc::new(Storage::open_in_memory(root.path().join("blobs")).unwrap()));
    let app = router(AppState { sessions, catalog: Arc::new(Catalog::default()) });
    for name in ["alpha", "beta"] {
        let (status, body) = request(&app, "POST", "/api/mcp", json!({"name":name,"config":{"type":"local","command":["/bin/sh",script,name],"environment":{},"enabled":true}})).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let (status, body) = request(&app, "POST", &format!("/api/mcp/{name}/connect"), json!({})).await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Value>(4);
    let provider = axum::Router::new().route("/responses", post(move |Json(body): Json<Value>| {
        let tx = tx.clone(); async move {
            tx.send(body).await.unwrap();
            ([("content-type", "text/event-stream")], "data: {\"type\":\"response.output_text.delta\",\"delta\":\"Selection observed\"}\n\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_selection\",\"status\":\"completed\"}}\n\n")
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let _task = Task(tokio::spawn(async move { axum::serve(listener, provider).await.unwrap(); }));
    let _environment = Environment::set(&[("OPENAI_BASE_URL", format!("http://{address}")), ("OPENAI_API_KEY", "mcp-selection-fixture-key".into())]);
    let mut ids = Vec::new();
    for _ in 0..2 {
        let (status, body) = request(&app, "POST", "/api/sessions", json!({"title":"MCP selection", "directory":root.path()})).await;
        assert_eq!(status, StatusCode::CREATED);
        ids.push(body["session"]["id"].as_str().unwrap().to_owned());
    }
    for (round, session_index, alpha_included) in [(0, 0, true), (1, 0, false), (2, 1, true), (3, 0, true)] {
        if round == 1 || round == 3 {
            let (status, body) = request(&app, "PUT", &format!("/api/sessions/{}/mcp/alpha", ids[0]), json!({"included":alpha_included})).await;
            assert_eq!(status, StatusCode::OK, "session selection must be acknowledged: {body}");
        }
        let (status, _) = request(&app, "POST", &format!("/api/sessions/{}/turns/stream", ids[session_index]), json!({"model":"openai/fixture","text":format!("Observe selection {round}")})).await;
        assert!(status.is_success());
        let captured = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.unwrap().unwrap();
        let tools = captured["tools"].as_array().expect("actual connected MCP schemas must reach the provider");
        let has = |suffix: &str| tools.iter().any(|tool| tool["name"].as_str().is_some_and(|name| name.ends_with(suffix)));
        assert_eq!(has("alpha_probe"), alpha_included, "alpha inclusion must change actual provider definitions, round {round}");
        assert!(has("beta_probe"), "another server must stay included");
    }
    for name in ["alpha", "beta"] {
        assert_eq!(request(&app, "POST", &format!("/api/mcp/{name}/disconnect"), json!({})).await.0, StatusCode::OK);
    }
}
