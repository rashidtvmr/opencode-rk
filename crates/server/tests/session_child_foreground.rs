//! Foreground child admission regression: a delegating root must be able to
//! start its real child turn while every root lane is occupied, even though
//! the parent stays blocked in its task tool waiting for that child.
use std::{env, future::IntoFuture, time::Duration};
use axum::{Router, Json, body::{Body, to_bytes}, extract::State, http::{Request, StatusCode}, routing::post};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{Value, json};
use tokio::sync::{mpsc, Semaphore};
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
        if let Some(value) = &self.1 { env::set_var(self.0, value); } else { env::remove_var(self.0); }
    }
}

/// Deterministic provider signalling: channel events announce every relevant
/// provider request; the held second root waits on a permit, never a timer.
struct Fixture {
    seen: mpsc::UnboundedSender<&'static str>,
    second_root_gate: Semaphore,
}

async fn provider(State(fixture): State<std::sync::Arc<Fixture>>, Json(body): Json<Value>) -> ([(&'static str, &'static str); 1], String) {
    let input = body["input"].to_string();
    let items = body["input"].as_array().unwrap();
    let last_user = items.iter().rposition(|item| item["role"] == "user").unwrap();
    let text = items[last_user]["content"].to_string();
    let output = items[last_user + 1..].iter().any(|item| item["type"] == "function_call_output");
    let event = if text.contains("CHILD_FOREGROUND_PROBE") {
        assert!(!body["tools"].as_array().is_some_and(|tools| tools.iter().any(|tool| tool["name"] == "task")), "children must not delegate recursively");
        let _ = fixture.seen.send("child");
        json!({"type":"response.output_text.delta","delta":"foreground child result"})
    } else if text.contains("HOLD_SECOND_ROOT") {
        let _ = fixture.seen.send("second_root");
        fixture.second_root_gate.acquire().await.unwrap().forget();
        json!({"type":"response.output_text.delta","delta":"second root completed"})
    } else if !output {
        json!({"type":"response.output_item.done","item":{"type":"function_call","id":"fc_task","call_id":"parent_task","name":"task","arguments":json!({"description":"foreground probe","prompt":"CHILD_FOREGROUND_PROBE: report a value","subagent_type":"explore"}).to_string()}})
    } else {
        assert!(input.contains("foreground child result"), "child result must return to the parent continuation: {input}");
        json!({"type":"response.output_text.delta","delta":"parent received child result"})
    };
    ([("content-type", "text/event-stream")], format!("data: {event}\n\ndata: {{\"type\":\"response.completed\",\"response\":{{\"status\":\"completed\"}}}}\n\n"))
}

fn request(method: &str, path: String, body: Value) -> Request<Body> {
    Request::builder().method(method).uri(path).header("content-type", "application/json").body(Body::from(body.to_string())).unwrap()
}

async fn value(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
}

async fn permission_id(app: &Router, session: &str) -> String {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let listing = value(app.clone().oneshot(request("GET", format!("/api/session/{session}/permission"), json!({}))).await.unwrap()).await;
            if let Some(id) = listing["data"].as_array().and_then(|rows| rows.first()).and_then(|row| row["id"].as_str()) {
                return id.to_owned();
            }
            tokio::task::yield_now().await;
        }
    }).await.expect("task permission must be requested")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn foreground_child_starts_when_root_lanes_saturated() {
    let dir = tempfile::tempdir().unwrap();
    let (seen_tx, mut seen_rx) = mpsc::unbounded_channel();
    let fixture = std::sync::Arc::new(Fixture { seen: seen_tx, second_root_gate: Semaphore::new(0) });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let mock = tokio::spawn(axum::serve(listener, Router::new().fallback(post(provider)).with_state(fixture.clone())).into_future());
    let _env = [
        Env::set("OPENAI_BASE_URL", base),
        Env::set("OPENAI_API_KEY", "disposable-fixture-key"),
        Env::set("HOME", dir.path()),
        Env::set("XDG_CONFIG_HOME", dir.path()),
        Env::set("OPENCODE_RK_HOME", dir.path()),
        Env::set("OPENCODE_RK_PERMISSION_RULES", dir.path().join("permission-rules.json")),
        Env::set("OPENCODE_RK_TURN_TOOLS", "task"),
    ];
    let sessions = SessionService::new(std::sync::Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap()));
    let parent = sessions.create_in_directory("parent", Some(dir.path().into())).await.unwrap();
    let held = sessions.create_in_directory("held root", Some(dir.path().into())).await.unwrap();
    let app = router(AppState { sessions: sessions.clone(), catalog: std::sync::Arc::new(Catalog::default()) });

    tokio::time::timeout(Duration::from_secs(15), async {
    let parent_response = app.clone().oneshot(request("POST", format!("/api/sessions/{}/turns/stream", parent.id), json!({"text":"delegate foreground probe","model":"openai/fixture"}))).await.unwrap();
    assert_eq!(parent_response.status(), StatusCode::CREATED);
    let parent_body = tokio::spawn(async move { to_bytes(parent_response.into_body(), 1024 * 1024).await.unwrap() });

    let request_id = permission_id(&app, &parent.id.to_string()).await;

    // Second root occupies the remaining root lane. Spawn the whole request so
    // this test never waits for response headers the held provider withholds.
    let held_path = format!("/api/sessions/{}/turns/stream", held.id);
    let held_app = app.clone();
    let held_body = tokio::spawn(async move {
        let response = held_app.oneshot(request("POST", held_path, json!({"text":"HOLD_SECOND_ROOT","model":"openai/fixture"}))).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED, "held root must be admitted");
        to_bytes(response.into_body(), 1024 * 1024).await.unwrap()
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match seen_rx.recv().await.expect("fixture channel closed") {
                "second_root" => break,
                _ => continue,
            }
        }
    }).await.expect("second root must be admitted and held in its provider request");

    // Both root lanes are now occupied. Approval must still start the child.
    let approved = app.clone().oneshot(request("POST", format!("/api/session/{}/permission/{request_id}/reply", parent.id), json!({"reply":"once"}))).await.unwrap();
    assert_eq!(approved.status(), StatusCode::NO_CONTENT);
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            match seen_rx.recv().await.expect("fixture channel closed") {
                "child" => break,
                _ => continue,
            }
        }
    }).await.expect("foreground child must run while both root lanes are held");

    // Only now free the held root: the child admission above was proved under
    // full root saturation, not after a root released.
    fixture.second_root_gate.add_permits(1);
    let held_bytes = tokio::time::timeout(Duration::from_secs(5), held_body).await.unwrap().unwrap();
    assert!(String::from_utf8_lossy(&held_bytes).contains("second root completed"));
    let parent_bytes = tokio::time::timeout(Duration::from_secs(5), parent_body).await.unwrap().unwrap();
    let parent_text = String::from_utf8_lossy(&parent_bytes);
    assert!(!parent_text.contains("\"type\":\"error\""), "{parent_text}");
    assert!(parent_text.contains("\"type\":\"assistant_message\""), "{parent_text}");
    assert!(parent_text.contains("parent received child result"), "child result must return to the parent turn: {parent_text}");
    }).await.expect("foreground journey must complete within the bounded outer timeout");
    mock.abort();
}
