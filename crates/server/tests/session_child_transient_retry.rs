//! Bounded automatic retry for eligible transient provider stream failures
//! during a child turn, without replaying completed or ambiguous side effects.
//!
//! Pre-sideeffect replay-safe failures (provider stream error/cut before any
//! tool_call and no durable tool admissions) are retried with bounded
//! exponential backoff. Permanent errors (bad_request), post-tool-cut errors,
//! and manual retry budget (3 max) are terminal.
//!
//! This is a compiling RED test: it exercises the real router/daemon/permission
//! broker with a loopback Responses SSE provider fixture. The fixture uses the
//! wire events consumed by `ResponsesStreamParser`.
use std::{
    env,
    future::IntoFuture,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use axum::{
    Router,
    body::{Body, to_bytes},
    extract::State,
    http::{Method, Request, StatusCode},
    Json,
    response::IntoResponse,
    routing::post,
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{Value, json};
use tower::ServiceExt;
use tokio::sync::mpsc;

const MAX_AUTO_RETRIES: u64 = 3;
const MAX_MANUAL_RETRIES: u64 = 3;

static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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
        if let Some(value) = &self.1 {
            env::set_var(self.0, value);
        } else {
            env::remove_var(self.0);
        }
    }
}

#[derive(Default)]
struct Fixture {
    /// Counts every provider request across all child generations.
    child_requests: AtomicUsize,
    /// Confirms the cancellation case reached its classified transient fault,
    /// rather than merely stopping an unstarted child.
    transient_failures: AtomicUsize,
    successful_read_output: AtomicBool,
}

#[derive(Default)]
struct InterruptFixture {
    child_requests: AtomicUsize,
    stream_started: tokio::sync::Notify,
    stream_sender: std::sync::Mutex<Option<mpsc::Sender<axum::body::Bytes>>>,
    headers_started: tokio::sync::Notify,
    headers_release: tokio::sync::Notify,
}

