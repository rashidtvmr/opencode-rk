//! Test-only child approval lifecycle: a child tool that needs human approval is
//! projected to the owning parent session, stays session-bound, and is settled
//! exactly once by that parent. Foreign, cancelled, stale and rejected replies
//! must never execute the child tool or settle a different pending request.
use std::{env, sync::{Arc, atomic::{AtomicUsize, Ordering}}, time::Duration};
use axum::{Router, Json, body::{Body, to_bytes}, extract::State, http::{Request, StatusCode}, routing::post};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{Value, json};
use tower::ServiceExt;

/// Process-global env is fixture state; serialize every journey so the default
/// parallel test runner cannot interleave `HOME`/`OPENAI_BASE_URL`.
static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const WRITE_CONTENT: &str = "session-child-approval-fixture-content";
const PARENT_PROBE: &str = "delegate child write probe";
const CHILD_PROBE: &str = "CHILD_WRITE_PROBE: write outside the project root";

struct Env(&'static str, Option<std::ffi::OsString>);
impl Env { fn set(name: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
    let previous = env::var_os(name); env::set_var(name, value); Self(name, previous)
} }
impl Drop for Env { fn drop(&mut self) { if let Some(v) = &self.1 { env::set_var(self.0, v); } else { env::remove_var(self.0); } } }

struct Fixture { outside: std::path::PathBuf, child_write_calls: AtomicUsize }

async fn provider(State(fixture): State<Arc<Fixture>>, Json(body): Json<Value>) -> ([(&'static str, &'static str);1], String) {
    let items = body["input"].as_array().unwrap();
    // The parent turn is the only one whose tool list still offers `task`;
    // children must never delegate recursively.
    let parent = body["tools"].as_array().unwrap().iter().any(|tool| tool["name"] == "task");
    let last_user = items.iter().rposition(|item| item["role"] == "user").unwrap();
    let output = items[last_user+1..].iter().any(|item| item["type"] == "function_call_output");
    let event = if parent && !output {
        json!({"type":"response.output_item.done","item":{"type":"function_call","id":"fc_task","call_id":"parent_task","name":"task","arguments":json!({"description":"child write probe","prompt":CHILD_PROBE,"subagent_type":"general","background":true}).to_string()}})
    } else if !parent && !output {
        fixture.child_write_calls.fetch_add(1, Ordering::SeqCst);
        json!({"type":"response.output_item.done","item":{"type":"function_call","id":"fc_child_write","call_id":"child_write","name":"write","arguments":json!({"path":&fixture.outside,"content":WRITE_CONTENT}).to_string()}})
    } else {
        json!({"type":"response.output_text.delta","delta":if parent {"background child started"} else {"child write settled"}})
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
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let result = value(app.clone().oneshot(request("GET", path.clone(), json!({}))).await.unwrap()).await;
            if predicate(&result) { return result; }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await.expect("fixture state must become observable within 5s")
}
async fn subagent_rows(app: &Router, parent: &str) -> Vec<Value> {
    let body = value(app.clone().oneshot(request("GET", format!("/api/session/{parent}/permission"), json!({}))).await.unwrap()).await;
    body["data"].as_array().map(|rows| rows.iter().filter(|row|
        row["action"].as_str().is_some_and(|action| action.starts_with("Subagent: "))).cloned().collect()).unwrap_or_default()
}
async fn wait_subagent_rows(app: &Router, parent: &str) -> Vec<Value> {
    poll(app, format!("/api/session/{parent}/permission"), |v| v["data"].as_array().is_some_and(|rows|
        rows.iter().any(|row| row["action"].as_str().is_some_and(|action| action.starts_with("Subagent: "))))).await;
    subagent_rows(app, parent).await
}
async fn wait_no_subagent_pending(app: &Router, parent: &str) {
    poll(app, format!("/api/session/{parent}/permission"), |v| v["data"].as_array().is_some_and(|rows|
        rows.iter().all(|row| !row["action"].as_str().is_some_and(|action| action.starts_with("Subagent: "))))).await;
}
async fn reply(app: &Router, session: &str, request_id: &str, kind: &str) -> StatusCode {
    app.clone().oneshot(request("POST", format!("/api/session/{session}/permission/{request_id}/reply"), json!({"reply":kind}))).await.unwrap().status()
}
async fn approve_parent_task(app: &Router, parent: &str) {
    let body = poll(app, format!("/api/session/{parent}/permission"), |v| v["data"].as_array().is_some_and(|rows| rows.iter().any(|row| row["action"] == "task"))).await;
    let id = body["data"].as_array().unwrap().iter().find(|row| row["action"] == "task").unwrap()["id"].as_str().unwrap().to_owned();
    assert_eq!(reply(app, parent, &id, "once").await, StatusCode::NO_CONTENT, "the parent task admission must be approvable once");
}
async fn wait_child_id(app: &Router, parent: &str) -> String {
    poll(app, format!("/api/sessions/{parent}/children"), |v| v["children"][0]["state"] == "running").await["children"][0]["session_id"].as_str().unwrap().to_owned()
}
async fn stop_child(app: &Router, parent: &str, child: &str) -> Value {
    value(app.clone().oneshot(request("POST", format!("/api/sessions/{parent}/children/{child}/stop"), json!({}))).await.unwrap()).await
}
async fn continue_child(app: &Router, parent: &str, child: &str, text: &str) -> Value {
    value(app.clone().oneshot(request("POST", format!("/api/sessions/{parent}/children/{child}/continue"), json!({"text":text}))).await.unwrap()).await
}
async fn wait_file(path: &std::path::Path) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !path.exists() { tokio::time::sleep(Duration::from_millis(5)).await; }
    }).await.expect("an approved child write must become observable within 5s");
}
async fn finish(h: Harness) {
    let body = tokio::time::timeout(Duration::from_secs(5), h.parent_body).await.expect("parent turn must settle within 5s").unwrap();
    let events: Vec<Value> = body.split(|byte| *byte == b'\n').filter(|line| !line.is_empty()).map(|line| serde_json::from_slice(line).unwrap()).collect();
    assert!(!events.iter().any(|event| event["type"] == "error"), "{events:?}");
    assert!(events.iter().any(|event| event["type"] == "assistant_message"), "{events:?}");
    h.mock.abort();
}

