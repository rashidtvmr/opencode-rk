//! Cancellation liveness when a child provider request hangs after a tool round.
//!
//! The fixture completes a real child tool dispatch, then blocks the provider
//! request for the next round.  Stopping the child must still settle the owned
//! producer and report a terminal `cancelled` state; the test deliberately does
//! not release the provider until after stop has returned.
use std::{env, future::IntoFuture, sync::{Arc, atomic::{AtomicUsize, Ordering}}, time::Duration};
use axum::{Router, Json, body::{Body, to_bytes}, extract::State, http::{Request, StatusCode}, routing::post};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{Value, json};
use tokio::sync::Notify;
use tower::ServiceExt;

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

struct Fixture {
    second_round: Notify,
    child_requests: AtomicUsize,
}

async fn provider(State(fixture): State<Arc<Fixture>>, Json(body): Json<Value>) -> ([(&'static str, &'static str); 1], String) {
    let items = body["input"].as_array().unwrap();
    let last_user = items.iter().rposition(|item| item["role"] == "user").unwrap();
    // Responses user content is commonly an array of typed content parts, not
    // a JSON string.  Use its JSON representation so the fixture recognizes
    // the actual wire envelope used by the turn engine.
    let prompt = items[last_user]["content"].to_string();
    let child = prompt.contains("CHILD_HANG_AFTER_TOOL");
    let has_output = items[last_user + 1..].iter().any(|item| item["type"] == "function_call_output");

    let event = if child && !has_output {
        fixture.child_requests.fetch_add(1, Ordering::SeqCst);
        json!({"type":"response.output_item.done","item":{
            "type":"function_call", "id":"fc_child_read", "call_id":"child_read",
            "name":"read", "arguments":json!({"path":"probe.txt"}).to_string()
        }})
    } else if child && has_output {
        fixture.child_requests.fetch_add(1, Ordering::SeqCst);
        // This is the deterministic provider fault: stop must not wait for it.
        fixture.second_round.notified().await;
        json!({"type":"response.output_text.delta","delta":"must not be reached"})
    } else if !has_output {
        json!({"type":"response.output_item.done","item":{
            "type":"function_call", "id":"fc_task", "call_id":"parent_task", "name":"task",
            "arguments":json!({"description":"hanging child","prompt":"CHILD_HANG_AFTER_TOOL: read then wait","subagent_type":"explore","background":true}).to_string()
        }})
    } else {
        json!({"type":"response.output_text.delta","delta":"background child started"})
    };
    ([ ("content-type", "text/event-stream") ], format!("data: {event}\n\ndata: {{\"type\":\"response.completed\",\"response\":{{\"status\":\"completed\"}}}}\n\n"))
}

fn request(method: &str, path: String, body: Value) -> Request<Body> {
    Request::builder().method(method).uri(path).header("content-type", "application/json")
        .body(Body::from(body.to_string())).unwrap()
}
async fn value(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
}
async fn poll(app: &Router, path: String, predicate: impl Fn(&Value) -> bool) -> Value {
    let mut last = Value::Null;
    match tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let result = value(app.clone().oneshot(request("GET", path.clone(), json!({}))).await.unwrap()).await;
            last = result.clone();
            if predicate(&result) { return result; }
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
    }).await {
        Ok(result) => result,
        Err(_) => panic!("fixture state must become observable: path={path}; last_value={last}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn child_stop_settles_when_provider_hangs_between_rounds() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("probe.txt"), "fixture child content").unwrap();
    let fixture = Arc::new(Fixture { second_round: Notify::new(), child_requests: AtomicUsize::new(0) });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let mock = tokio::spawn(axum::serve(listener, Router::new().fallback(post(provider)).with_state(fixture.clone())).into_future());
    let _env = [
        Env::set("OPENAI_BASE_URL", base), Env::set("OPENAI_API_KEY", "disposable-fixture-key"),
        Env::set("HOME", dir.path()), Env::set("XDG_CONFIG_HOME", dir.path()),
        Env::set("OPENCODE_RK_TURN_TOOLS", "task,read"),
    ];
    let sessions = SessionService::new(Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap()));
    let parent = sessions.create_in_directory("parent", Some(dir.path().into())).await.unwrap();
    let app = router(AppState { sessions: sessions.clone(), catalog: Arc::new(Catalog::default()) });

    let response = app.clone().oneshot(request("POST", format!("/api/sessions/{}/turns/stream", parent.id),
        json!({"text":"delegate hanging child", "model":"openai/fixture"}))).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let parent_body = tokio::spawn(async move { to_bytes(response.into_body(), 1024 * 1024).await.unwrap() });
    let permission = poll(&app, format!("/api/session/{}/permission", parent.id), |v| v["data"].as_array().is_some_and(|rows| !rows.is_empty())).await;
    let request_id = permission["data"][0]["id"].as_str().unwrap();
    let approved = app.clone().oneshot(request("POST", format!("/api/session/{}/permission/{request_id}/reply", parent.id), json!({"reply":"once"}))).await.unwrap();
    assert_eq!(approved.status(), StatusCode::NO_CONTENT);

    let listing = poll(&app, format!("/api/sessions/{}/children", parent.id), |v| v["children"].as_array().is_some_and(|rows| rows.first().is_some_and(|row| row["state"] == "running")) ).await;
    let child = listing["children"][0]["session_id"].as_str().unwrap().to_owned();
    // `read` is a safe, automatically admitted child tool under the fixture
    // policy; unlike the parent `task` admission it has no permission row to
    // approve.  Wait for the provider's second-round request instead.
    tokio::time::timeout(Duration::from_secs(8), async { while fixture.child_requests.load(Ordering::SeqCst) < 2 { tokio::task::yield_now().await; } }).await.unwrap();

    let stopped = tokio::time::timeout(Duration::from_secs(2), app.clone().oneshot(request("POST", format!("/api/sessions/{}/children/{child}/stop", parent.id), json!({}))))
        .await.expect("child stop must not wait for a hung provider").unwrap();
    assert_eq!(stopped.status(), StatusCode::OK);
    let stopped = value(stopped).await;
    assert_eq!(stopped["state"], "cancelled");
    let final_state = poll(&app, format!("/api/sessions/{}/children", parent.id), |v| v["children"].as_array().is_some_and(|rows| rows.first().is_some_and(|row| row["state"] == "cancelled"))).await;
    assert_eq!(final_state["children"][0]["state"], "cancelled");
    assert_eq!(fixture.child_requests.load(Ordering::SeqCst), 2, "stop must not replay the child provider/tool round");
    fixture.second_round.notify_waiters();
    let _ = tokio::time::timeout(Duration::from_secs(2), parent_body).await;
    mock.abort();
}