async fn interrupt_provider(
    State(fixture): State<Arc<InterruptFixture>>,
    Json(body): Json<Value>,
) -> axum::response::Response {
    let input = body["input"].as_array().expect("responses input array");
    let user = input.iter().rfind(|item| item["role"] == "user")
        .map(|item| item["content"].to_string()).unwrap_or_default();
    if user.contains("CHILD_INTERRUPT_HOLD_HEADERS") {
        fixture.child_requests.fetch_add(1, Ordering::SeqCst);
        fixture.headers_started.notify_one();
        fixture.headers_release.notified().await;
        return ([ ("content-type", "text/event-stream") ], "data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"bad_gateway\",\"message\":\"released after interrupt\"}}}\n\n".to_owned()).into_response();
    }
    if user.contains("CHILD_INTERRUPT_HOLD") {
        fixture.child_requests.fetch_add(1, Ordering::SeqCst);
        let (sender, receiver) = mpsc::channel(2);
        sender.send(axum::body::Bytes::from(
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"held\"}\n\n",
        )).await.expect("held stream receiver must be installed");
        *fixture.stream_sender.lock().unwrap() = Some(sender);
        fixture.stream_started.notify_one();
        let stream = futures_util::stream::unfold(receiver, |mut receiver| async move {
            receiver.recv().await.map(|bytes| (Ok::<_, std::convert::Infallible>(bytes), receiver))
        });
        return ([("content-type", "text/event-stream")], axum::body::Body::from_stream(stream)).into_response();
    }
    let output_done = input.iter().any(|item| item["type"] == "function_call_output");
    let child_prompt = if user.contains("interrupt headers child") {
        "CHILD_INTERRUPT_HOLD_HEADERS: hold before headers"
    } else {
        "CHILD_INTERRUPT_HOLD: hold stream"
    };
    let event = if output_done {
        json!({"type":"response.output_text.delta","delta":"parent done"})
    } else {
        json!({"type":"response.output_item.done","item":{"type":"function_call","id":"fc_interrupt_child","call_id":"parent_task","name":"task","arguments":json!({"description":"interrupt child","prompt":child_prompt,"subagent_type":"general","background":true}).to_string()}})
    };
    ([ ("content-type", "text/event-stream") ], format!("data: {event}\n\ndata: {{\"type\":\"response.completed\",\"response\":{{\"status\":\"completed\"}}}}\n\n")).into_response()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn session_interrupt_cancels_held_child_stream_without_retry() {
    let _serial = TEST_LOCK.lock().await;
    let dir = tempfile::tempdir().unwrap();
    let fixture = Arc::new(InterruptFixture::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let mock = tokio::spawn(axum::serve(listener, Router::new().fallback(post(interrupt_provider)).with_state(fixture.clone())).into_future());
    let _env = [
        Env::set("OPENAI_BASE_URL", &base), Env::set("OPENAI_API_KEY", "disposable-fixture-key"),
        Env::set("HOME", dir.path()), Env::set("XDG_CONFIG_HOME", dir.path()),
        Env::set("OPENCODE_CONFIG_DIR", dir.path().join("global-config")), Env::set("OPENCODE_RK_HOME", dir.path()),
        Env::set("OPENCODE_RK_PERMISSION_RULES", dir.path().join("permission-rules.json")),
        Env::set("OPENCODE_RK_TURN_TOOLS", "task,read"),
    ];
    let sessions = SessionService::new(Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap()));
    let parent = sessions.create_in_directory("interrupt child parent", Some(dir.path().into())).await.unwrap();
    let app = router(AppState { sessions: sessions.clone(), catalog: Arc::new(Catalog::default()) });
    let response = app.clone().oneshot(request(Method::POST, format!("/api/sessions/{}/turns/stream", parent.id), json!({"text":"delegate interrupt child", "model":"openai/fixture"}))).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let parent_body = tokio::spawn(async move { to_bytes(response.into_body(), 1024 * 1024).await.unwrap() });
    let permission = poll(&app, format!("/api/session/{}/permission", parent.id), |v| v["data"].as_array().is_some_and(|rows| !rows.is_empty())).await.0;
    let permission_id = permission["data"][0]["id"].as_str().unwrap();
    let approved = app.clone().oneshot(request(Method::POST, format!("/api/session/{}/permission/{permission_id}/reply", parent.id), json!({"reply":"once"}))).await.unwrap();
    assert_eq!(approved.status(), StatusCode::NO_CONTENT);
    let child = poll(&app, format!("/api/sessions/{}/children", parent.id), |v| v["children"].as_array().is_some_and(|rows| rows.len() == 1 && rows[0]["state"] == "running")).await.0;
    let child_id = child["children"][0]["session_id"].as_str().unwrap().to_owned();
    tokio::time::timeout(Duration::from_secs(3), fixture.stream_started.notified()).await.unwrap();
    let child = poll(&app, format!("/api/sessions/{}/children", parent.id), |v| {
        v["children"].as_array().is_some_and(|rows| {
            rows.len() == 1
                && rows[0]["state"] == "running"
                && rows[0]["output_tail"] == "held"
                && fixture.child_requests.load(Ordering::SeqCst) == 1
        })
    }).await.0;
    assert_eq!(child["children"][0]["output_tail"], "held");
    let interrupted = tokio::time::timeout(Duration::from_secs(3), app.clone().oneshot(request(Method::POST, format!("/api/session/{child_id}/interrupt"), json!({})))).await.unwrap().unwrap();
    assert_eq!(interrupted.status(), StatusCode::NO_CONTENT);
    let final_state = poll(&app, format!("/api/sessions/{}/children", parent.id), |v| v["children"].as_array().is_some_and(|rows| rows.len() == 1 && rows[0]["state"] == "cancelled")).await.0;
    assert_eq!(final_state["children"][0]["state"], "cancelled");
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(fixture.child_requests.load(Ordering::SeqCst), 1);
    tokio::time::timeout(Duration::from_secs(3), parent_body).await.unwrap().unwrap();
    mock.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn session_interrupt_cancels_child_before_provider_headers_without_retry() {
    let _serial = TEST_LOCK.lock().await;
    let dir = tempfile::tempdir().unwrap();
    let fixture = Arc::new(InterruptFixture::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let mock = tokio::spawn(axum::serve(listener, Router::new().fallback(post(interrupt_provider)).with_state(fixture.clone())).into_future());
    let _env = [
        Env::set("OPENAI_BASE_URL", &base), Env::set("OPENAI_API_KEY", "disposable-fixture-key"),
        Env::set("HOME", dir.path()), Env::set("XDG_CONFIG_HOME", dir.path()),
        Env::set("OPENCODE_CONFIG_DIR", dir.path().join("global-config")), Env::set("OPENCODE_RK_HOME", dir.path()),
        Env::set("OPENCODE_RK_PERMISSION_RULES", dir.path().join("permission-rules.json")),
        Env::set("OPENCODE_RK_TURN_TOOLS", "task,read"),
    ];
    let sessions = SessionService::new(Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap()));
    let parent = sessions.create_in_directory("interrupt headers parent", Some(dir.path().into())).await.unwrap();
    let app = router(AppState { sessions: sessions.clone(), catalog: Arc::new(Catalog::default()) });
    let response = app.clone().oneshot(request(Method::POST, format!("/api/sessions/{}/turns/stream", parent.id), json!({"text":"delegate interrupt headers child", "model":"openai/fixture"}))).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let parent_body = tokio::spawn(async move { to_bytes(response.into_body(), 1024 * 1024).await.unwrap() });
    let permission = poll(&app, format!("/api/session/{}/permission", parent.id), |v| v["data"].as_array().is_some_and(|rows| !rows.is_empty())).await.0;
    let permission_id = permission["data"][0]["id"].as_str().unwrap();
    let approved = app.clone().oneshot(request(Method::POST, format!("/api/session/{}/permission/{permission_id}/reply", parent.id), json!({"reply":"once"}))).await.unwrap();
    assert_eq!(approved.status(), StatusCode::NO_CONTENT);
    let child = poll(&app, format!("/api/sessions/{}/children", parent.id), |v| v["children"].as_array().is_some_and(|rows| rows.len() == 1 && rows[0]["state"] == "running")).await.0;
    let child_id = child["children"][0]["session_id"].as_str().unwrap().to_owned();
    tokio::time::timeout(Duration::from_secs(3), fixture.headers_started.notified()).await.unwrap();
    assert_eq!(fixture.child_requests.load(Ordering::SeqCst), 1);
    let interrupted = tokio::time::timeout(Duration::from_secs(3), app.clone().oneshot(request(Method::POST, format!("/api/session/{child_id}/interrupt"), json!({})))).await.unwrap().unwrap();
    assert_eq!(interrupted.status(), StatusCode::NO_CONTENT);
    let final_state = poll(&app, format!("/api/sessions/{}/children", parent.id), |v| v["children"].as_array().is_some_and(|rows| rows.len() == 1 && rows[0]["state"] == "cancelled")).await.0;
    assert_eq!(final_state["children"][0]["state"], "cancelled");
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(fixture.child_requests.load(Ordering::SeqCst), 1);
    fixture.headers_release.notify_one();
    tokio::time::timeout(Duration::from_secs(3), parent_body).await.unwrap().unwrap();
    mock.abort();
}

/// Provider fixture that simulates transient vs permanent stream failures.
///
/// Behavior is controlled by marker strings in the user prompt:
/// - "TRANSIENT_STREAM_ERROR": first child request emits `response.failed`
///   before any function call; second request succeeds.
/// - "PERMANENT_BAD_REQUEST": emits `response.failed` with `error.code` set
///   to `bad_request` every time.
/// - "TOOL_THEN_ERROR": emits a real Responses function-call item, executes
///   it, then fails the next provider round (ambiguous).
/// - "STREAM_CUT": closes the stream with an incomplete SSE data record.
async fn provider(
    State(fixture): State<Arc<Fixture>>,
    Json(body): Json<Value>,
) -> ([(&'static str, &'static str); 1], String) {
    let input = body["input"].as_array().expect("responses input array");
    let user = input
        .iter()
        .rfind(|item| item["role"] == "user")
        .map(|item| item["content"].to_string())
        .expect("fixture provider request must contain a user message");

    if user.contains("CHILD_TRANSIENT") {
        let attempt = fixture.child_requests.fetch_add(1, Ordering::SeqCst) + 1;
        if user.contains("TRANSIENT_FAIL_FIRST") {
            // First attempt: emit a transient Responses failure before any
            // function_call item. The provider error code is explicit in the
            // supported response.failed envelope.
            if attempt == 1 {
                fixture.transient_failures.fetch_add(1, Ordering::SeqCst);
                return (
                    [("content-type", "text/event-stream")],
                    "event: response.failed\ndata: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\",\"error\":{\"code\":\"bad_gateway\",\"message\":\"provider stream ended before completion\"}}}\n\n".to_owned(),
                );
            }
            // Retry succeeds.
            return (
                [("content-type", "text/event-stream")],
                format!(
                    "data: {{\"type\":\"response.output_text.delta\",\"delta\":\"recovered after retry\"}}\n\n\
                     data: {{\"type\":\"response.completed\",\"response\":{{\"status\":\"completed\"}}}}\n\n"
                ),
            );
        }
        if user.contains("TRANSIENT_FAIL_ALWAYS") {
            // Always fails transiently (for testing auto-retry exhaustion + manual retry).
            fixture.transient_failures.fetch_add(1, Ordering::SeqCst);
            return (
                [("content-type", "text/event-stream")],
                "event: response.failed\ndata: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\",\"error\":{\"code\":\"bad_gateway\",\"message\":\"provider stream ended before completion\"}}}\n\n".to_owned(),
            );
        }
        if user.contains("PERMANENT_BAD_REQUEST") {
            return (
                [("content-type", "text/event-stream")],
                "event: response.failed\ndata: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\",\"error\":{\"code\":\"bad_request\",\"message\":\"invalid request\"}}}\n\n".to_owned(),
            );
        }
        if user.contains("TOOL_THEN_ERROR") {
            let has_output = input.iter().any(|item| item["type"] == "function_call_output");
            if !has_output {
                return (
                    [("content-type", "text/event-stream")],
                    "event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"id\":\"fc_child_read\",\"call_id\":\"child_read\",\"name\":\"read\",\"arguments\":\"{\\\"path\\\":\\\"probe.txt\\\"}\"}}\n\nevent: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n".to_owned(),
                );
            }
            let output = input
                .iter()
                .find(|item| item["type"] == "function_call_output" && item["call_id"] == "child_read")
                .and_then(|item| item["output"].as_str())
                .expect("tool output must use the Responses function_call_output envelope");
            let output: Value = serde_json::from_str(output).expect("tool output must be opencode.tool-result.v1 JSON");
            assert_eq!(output["format"], "opencode.tool-result.v1");
            assert_eq!(output["success"], true);
            assert_eq!(output["output"], "fixture child content");
            fixture.successful_read_output.store(true, Ordering::SeqCst);
            return (
                [("content-type", "text/event-stream")],
                "event: response.failed\ndata: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\",\"error\":{\"code\":\"bad_gateway\",\"message\":\"stream cut after tool admission\"}}}\n\n".to_owned(),
            );
        }
        if user.contains("STREAM_CUT") {
            // First attempt: stream ends abruptly with no completion event
            // (connection drop), which the client reports as UnexpectedStreamEnd.
            // Retry succeeds.
            if attempt == 1 {
                return ([("content-type", "text/event-stream")], "data: {\"type\":\"response.output_text.delta\",\"delta\":\"partial".to_owned());
            }
            return (
                [("content-type", "text/event-stream")],
                format!(
                    "data: {{\"type\":\"response.output_text.delta\",\"delta\":\"recovered after retry\"}}\n\n\
                     data: {{\"type\":\"response.completed\",\"response\":{{\"status\":\"completed\"}}}}\n\n"
                ),
            );
        }
    }

    // Root/parent turn: delegate via task tool.
    let output_done = input
        .iter()
        .any(|item| item["type"] == "function_call_output");
    let event = if output_done {
        json!({"type":"response.output_text.delta","delta":"parent done"})
    } else {
        json!({"type":"response.output_item.done","item":{
            "type":"function_call","id":"fc_child","call_id":"parent_task",
            "name":"task","arguments":json!({
                "description":"transient child","prompt":user.replace("PARENT_DELEGATE ","CHILD_TRANSIENT "),
                "subagent_type":"general","background":true
            }).to_string()
        }})
    };
    (
        [("content-type", "text/event-stream")],
        format!(
            "data: {event}\n\n\
             data: {{\"type\":\"response.completed\",\"response\":{{\"status\":\"completed\"}}}}\n\n"
        ),
    )
}

fn request(method: Method, uri: String, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn json_body(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
}

async fn poll(app: &Router, path: String, predicate: impl Fn(&Value) -> bool) -> (Value, Duration) {
    let started = Instant::now();
    let mut last = Value::Null;
    let result = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            last = json_body(
                app.clone()
                    .oneshot(request(Method::GET, path.clone(), json!({})))
                    .await
                    .unwrap(),
            )
            .await;
            if predicate(&last) {
                return last.clone();
            }
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("fixture state at {path} must become observable; last snapshot: {last}"));
    (result, started.elapsed())
}

/// Approve the parent's first task-tool permission request and drain the
/// parent turn SSE body.
async fn delegate(app: &Router, parent: &opencode_rk_contracts::SessionId, marker: &str) {
    let response = app
        .clone()
        .oneshot(request(
            Method::POST,
            format!("/api/sessions/{}/turns/stream", parent),
            json!({"text": marker, "model":"openai/fixture"}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = tokio::spawn(async move {
        to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap()
    });
    let (permission, _) = poll(
        app,
        format!("/api/session/{}/permission", parent),
        |v| v["data"].as_array().is_some_and(|rows| !rows.is_empty()),
    )
    .await;
    let request_id = permission["data"][0]["id"].as_str().unwrap().to_owned();
    let approved = app
        .clone()
        .oneshot(request(
            Method::POST,
            format!("/api/session/{}/permission/{request_id}/reply", parent),
            json!({"reply": "once"}),
        ))
        .await
        .unwrap();
    assert_eq!(approved.status(), StatusCode::NO_CONTENT);
    let body = tokio::time::timeout(Duration::from_secs(10), body)
        .await
        .unwrap()
        .unwrap();
    let events: Vec<Value> = body
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect();
    assert!(
        !events.iter().any(|event| event["type"] == "error"),
        "parent turn must not fail: {events:?}"
    );
}

/// A transient provider stream error before any tool_call is retried with
/// bounded exponential backoff, then succeeds. The durable admission ledger
/// must show zero tool admissions; history has exactly one user intent.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transient_stream_error_before_tools_is_retried_then_succeeds() {
    let _serial = TEST_LOCK.lock().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("probe.txt"), "fixture child content").unwrap();
    let fixture = Arc::new(Fixture::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let mock = tokio::spawn(
        axum::serve(
            listener,
            axum::Router::new().fallback(post(provider)).with_state(fixture.clone()),
        )
            .into_future(),
    );
    let _env = [
        Env::set("OPENAI_BASE_URL", &base),
        Env::set("OPENAI_API_KEY", "disposable-fixture-key"),
        Env::set("HOME", dir.path()),
        Env::set("XDG_CONFIG_HOME", dir.path()),
        Env::set("OPENCODE_CONFIG_DIR", dir.path().join("global-config")),
        Env::set("OPENCODE_RK_HOME", dir.path()),
        Env::set("OPENCODE_RK_PERMISSION_RULES", dir.path().join("permission-rules.json")),
        Env::set("OPENCODE_RK_TURN_TOOLS", "task,read"),
    ];
    let sessions = SessionService::new(Arc::new(
        Storage::open_in_memory(dir.path().join("blobs")).unwrap(),
    ));
    let parent = sessions
        .create_in_directory("transient retry parent", Some(dir.path().into()))
        .await
        .unwrap();
    let app = router(AppState {
        sessions: sessions.clone(),
        catalog: Arc::new(Catalog::default()),
    });

    delegate(
        &app,
        &parent.id,
        "PARENT_DELEGATE TRANSIENT_FAIL_FIRST test",
    )
    .await;

    // Child should complete after retry (2 provider calls total).
    let (done, elapsed) = poll(
        &app,
        format!("/api/sessions/{}/children", parent.id),
        |v| {
            v["children"].as_array().is_some_and(|rows| {
                rows.len() == 1
                    && matches!(
                        rows[0]["state"].as_str(),
                        Some("completed" | "error")
                    )
            })
        },
    )
    .await;
    let child = &done["children"][0];
    assert_eq!(
        child["state"].as_str(),
        Some("completed"),
        "child must recover after transient stream error: {child}"
    );
    assert_eq!(
        child["output_tail"].as_str(),
        Some("recovered after retry"),
        "child must show recovered output: {child}"
    );
    assert_eq!(
        fixture.child_requests.load(Ordering::SeqCst),
        2,
        "transient retry must reach provider exactly twice: first fails, second succeeds"
    );
    assert!(elapsed <= Duration::from_secs(15), "retry fixture must remain bounded: {elapsed:?}");
    // Durable admission ledger: no tool admissions ever happened.
    assert!(
        !sessions
            .has_tool_admissions(
                child["session_id"]
                    .as_str()
                    .unwrap()
                    .parse()
                    .unwrap()
            )
            .await
            .unwrap(),
        "transient retry before any tool must leave no durable tool admission records"
    );
    // History must have exactly one user intent (the child prompt), not a replay.
    let user_texts: Vec<String> = sessions
        .messages(
            child["session_id"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap(),
            100,
        )
        .await
        .unwrap()
        .into_iter()
        .filter(|m| m.role == opencode_rk_contracts::MessageRole::User)
        .filter_map(|m| match &m.body {
            opencode_rk_contracts::PayloadRef::Inline { text } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        user_texts.len(),
        1,
        "retry must not duplicate user prompt in durable history: {user_texts:?}"
    );
    mock.abort();
}

/// A permanent bad_request SSE error must never be retried.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn permanent_bad_request_is_not_retried() {
    let _serial = TEST_LOCK.lock().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("probe.txt"), "fixture child content").unwrap();
    let fixture = Arc::new(Fixture::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let mock = tokio::spawn(
        axum::serve(
            listener,
            axum::Router::new().fallback(post(provider)).with_state(fixture.clone()),
        )
            .into_future(),
    );
    let _env = [
        Env::set("OPENAI_BASE_URL", &base),
        Env::set("OPENAI_API_KEY", "disposable-fixture-key"),
        Env::set("HOME", dir.path()),
        Env::set("XDG_CONFIG_HOME", dir.path()),
        Env::set("OPENCODE_CONFIG_DIR", dir.path().join("global-config")),
        Env::set("OPENCODE_RK_HOME", dir.path()),
        Env::set("OPENCODE_RK_PERMISSION_RULES", dir.path().join("permission-rules.json")),
        Env::set("OPENCODE_RK_TURN_TOOLS", "task,read"),
    ];
    let sessions = SessionService::new(Arc::new(
        Storage::open_in_memory(dir.path().join("blobs")).unwrap(),
    ));
    let parent = sessions
        .create_in_directory("permanent error parent", Some(dir.path().into()))
        .await
        .unwrap();
    let app = router(AppState {
        sessions: sessions.clone(),
        catalog: Arc::new(Catalog::default()),
    });

    delegate(
        &app,
        &parent.id,
        "PARENT_DELEGATE PERMANENT_BAD_REQUEST test",
    )
    .await;

    let (done, _) = poll(
        &app,
        format!("/api/sessions/{}/children", parent.id),
        |v| {
            v["children"].as_array().is_some_and(|rows| {
                rows.len() == 1
                    && matches!(
                        rows[0]["state"].as_str(),
                        Some("completed" | "error")
                    )
            })
        },
    )
    .await;
    let child = &done["children"][0];
    assert_eq!(
        child["state"].as_str(),
        Some("error"),
        "permanent bad_request must not retry: {child}"
    );
    assert_eq!(
        fixture.child_requests.load(Ordering::SeqCst),
        1,
        "permanent bad_request must never reach provider again"
    );
    // Wait briefly to confirm no late retry.
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(
        fixture.child_requests.load(Ordering::SeqCst),
        1,
        "no late retry after permanent error"
    );
    mock.abort();
}

/// A stream cut (connection closed mid-text, no error event) before any tool_call
/// is a transient failure that should be retried.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stream_cut_before_tools_is_retried() {
    let _serial = TEST_LOCK.lock().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("probe.txt"), "fixture child content").unwrap();
    let fixture = Arc::new(Fixture::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let mock = tokio::spawn(
        axum::serve(
            listener,
            axum::Router::new().fallback(post(provider)).with_state(fixture.clone()),
        )
            .into_future(),
    );
    let _env = [
        Env::set("OPENAI_BASE_URL", &base),
        Env::set("OPENAI_API_KEY", "disposable-fixture-key"),
        Env::set("HOME", dir.path()),
        Env::set("XDG_CONFIG_HOME", dir.path()),
        Env::set("OPENCODE_CONFIG_DIR", dir.path().join("global-config")),
        Env::set("OPENCODE_RK_HOME", dir.path()),
        Env::set("OPENCODE_RK_PERMISSION_RULES", dir.path().join("permission-rules.json")),
        Env::set("OPENCODE_RK_TURN_TOOLS", "task,read"),
    ];
    let sessions = SessionService::new(Arc::new(
        Storage::open_in_memory(dir.path().join("blobs")).unwrap(),
    ));
    let parent = sessions
        .create_in_directory("stream cut parent", Some(dir.path().into()))
        .await
        .unwrap();
    let app = router(AppState {
        sessions: sessions.clone(),
        catalog: Arc::new(Catalog::default()),
    });

    delegate(
        &app,
        &parent.id,
        "PARENT_DELEGATE STREAM_CUT test",
    )
    .await;

    let (done, _) = poll(
        &app,
        format!("/api/sessions/{}/children", parent.id),
        |v| {
            v["children"].as_array().is_some_and(|rows| {
                rows.len() == 1
                    && matches!(
                        rows[0]["state"].as_str(),
                        Some("completed" | "error")
                    )
            })
        },
    )
    .await;
    let child = &done["children"][0];
    assert_eq!(
        child["state"].as_str(),
        Some("completed"),
        "stream cut before tools should retry and succeed: {child}"
    );
    assert_eq!(
        fixture.child_requests.load(Ordering::SeqCst),
        2,
        "stream cut must perform exactly one retry"
    );
    mock.abort();
}

/// A provider stream error that occurs AFTER a tool_call has been observed is
/// ambiguous and must NOT be auto-retried. The child is terminal in error state.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stream_error_after_tool_admission_is_not_auto_retried() {
    let _serial = TEST_LOCK.lock().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("probe.txt"), "fixture child content").unwrap();
    let fixture = Arc::new(Fixture::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let mock = tokio::spawn(
        axum::serve(
            listener,
            axum::Router::new().fallback(post(provider)).with_state(fixture.clone()),
        )
            .into_future(),
    );
    let _env = [
        Env::set("OPENAI_BASE_URL", &base),
        Env::set("OPENAI_API_KEY", "disposable-fixture-key"),
        Env::set("HOME", dir.path()),
        Env::set("XDG_CONFIG_HOME", dir.path()),
        Env::set("OPENCODE_CONFIG_DIR", dir.path().join("global-config")),
        Env::set("OPENCODE_RK_HOME", dir.path()),
        Env::set("OPENCODE_RK_PERMISSION_RULES", dir.path().join("permission-rules.json")),
        Env::set("OPENCODE_RK_TURN_TOOLS", "task,read"),
    ];
    let sessions = SessionService::new(Arc::new(
        Storage::open_in_memory(dir.path().join("blobs")).unwrap(),
    ));
    let parent = sessions
        .create_in_directory("ambiguous error parent", Some(dir.path().into()))
        .await
        .unwrap();
    let app = router(AppState {
        sessions: sessions.clone(),
        catalog: Arc::new(Catalog::default()),
    });

    delegate(
        &app,
        &parent.id,
        "PARENT_DELEGATE TOOL_THEN_ERROR test",
    )
    .await;

    let (done, _) = poll(
        &app,
        format!("/api/sessions/{}/children", parent.id),
        |v| {
            v["children"].as_array().is_some_and(|rows| {
                rows.len() == 1
                    && matches!(
                        rows[0]["state"].as_str(),
                        Some("completed" | "error")
                    )
            })
        },
    )
    .await;
    let child = &done["children"][0];
    assert_eq!(
        child["state"].as_str(),
        Some("error"),
        "ambiguous error after tool must be terminal: {child}"
    );
    // Durable admission ledger must show a tool admission existed.
    assert!(
        sessions
            .has_tool_admissions(
                child["session_id"]
                    .as_str()
                    .unwrap()
                    .parse()
                    .unwrap()
            )
            .await
            .unwrap(),
        "tool admission must be durable even after failure"
    );
    // The function-call round is a normal two-request turn: the first request
    // advertises the call, and the second request receives its durable output
    // before the provider failure. The safety property is no third request.
    let child_id = child["session_id"].as_str().unwrap().parse().unwrap();
    let tool_history = sessions.display_tool_history(child_id, 100).await.unwrap();
    assert_eq!(tool_history.len(), 2, "one call and one output record must be retained");
    let calls: Vec<_> = tool_history.iter().filter(|item| item.kind == "call").collect();
    let outputs: Vec<_> = tool_history.iter().filter(|item| item.kind == "output").collect();
    assert_eq!(calls.len(), 1, "exactly one tool call record must be retained");
    assert_eq!(outputs.len(), 1, "exactly one tool output record must be retained");
    assert_eq!(calls[0].call_id, "child_read");
    assert_eq!(outputs[0].call_id, "child_read");
    let persisted_output: Value = serde_json::from_str(&outputs[0].payload)
        .expect("persisted tool output must be valid JSON");
    assert_eq!(persisted_output["format"], "opencode.tool-result.v1");
    assert_eq!(persisted_output["success"], true);
    assert_eq!(persisted_output["output"], "fixture child content");
    assert!(sessions.has_tool_admissions(child_id).await.unwrap());
    assert!(
        fixture.successful_read_output.load(Ordering::SeqCst),
        "the ambiguous failure must follow a successful durable read result"
    );
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(
        fixture.child_requests.load(Ordering::SeqCst),
        2,
        "ambiguous error must not trigger a third provider request"
    );
    mock.abort();
}

/// Cancellation during the transient-retry backoff sleep must not allow the
/// pending retry attempt to fire. No extra provider calls beyond the first.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancellation_during_backoff_stops_retry() {
    let _serial = TEST_LOCK.lock().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("probe.txt"), "fixture child content").unwrap();
    let fixture = Arc::new(Fixture::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let mock = tokio::spawn(
        axum::serve(
            listener,
            axum::Router::new().fallback(post(provider)).with_state(fixture.clone()),
        )
            .into_future(),
    );
    let _env = [
        Env::set("OPENAI_BASE_URL", &base),
        Env::set("OPENAI_API_KEY", "disposable-fixture-key"),
        Env::set("HOME", dir.path()),
        Env::set("XDG_CONFIG_HOME", dir.path()),
        Env::set("OPENCODE_CONFIG_DIR", dir.path().join("global-config")),
        Env::set("OPENCODE_RK_HOME", dir.path()),
        Env::set("OPENCODE_RK_PERMISSION_RULES", dir.path().join("permission-rules.json")),
        Env::set("OPENCODE_RK_TURN_TOOLS", "task,read"),
    ];
    let sessions = SessionService::new(Arc::new(
        Storage::open_in_memory(dir.path().join("blobs")).unwrap(),
    ));
    let parent = sessions
        .create_in_directory("cancel backoff parent", Some(dir.path().into()))
        .await
        .unwrap();
    let app = router(AppState {
        sessions: sessions.clone(),
        catalog: Arc::new(Catalog::default()),
    });

    delegate(
        &app,
        &parent.id,
        "PARENT_DELEGATE TRANSIENT_FAIL_FIRST cancel-backoff test",
    )
    .await;

    // Wait until the provider has failed transiently and the child has entered
    // the retry backoff. A terminal error would make this test vacuous.
    let (listing, _) = poll(
        &app,
        format!("/api/sessions/{}/children", parent.id),
        |v| {
            v["children"].as_array().is_some_and(|rows| {
                rows.len() == 1
                    && matches!(rows[0]["state"].as_str(), Some("queued" | "running"))
                    && rows[0]["auto_retry_attempts"].as_u64() == Some(1)
                    && fixture.child_requests.load(Ordering::SeqCst) == 1
                })
        },
    )
    .await;
    let child_id = listing["children"][0]["session_id"].as_str().unwrap().to_string();
    assert_eq!(
        fixture.transient_failures.load(Ordering::SeqCst),
        1,
        "cancellation case must observe the classified transient provider failure before stop"
    );

    // The child is in the transient retry state; cancel it during backoff.
    let stopped = tokio::time::timeout(
        Duration::from_secs(3),
        app.clone().oneshot(request(
            Method::POST,
            format!("/api/sessions/{}/children/{}/stop", parent.id, child_id),
            json!({}),
        )),
    )
    .await
    .expect("stop must not deadlock")
    .unwrap();
    let stopped = json_body(stopped).await;
    assert_eq!(
        stopped["state"].as_str(),
        Some("cancelled"),
        "child must be cancellable during backoff: {stopped}"
    );

    // No further provider calls after cancellation.
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(
        fixture.child_requests.load(Ordering::SeqCst),
        1,
        "cancellation during backoff must not trigger extra retry attempts"
    );
    mock.abort();
}

/// Manual retry (explicit /retry endpoint) preserves the existing 3-attempt
/// budget and works alongside automatic retries. After auto-retry exhaustion,
/// manual retry is still available and counted independently.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_retry_budget_independent_of_auto_retry_exhaustion() {
    let _serial = TEST_LOCK.lock().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("probe.txt"), "fixture child content").unwrap();
    let fixture = Arc::new(Fixture::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let mock = tokio::spawn(
        axum::serve(
            listener,
            axum::Router::new().fallback(post(provider)).with_state(fixture.clone()),
        )
            .into_future(),
    );
    let _env = [
        Env::set("OPENAI_BASE_URL", &base),
        Env::set("OPENAI_API_KEY", "disposable-fixture-key"),
        Env::set("HOME", dir.path()),
        Env::set("XDG_CONFIG_HOME", dir.path()),
        Env::set("OPENCODE_CONFIG_DIR", dir.path().join("global-config")),
        Env::set("OPENCODE_RK_HOME", dir.path()),
        Env::set("OPENCODE_RK_PERMISSION_RULES", dir.path().join("permission-rules.json")),
        Env::set("OPENCODE_RK_TURN_TOOLS", "task,read"),
    ];
    let sessions = SessionService::new(Arc::new(
        Storage::open_in_memory(dir.path().join("blobs")).unwrap(),
    ));
    let parent = sessions
        .create_in_directory("manual retry parent", Some(dir.path().into()))
        .await
        .unwrap();
    let app = router(AppState {
        sessions: sessions.clone(),
        catalog: Arc::new(Catalog::default()),
    });

    delegate(
        &app,
        &parent.id,
        "PARENT_DELEGATE TRANSIENT_FAIL_ALWAYS test",
    )
    .await;

    // The policy is three automatic retries per generation: one initial
    // request plus three retries. Manual retry starts a fresh generation and
    // receives the same automatic retry allowance independently.
    let (failed, _) = poll(
        &app,
        format!("/api/sessions/{}/children", parent.id),
        |v| v["children"].as_array().is_some_and(|rows| rows.len() == 1 && rows[0]["state"] == "error"),
    )
    .await;
    let child_id = failed["children"][0]["session_id"].as_str().unwrap().to_string();
    assert_eq!(
        fixture.child_requests.load(Ordering::SeqCst),
        MAX_AUTO_RETRIES as usize + 1,
        "auto-retry must exhaust at {MAX_AUTO_RETRIES} retries: {failed}"
    );
    assert_eq!(failed["children"][0]["generation"].as_u64(), Some(1));
    assert_eq!(failed["children"][0]["manual_retries"].as_u64(), Some(0));
    assert_eq!(failed["children"][0]["auto_retry_attempts"].as_u64(), Some(MAX_AUTO_RETRIES));
    let initial_generation_requests = MAX_AUTO_RETRIES as usize + 1;

    // The provider always fails transiently. Each explicit manual retry must
    // start a new generation, not merely re-observe the previous terminal
    // state, and remains available up to three generations.
    for expected in 1..=MAX_MANUAL_RETRIES {
        let before = fixture.child_requests.load(Ordering::SeqCst);
        let before_snapshot = json_body(
            app.clone()
                .oneshot(request(
                    Method::GET,
                    format!("/api/sessions/{}/children", parent.id),
                    json!({}),
                ))
                .await
                .unwrap(),
        ).await;
        let before_generation = before_snapshot["children"][0]["generation"].as_u64().unwrap();
        let retry = app
            .clone()
            .oneshot(request(
                Method::POST,
                format!("/api/sessions/{}/children/{}/retry", parent.id, child_id),
                json!({}),
            ))
            .await
            .unwrap();
        assert_eq!(
            retry.status(),
            StatusCode::OK,
            "manual retry {expected} must be accepted"
        );
        let (state, _) = poll(
            &app,
            format!("/api/sessions/{}/children", parent.id),
            |v| {
                v["children"].as_array().is_some_and(|rows| {
                    rows.len() == 1
                        && rows[0]["state"] == "error"
                        && fixture.child_requests.load(Ordering::SeqCst)
                            >= before + MAX_AUTO_RETRIES as usize + 1
                })
            },
        )
        .await;
        assert_eq!(
            fixture.child_requests.load(Ordering::SeqCst),
            before + MAX_AUTO_RETRIES as usize + 1,
            "manual retry {expected} must execute a complete four-request generation"
        );
        assert_eq!(
            fixture.child_requests.load(Ordering::SeqCst),
            initial_generation_requests * (expected as usize + 1),
            "manual retry {expected} must reach cumulative request boundary {}",
            initial_generation_requests * (expected as usize + 1)
        );
        assert_eq!(state["children"][0]["state"], "error");
        assert_eq!(state["children"][0]["generation"].as_u64(), Some(before_generation + 1));
        assert_eq!(state["children"][0]["manual_retries"].as_u64(), Some(expected));
        assert_eq!(state["children"][0]["auto_retry_attempts"].as_u64(), Some(MAX_AUTO_RETRIES));
    }

    // Exhausted: manual retry budget is 3.
    let before_rejected_retry = json_body(
        app.clone()
            .oneshot(request(
                Method::GET,
                format!("/api/sessions/{}/children", parent.id),
                json!({}),
            ))
            .await
            .unwrap(),
    )
    .await;
    let before_rejected_child = before_rejected_retry["children"][0].clone();
    let exhausted = app
        .clone()
        .oneshot(request(
            Method::POST,
            format!("/api/sessions/{}/children/{}/retry", parent.id, child_id),
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(
        exhausted.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "manual retry budget must be exhausted after {MAX_MANUAL_RETRIES}"
    );
    let body = json_body(exhausted).await;
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("manual retry budget exhausted"),
        "exhaustion message must be visible: {body}"
    );
    let after_rejected_retry = json_body(
        app.clone()
            .oneshot(request(
                Method::GET,
                format!("/api/sessions/{}/children", parent.id),
                json!({}),
            ))
            .await
            .unwrap(),
    )
    .await;
    let after_rejected_child = &after_rejected_retry["children"][0];
    assert_eq!(
        after_rejected_child["generation"],
        before_rejected_child["generation"],
        "rejected manual retry must not claim a new generation"
    );
    assert_eq!(after_rejected_child["generation"], 4);
    assert_eq!(after_rejected_child["manual_retries"], 3);
    assert_eq!(after_rejected_child["state"], "error");
    let stopped = tokio::time::timeout(
        Duration::from_secs(3),
        app.clone().oneshot(request(
            Method::POST,
            format!("/api/sessions/{}/children/{}/stop", parent.id, child_id),
            json!({}),
        )),
    )
    .await
    .expect("stop after rejected retry must complete")
    .unwrap();
    assert_eq!(stopped.status(), StatusCode::OK);
    let stopped = json_body(stopped).await;
    assert!(
        matches!(stopped["state"].as_str(), Some("error" | "cancelled")),
        "bounded stop must return a terminal state: {stopped}"
    );
    assert_eq!(
        fixture.child_requests.load(Ordering::SeqCst),
        initial_generation_requests * (1 + MAX_MANUAL_RETRIES as usize),
        "manual budget must be independent while automatic retry remains per generation"
    );
    mock.abort();
}