#[allow(dead_code)]
struct Harness {
    dir: tempfile::TempDir,
    fixture: Arc<Fixture>,
    app: Router,
    parent: String,
    foreign: String,
    parent_body: tokio::task::JoinHandle<Vec<u8>>,
    mock: tokio::task::JoinHandle<()>,
    envs: Vec<Env>,
}
async fn harness() -> Harness {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("project")).unwrap();
    let fixture = Arc::new(Fixture { outside: dir.path().join("outside.txt"), child_write_calls: AtomicUsize::new(0) });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let provider_fixture = fixture.clone();
    let mock = tokio::spawn(async move {
        axum::serve(listener, Router::new().fallback(post(provider)).with_state(provider_fixture)).await.unwrap()
    });
    let envs = vec![
        Env::set("OPENAI_BASE_URL", base),
        Env::set("OPENAI_API_KEY", "disposable-fixture-key"),
        Env::set("HOME", dir.path()),
        Env::set("XDG_CONFIG_HOME", dir.path()),
        Env::set("OPENCODE_RK_PERMISSION_RULES", dir.path().join("permission-rules.json")),
        Env::set("OPENCODE_RK_TURN_TOOLS", "task,write"),
    ];
    let sessions = SessionService::new(Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap()));
    let parent = sessions.create_in_directory("parent", Some(dir.path().join("project"))).await.unwrap();
    let foreign = sessions.create("foreign").await.unwrap();
    let app = router(AppState { sessions: sessions.clone(), catalog: Arc::new(Catalog::default()) });
    let response = app.clone().oneshot(request("POST", format!("/api/sessions/{}/turns/stream", parent.id), json!({"text":PARENT_PROBE, "model":"openai/fixture"}))).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let parent_body = tokio::spawn(async move { to_bytes(response.into_body(), 1024 * 1024).await.unwrap().to_vec() });
    Harness { dir, fixture, app, parent: parent.id.to_string(), foreign: foreign.id.to_string(), parent_body, mock, envs }
}

#[tokio::test(flavor="multi_thread", worker_threads=2)]
async fn child_write_approval_forwarded_to_parent_foreign_reply_not_found_then_once_executes() {
    let _test = TEST_LOCK.lock().await;
    let h = harness().await;
    approve_parent_task(&h.app, &h.parent).await;
    let rows = wait_subagent_rows(&h.app, &h.parent).await;
    assert_eq!(rows.len(), 1, "exactly one child approval must be projected to the parent: {rows:?}");
    let pending = &rows[0];
    assert_eq!(pending["action"], "Subagent: write", "the child tool approval must be forwarded with its action: {pending}");
    assert_eq!(pending["sessionID"].as_str(), Some(h.parent.as_str()), "the projection must be parent-scoped: {pending}");
    assert_eq!(pending["source"]["type"], "tool");
    assert_eq!(pending["source"]["callID"], "child_write");
    assert!(pending["resources"][0].as_str().is_some_and(|resource| resource.contains(h.fixture.outside.to_str().unwrap())), "{pending}");
    let request_id = pending["id"].as_str().unwrap().to_owned();

    assert_eq!(reply(&h.app, &h.foreign, &request_id, "once").await, StatusCode::NOT_FOUND, "a foreign parent must not settle a child approval");
    assert!(!h.fixture.outside.exists(), "a foreign reply must never execute the child tool");
    assert!(subagent_rows(&h.app, &h.parent).await.iter().any(|row| row["id"] == request_id), "a foreign reply must leave the child pending intact");

    assert_eq!(reply(&h.app, &h.parent, &request_id, "once").await, StatusCode::NO_CONTENT);
    wait_file(&h.fixture.outside).await;
    assert_eq!(std::fs::read_to_string(&h.fixture.outside).unwrap(), WRITE_CONTENT, "the approved child write must apply exactly its broker-approved content");
    let children = poll(&h.app, format!("/api/sessions/{}/children", h.parent), |v| v["children"][0]["state"] == "completed").await;
    assert_eq!(children["children"][0]["awaiting_permission"], false);
    assert_eq!(reply(&h.app, &h.parent, &request_id, "once").await, StatusCode::NOT_FOUND, "a settled once approval must be one-shot");
    assert_eq!(h.fixture.child_write_calls.load(Ordering::SeqCst), 1, "a settled approval must not re-request the child tool");
    finish(h).await;
}

