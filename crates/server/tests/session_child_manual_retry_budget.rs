//! Manual child retries are a bounded, pre-side-effect control.
//!
//! This deliberately exercises the public router with a real loopback provider:
//! the parent task permission is approved by the parent, while every child
//! provider request fails before a tool can be admitted.  A retry is counted
//! only when the child has been admitted and a new generation is started.
use std::{
    env,
    future::IntoFuture,
    sync::{Arc, Mutex, atomic::{AtomicBool, AtomicUsize, Ordering}},
    time::Duration,
};

use axum::{
    Router,
    Json,
    body::{Body, to_bytes},
    extract::State,
    http::{Request, Response, StatusCode},
    response::IntoResponse,
    routing::post,
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{Value, json};
use tower::ServiceExt;

const MAX_MANUAL_RETRIES: u64 = 3;
const EXHAUSTION: &str = "manual retry budget exhausted (maximum 3 retries)";

static ENV_LOCK: Mutex<()> = Mutex::new(());

struct Env(&'static str, Option<std::ffi::OsString>);
impl Env {
    fn set(name: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let previous = env::var_os(name);
        env::set_var(name, value);
        Self(name, previous)
    }
}
impl Drop for Env {
    fn drop(&mut self) {
        if let Some(value) = &self.1 { env::set_var(self.0, value); }
        else { env::remove_var(self.0); }
    }
}

#[derive(Default)]
struct Fixture {
    requests: AtomicUsize,
    child_requests: AtomicUsize,
    hold_first_child: AtomicBool,
    release_first_child: AtomicBool,
}

async fn provider(State(fixture): State<Arc<Fixture>>, Json(body): Json<Value>) -> Response<Body> {
    fixture.requests.fetch_add(1, Ordering::SeqCst);
    let input = body["input"].as_array().expect("responses input array");
    let user = input.iter()
        .rfind(|item| item["role"] == "user")
        .and_then(|item| item["content"].as_str())
        .expect("fixture provider request must contain a user message with string content");
    if user.contains("CHILD_RETRY_BUDGET") {
        let child_request = fixture.child_requests.fetch_add(1, Ordering::SeqCst) + 1;
        if child_request == 1 && fixture.hold_first_child.load(Ordering::SeqCst) {
            while !fixture.release_first_child.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }
        // A provider failure, rather than an admission failure: no tool call
        // or function output is ever returned to the child turn engine.
        return (StatusCode::BAD_REQUEST, Json(json!({
            "error": {"message": "permanent fixture provider failure", "type": "invalid_request_error"}
        }))).into_response();
    }

    let output_done = input.iter().any(|item| item["type"] == "function_call_output");
    let event = if output_done {
        json!({"type":"response.output_text.delta","delta":"parent finished"})
    } else {
        json!({"type":"response.output_item.done","item":{
            "type":"function_call", "id":"fc_retry_budget", "call_id":"parent_task",
            "name":"task", "arguments":json!({
                "description":"retry budget child", "prompt":"CHILD_RETRY_BUDGET: permanently fail before tools",
                "subagent_type":"explore", "background":true
            }).to_string()
        }})
    };
    ([ ("content-type", "text/event-stream") ],
        format!("data: {event}\n\ndata: {{\"type\":\"response.completed\",\"response\":{{\"status\":\"completed\"}}}}\n\n"))
        .into_response()
}

fn request(method: &str, path: String, body: Value) -> Request<Body> {
    Request::builder().method(method).uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string())).unwrap()
}

async fn json_body(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
}

