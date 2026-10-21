//! Real router -> provider -> broker -> child turn -> native read tool ->
//! durable child history, including cancellation and safe retry controls.
use std::{env, future::IntoFuture, sync::{Arc, atomic::{AtomicUsize, Ordering}}, time::Duration};
use axum::{Router, Json, body::{Body, to_bytes}, extract::State, http::{Request, StatusCode}, routing::post};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{Value, json};
use tower::ServiceExt;

struct Env(&'static str, Option<std::ffi::OsString>);
impl Env { fn set(name: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
    let previous = env::var_os(name); env::set_var(name, value); Self(name, previous)
} }
impl Drop for Env { fn drop(&mut self) { if let Some(v) = &self.1 { env::set_var(self.0, v); } else { env::remove_var(self.0); } } }

#[derive(Default)]
struct Fixture { release: tokio::sync::Notify, child_requests: AtomicUsize }
async fn provider(State(fixture): State<Arc<Fixture>>, Json(body): Json<Value>) -> ([(&'static str, &'static str);1], String) {
    let input = body["input"].to_string();
    let items = body["input"].as_array().unwrap();
    let last_user = items.iter().rposition(|item| item["role"] == "user").unwrap();
    let child = items[last_user]["content"].to_string().contains("CHILD_READ_PROBE");
    let output = items[last_user+1..].iter().any(|item| item["type"] == "function_call_output");
    let event = if child && !output {
        assert!(!body["tools"].as_array().unwrap().iter().any(|tool| tool["name"] == "task"), "children must not delegate recursively");
        fixture.child_requests.fetch_add(1, Ordering::SeqCst);
        fixture.release.notified().await;
        json!({"type":"response.output_item.done","item":{"type":"function_call","id":"fc_child_read","call_id":"child_read","name":"read","arguments":json!({"path":"probe.txt"}).to_string()}})
    } else if !child && !output {
        json!({"type":"response.output_item.done","item":{"type":"function_call","id":"fc_task","call_id":"parent_task","name":"task","arguments":json!({"description":"read probe","prompt":"CHILD_READ_PROBE: read probe.txt","subagent_type":"explore","background":true}).to_string()}})
    } else {
        if child { assert!(input.contains("fixture child content"), "real tool output must reach child continuation"); }
        json!({"type":"response.output_text.delta","delta":if child {"child read completed"} else {"background child started"}})
    };
    ([ ("content-type", "text/event-stream") ], format!("data: {event}\n\ndata: {{\"type\":\"response.completed\",\"response\":{{\"status\":\"completed\"}}}}\n\n"))
}
fn request(method: &str, path: String, body: Value) -> Request<Body> {
    Request::builder().method(method).uri(path).header("content-type", "application/json").body(Body::from(body.to_string())).unwrap()
}
async fn value(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
}
async fn poll(app: &Router, path: String, predicate: impl Fn(&Value)->bool) -> Value {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let result = value(app.clone().oneshot(request("GET",path.clone(),json!({}))).await.unwrap()).await;
            if predicate(&result) { return result; }
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
    }).await.expect("fixture state must become observable")
}