#[tokio::test(flavor="multi_thread", worker_threads=2)]
async fn canceled_child_pending_stale_reply_not_found_and_side_effect_free() {
    let _test = TEST_LOCK.lock().await;
    let h = harness().await;
    approve_parent_task(&h.app, &h.parent).await;
    let rows = wait_subagent_rows(&h.app, &h.parent).await;
    assert_eq!(rows.len(), 1, "{rows:?}");
    let request_id = rows[0]["id"].as_str().unwrap().to_owned();
    let child = wait_child_id(&h.app, &h.parent).await;

    assert_eq!(stop_child(&h.app, &h.parent, &child).await["state"], "cancelled", "cancelling a child awaiting approval must settle it as cancelled");
    assert!(!h.fixture.outside.exists(), "a cancelled child approval must never execute its tool");
    wait_no_subagent_pending(&h.app, &h.parent).await;

    assert_eq!(reply(&h.app, &h.parent, &request_id, "once").await, StatusCode::NOT_FOUND, "replying to the cancelled generation's stale request must be not found");
    assert!(!h.fixture.outside.exists(), "a stale approval must have no effect");
    assert_eq!(h.fixture.child_write_calls.load(Ordering::SeqCst), 1, "a stale approval must not start another child generation");
    finish(h).await;
}

#[tokio::test(flavor="multi_thread", worker_threads=2)]
async fn followup_generation_pending_distinct_stale_approval_cannot_settle_and_reject_side_effect_free() {
    let _test = TEST_LOCK.lock().await;
    let h = harness().await;
    approve_parent_task(&h.app, &h.parent).await;
    let first = wait_subagent_rows(&h.app, &h.parent).await;
    assert_eq!(first.len(), 1, "{first:?}");
    let first_id = first[0]["id"].as_str().unwrap().to_owned();
    assert_eq!(first[0]["action"], "Subagent: write");
    let child = wait_child_id(&h.app, &h.parent).await;
    assert_eq!(stop_child(&h.app, &h.parent, &child).await["state"], "cancelled");
    wait_no_subagent_pending(&h.app, &h.parent).await;

    continue_child(&h.app, &h.parent, &child, "CHILD_WRITE_PROBE: follow-up generation").await;
    poll(&h.app, format!("/api/sessions/{}/children", h.parent), |v| v["children"][0]["generation"] == 2).await;
    let second = wait_subagent_rows(&h.app, &h.parent).await;
    assert_eq!(second.len(), 1, "{second:?}");
    let second_id = second[0]["id"].as_str().unwrap().to_owned();
    assert_ne!(first_id, second_id, "each child generation must mint a distinct pending id");
    assert_eq!(second[0]["action"], "Subagent: write");

    assert_eq!(reply(&h.app, &h.parent, &first_id, "once").await, StatusCode::NOT_FOUND, "the previous generation's stale approval must be not found");
    assert!(!h.fixture.outside.exists(), "a stale approval must not execute the new generation's tool");
    assert!(subagent_rows(&h.app, &h.parent).await.iter().any(|row| row["id"] == second_id), "a stale approval must not settle the new pending request");

    assert_eq!(reply(&h.app, &h.parent, &second_id, "reject").await, StatusCode::NO_CONTENT);
    assert!(!h.fixture.outside.exists(), "a rejected child write must never create the file");
    poll(&h.app, format!("/api/sessions/{}/children", h.parent), |v| v["children"][0]["state"] != "running" && v["children"][0]["awaiting_permission"] == false).await;
    wait_no_subagent_pending(&h.app, &h.parent).await;
    assert_eq!(h.fixture.child_write_calls.load(Ordering::SeqCst), 2, "a rejected generation must settle without a continuation request");
    finish(h).await;
}