async fn poll(app: &Router, path: String, predicate: impl Fn(&Value) -> bool) -> Value {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let value = json_body(app.clone().oneshot(request("GET", path.clone(), json!({}))).await.unwrap()).await;
            if predicate(&value) { return value; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("child state must become observable")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn child_manual_retry_budget_counts_only_admitted_generations_and_exhausts_at_three() {
    let _env_lock = ENV_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let sentinel = dir.path().join("must-not-be-created");
    let fixture = Arc::new(Fixture { hold_first_child: AtomicBool::new(true), ..Default::default() });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let mock = tokio::spawn(axum::serve(listener, Router::new().fallback(post(provider)).with_state(fixture.clone())).into_future());
    let _env = [
        Env::set("OPENAI_BASE_URL", base), Env::set("OPENAI_API_KEY", "disposable-fixture-key"),
        Env::set("HOME", dir.path()), Env::set("XDG_CONFIG_HOME", dir.path()),
        Env::set("OPENCODE_CONFIG_DIR", dir.path().join("global-config")), Env::set("OPENCODE_RK_HOME", dir.path()),
        Env::set("OPENCODE_RK_PERMISSION_RULES", dir.path().join("permission-rules.json")),
        Env::set("OPENCODE_RK_TURN_TOOLS", "task,read"),
    ];
    let sessions = SessionService::new(Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap()));
    let parent = sessions.create_in_directory("retry budget parent", Some(dir.path().into())).await.unwrap();
    let app = router(AppState { sessions: sessions.clone(), catalog: Arc::new(Catalog::default()) });

    let response = app.clone().oneshot(request("POST", format!("/api/sessions/{}/turns/stream", parent.id),
        json!({"text":"delegate retry budget child", "model":"openai/fixture"}))).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let parent_body = tokio::spawn(async move { to_bytes(response.into_body(), 1024 * 1024).await.unwrap() });
    let permission = poll(&app, format!("/api/session/{}/permission", parent.id),
        |v| v["data"].as_array().is_some_and(|rows| !rows.is_empty())).await;
    let permission_id = permission["data"][0]["id"].as_str().unwrap();
    let approved = app.clone().oneshot(request("POST", format!("/api/session/{}/permission/{permission_id}/reply", parent.id), json!({"reply":"once"}))).await.unwrap();
    assert_eq!(approved.status(), StatusCode::NO_CONTENT);
    let listing = poll(&app, format!("/api/sessions/{}/children", parent.id), |v| v["children"][0]["state"] == "running").await;
    let child = listing["children"][0]["session_id"].as_str().unwrap().to_owned();
    tokio::time::timeout(Duration::from_secs(8), parent_body).await.unwrap().unwrap();
    assert_eq!(fixture.child_requests.load(Ordering::SeqCst), 1);

    // A running retry is rejected before it consumes budget or starts a generation.
    let running_retry = app.clone().oneshot(request("POST", format!("/api/sessions/{}/children/{child}/retry", parent.id), json!({}))).await.unwrap();
    assert_eq!(running_retry.status(), StatusCode::BAD_REQUEST);
    assert!(json_body(running_retry).await.to_string().contains("still running"));
    assert_eq!(fixture.child_requests.load(Ordering::SeqCst), 1);
    let running_snapshot = json_body(app.clone().oneshot(request("GET", format!("/api/sessions/{}/children", parent.id), json!({}))).await.unwrap()).await;
    assert_eq!(running_snapshot["children"][0]["manual_retries"], 0);
    assert_eq!(running_snapshot["children"][0]["generation"], 1);

    fixture.release_first_child.store(true, Ordering::SeqCst);
    poll(&app, format!("/api/sessions/{}/children", parent.id), |v| v["children"][0]["state"] == "error").await;
    for expected in 1..=MAX_MANUAL_RETRIES {
        let retry = app.clone().oneshot(request("POST", format!("/api/sessions/{}/children/{child}/retry", parent.id), json!({}))).await.unwrap();
        assert_eq!(retry.status(), StatusCode::OK, "admitted manual retry {expected}");
        let state = poll(&app, format!("/api/sessions/{}/children", parent.id), |v| v["children"][0]["state"] == "error").await;
        assert_eq!(state["children"][0]["manual_retries"], expected);
        assert_eq!(state["children"][0]["generation"], expected + 1);
    }
    let before = fixture.requests.load(Ordering::SeqCst);
    let before_state = json_body(app.clone().oneshot(request("GET", format!("/api/sessions/{}/children", parent.id), json!({}))).await.unwrap()).await;
    let exhausted = app.clone().oneshot(request("POST", format!("/api/sessions/{}/children/{child}/retry", parent.id), json!({}))).await.unwrap();
    assert_eq!(exhausted.status(), StatusCode::TOO_MANY_REQUESTS);
    let exhausted_body = json_body(exhausted).await;
    assert_eq!(exhausted_body, json!({"code":"too_many_requests", "message":EXHAUSTION}));
    assert_eq!(fixture.requests.load(Ordering::SeqCst), before);
    let after = json_body(app.clone().oneshot(request("GET", format!("/api/sessions/{}/children", parent.id), json!({}))).await.unwrap()).await;
    assert_eq!(after["children"][0]["generation"], before_state["children"][0]["generation"]);
    assert_eq!(after["children"][0]["manual_retries"], MAX_MANUAL_RETRIES);
    assert!(!sessions.has_tool_admissions(child.parse().unwrap()).await.unwrap());
    assert!(!sentinel.exists(), "a pre-admission provider failure must not create file effects");
    mock.abort();
}