#[tokio::test(flavor="multi_thread", worker_threads=2)]
async fn background_child_stop_retry_real_tool_and_durable_detail() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("probe.txt"), "fixture child content").unwrap();
    let fixture = Arc::new(Fixture::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}",listener.local_addr().unwrap());
    let mock = tokio::spawn(axum::serve(listener, Router::new().fallback(post(provider)).with_state(fixture.clone())).into_future());
    let _env = [Env::set("OPENAI_BASE_URL", base), Env::set("OPENAI_API_KEY", "disposable-fixture-key"), Env::set("HOME",dir.path()), Env::set("XDG_CONFIG_HOME",dir.path()), Env::set("OPENCODE_RK_TURN_TOOLS","task,read")];
    let sessions = SessionService::new(Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap()));
    let parent = sessions.create_in_directory("parent",Some(dir.path().into())).await.unwrap();
    let foreign = sessions.create("foreign").await.unwrap();
    let app = router(AppState { sessions:sessions.clone(), catalog:Arc::new(Catalog::default()) });
    let response = app.clone().oneshot(request("POST",format!("/api/sessions/{}/turns/stream",parent.id),json!({"text":"delegate probe", "model":"openai/fixture"}))).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = tokio::spawn(async move { to_bytes(response.into_body(),1024*1024).await.unwrap() });
    let permission = poll(&app,format!("/api/session/{}/permission",parent.id), |v| v["data"].as_array().is_some_and(|rows| !rows.is_empty())).await;
    let request_id = permission["data"][0]["id"].as_str().unwrap();
    assert_eq!(permission["data"][0]["action"],"task");
    let denied = app.clone().oneshot(request("POST",format!("/api/session/{}/permission/{request_id}/reply",foreign.id),json!({"reply":"once"}))).await.unwrap();
    assert_eq!(denied.status(), StatusCode::NOT_FOUND);
    assert_eq!(fixture.child_requests.load(Ordering::SeqCst),0);
    let approved = app.clone().oneshot(request("POST",format!("/api/session/{}/permission/{request_id}/reply",parent.id),json!({"reply":"once"}))).await.unwrap();
    assert_eq!(approved.status(), StatusCode::NO_CONTENT);
    let listing = poll(&app,format!("/api/sessions/{}/children",parent.id), |v| v["children"][0]["state"] == "running").await;
    let child = listing["children"][0]["session_id"].as_str().unwrap().to_owned();
    assert_eq!(listing["children"][0]["state"],"running");
    let parent_body = tokio::time::timeout(Duration::from_secs(8),body).await.unwrap().unwrap();
    let parent_events: Vec<Value> = parent_body.split(|byte| *byte == b'\n').filter(|line| !line.is_empty()).map(|line|serde_json::from_slice(line).unwrap()).collect();
    assert!(!parent_events.iter().any(|event| event["type"] == "error"), "{parent_events:?}");
    assert!(parent_events.iter().any(|event| event["type"] == "assistant_message"), "{parent_events:?}");
    tokio::time::timeout(Duration::from_secs(8), async {
        while fixture.child_requests.load(Ordering::SeqCst) < 1 { tokio::time::sleep(Duration::from_millis(15)).await; }
    }).await.unwrap();
    let stopped = value(app.clone().oneshot(request("POST",format!("/api/sessions/{}/children/{child}/stop",parent.id),json!({}))).await.unwrap()).await;
    assert_eq!(stopped["state"],"cancelled");
    let retry = app.clone().oneshot(request("POST",format!("/api/sessions/{}/children/{child}/retry",parent.id),json!({}))).await.unwrap();
    assert_eq!(retry.status(),StatusCode::OK);
    tokio::time::timeout(Duration::from_secs(8), async {
        while fixture.child_requests.load(Ordering::SeqCst) < 2 { tokio::time::sleep(Duration::from_millis(15)).await; }
    }).await.unwrap();
    fixture.release.notify_waiters();
    let final_state = poll(&app,format!("/api/sessions/{}/children",parent.id), |v| v["children"][0]["state"] == "completed").await;
    assert_eq!(final_state["children"][0]["output_tail"],"child read completed");
    assert_eq!(final_state["children"][0]["has_tool_calls"],true);
    let child_id = child.parse().unwrap();
    assert!(!sessions.display_tool_history(child_id,100).await.unwrap().is_empty());
    assert!(sessions.has_tool_admissions(child_id).await.unwrap());
    assert!(!sessions.has_tool_admissions(foreign.id).await.unwrap());
    assert!(sessions.messages(child_id,100).await.unwrap().iter().any(|m| m.role == opencode_rk_contracts::MessageRole::Assistant));
    let retry = app.clone().oneshot(request("POST",format!("/api/sessions/{}/children/{child}/retry",parent.id),json!({}))).await.unwrap();
    assert_eq!(retry.status(),StatusCode::BAD_REQUEST,"tool-bearing task must never be blindly replayed");
    let denied_parent = sessions.create_in_directory("denied parent",Some(dir.path().into())).await.unwrap();
    let response = app.clone().oneshot(request("POST",format!("/api/sessions/{}/turns/stream",denied_parent.id),json!({"text":"delegate denied probe", "model":"openai/fixture"}))).await.unwrap();
    assert_eq!(response.status(),StatusCode::CREATED);
    let denied_body = tokio::spawn(async move { to_bytes(response.into_body(),1024*1024).await.unwrap() });
    let permission = poll(&app,format!("/api/session/{}/permission",denied_parent.id), |v| v["data"].as_array().is_some_and(|rows| !rows.is_empty())).await;
    let request_id = permission["data"][0]["id"].as_str().unwrap();
    let reply = app.clone().oneshot(request("POST",format!("/api/session/{}/permission/{request_id}/reply",denied_parent.id),json!({"reply":"reject"}))).await.unwrap();
    assert_eq!(reply.status(),StatusCode::NO_CONTENT);
    tokio::time::timeout(Duration::from_secs(8),denied_body).await.unwrap().unwrap();
    let listing = value(app.clone().oneshot(request("GET",format!("/api/sessions/{}/children",denied_parent.id),json!({}))).await.unwrap()).await;
    assert_eq!(listing["children"],json!([]),"denied delegation must never create a child session");
    assert_eq!(fixture.child_requests.load(Ordering::SeqCst),2,"denied delegation must never reach the child provider");
    mock.abort();
}
