//! Defined restart/resume outcomes for a completed delegated child.
//!
//! A real router creates and completes a foreground child against a disposable
//! on-disk `Storage`. The router and session service are then dropped and a new
//! router is built over the same disk state (a fresh daemon-lifetime child
//! registry). The test proves the defined outcome: durable root/child
//! transcripts, no provider replay, an explicitly current-daemon-empty child
//! listing, defined 404s for old-registry controls with no effects, and an old
//! child whose ordinary session history remains readable without being classed
//! as active. Automatic resumption/registry persistence is deliberately NOT
//! asserted because the product defines none. An installed OS process restart
//! is not simulated here and remains unproven.
use std::{env, future::IntoFuture, sync::{Arc, atomic::{AtomicUsize, Ordering}}, time::Duration};
use axum::{Router, Json, body::{Body, to_bytes}, extract::State, http::{Request, StatusCode}, routing::post};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::{Storage, StoragePaths};
use serde_json::{Value, json};
use tower::ServiceExt;

const REGISTRY_SCOPE: &str = "current daemon lifetime; child transcripts remain durable";

struct Env(&'static str, Option<std::ffi::OsString>);
impl Env { fn set(name: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
    let previous = env::var_os(name); env::set_var(name, value); Self(name, previous)
} }
impl Drop for Env { fn drop(&mut self) { if let Some(v) = &self.1 { env::set_var(self.0, v); } else { env::remove_var(self.0); } } }

#[derive(Default)]
struct Fixture { requests: AtomicUsize }

async fn provider(State(fixture): State<Arc<Fixture>>, Json(body): Json<Value>) -> ([(&'static str, &'static str);1], String) {
    fixture.requests.fetch_add(1, Ordering::SeqCst);
    let items = body["input"].as_array().unwrap();
    let last_user = items.iter().rposition(|item| item["role"] == "user").unwrap();
    let prompt = items[last_user]["content"].to_string();
    let output_done = items[last_user+1..].iter().any(|item| item["type"] == "function_call_output");
    let event = if prompt.contains("CHILD_FOREGROUND_COMPLETE") {
        assert!(!output_done, "fixture child must complete in one provider turn");
        json!({"type":"response.output_text.delta","delta":"child finished"})
    } else if !output_done {
        json!({"type":"response.output_item.done","item":{"type":"function_call","id":"fc_task","call_id":"parent_task","name":"task","arguments":json!({"description":"foreground probe","prompt":"CHILD_FOREGROUND_COMPLETE: finish now","subagent_type":"explore"}).to_string()}})
    } else {
        json!({"type":"response.output_text.delta","delta":"root finished"})
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
            let result = value(app.clone().oneshot(request("GET", path.clone(), json!({}))).await.unwrap()).await;
            if predicate(&result) { return result; }
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
    }).await.expect("fixture state must become observable")
}

#[tokio::test(flavor="multi_thread", worker_threads=2)]
async fn completed_child_history_survives_registry_restart_without_replay_or_old_controls() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = Arc::new(Fixture::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}",listener.local_addr().unwrap());
    let mock = tokio::spawn(axum::serve(listener, Router::new().fallback(post(provider)).with_state(fixture.clone())).into_future());
    let _env = [
        Env::set("OPENAI_BASE_URL", base),
        Env::set("OPENAI_API_KEY", "disposable-fixture-key"),
        Env::set("HOME", dir.path()),
        Env::set("XDG_CONFIG_HOME", dir.path()),
        Env::set("OPENCODE_CONFIG_DIR", dir.path().join("global-config")),
        Env::set("OPENCODE_RK_HOME", dir.path()),
        Env::set("OPENCODE_RK_PERMISSION_RULES", dir.path().join("permission-rules.json")),
        Env::set("OPENCODE_RK_TURN_TOOLS", "task"),
    ];

    // Router 1: disposable on-disk storage, one foreground child completed by a
    // real provider turn, then the parent turn completed by the fixture.
    let paths = StoragePaths::under(dir.path().join("data"));
    let storage = Arc::new(Storage::open(paths.clone()).unwrap());
    let sessions = SessionService::new(storage.clone());
    let parent = sessions.create_in_directory("restart root", Some(dir.path().into())).await.unwrap();
    let parent_id = parent.id.to_string();
    let app = router(AppState { sessions: sessions.clone(), catalog: Arc::new(Catalog::default()) });

    let response = app.clone().oneshot(request("POST", format!("/api/sessions/{parent_id}/turns/stream"),
        json!({"text":"delegate foreground probe", "model":"openai/fixture"}))).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let parent_body = tokio::spawn(async move { to_bytes(response.into_body(), 1024 * 1024).await.unwrap() });
    let permission = poll(&app, format!("/api/session/{parent_id}/permission"),
        |v| v["data"].as_array().is_some_and(|rows| !rows.is_empty())).await;
    let request_id = permission["data"][0]["id"].as_str().unwrap().to_owned();
    let approved = app.clone().oneshot(request("POST", format!("/api/session/{parent_id}/permission/{request_id}/reply"),
        json!({"reply":"once"}))).await.unwrap();
    assert_eq!(approved.status(), StatusCode::NO_CONTENT);
    let parent_body = tokio::time::timeout(Duration::from_secs(8), parent_body).await.unwrap().unwrap();
    let parent_events: Vec<Value> = parent_body.split(|byte| *byte == b'\n').filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap()).collect();
    assert!(!parent_events.iter().any(|event| event["type"] == "error"), "root turn must not fail: {parent_events:?}");
    assert!(parent_events.iter().any(|event| event["type"] == "assistant_message"), "{parent_events:?}");

    let listing = value(app.clone().oneshot(request("GET", format!("/api/sessions/{parent_id}/children"), json!({}))).await.unwrap()).await;
    assert_eq!(listing["registry_scope"], REGISTRY_SCOPE);
    assert_eq!(listing["children"][0]["state"], "completed", "foreground child must be completed before restart");
    let child_id = listing["children"][0]["session_id"].as_str().unwrap().to_owned();
    let requests_before_restart = fixture.requests.load(Ordering::SeqCst);
    assert!(requests_before_restart >= 3, "root admission, child turn and root continuation must each reach the provider");

    // Daemon registry reconstruction: drop the router, its session service and
    // the storage handle, then reopen the same disposable disk paths. The new
    // router owns a fresh `Children` registry (current-daemon scope only).
    drop(app);
    drop(sessions);
    drop(storage);
    let sessions = SessionService::new(Arc::new(Storage::open(paths).unwrap()));
    let app = router(AppState { sessions: sessions.clone(), catalog: Arc::new(Catalog::default()) });

    // Root and child transcripts are durable and readable through ordinary
    // session endpoints; neither read may touch the provider.
    let root = app.clone().oneshot(request("GET", format!("/api/sessions/{parent_id}"), json!({}))).await.unwrap();
    assert_eq!(root.status(), StatusCode::OK);
    let root = value(root).await;
    assert_eq!(root["session"]["id"].as_str(), Some(parent_id.as_str()));
    let root_messages = app.clone().oneshot(request("GET", format!("/api/sessions/{parent_id}/messages?limit=50"), json!({}))).await.unwrap();
    assert_eq!(root_messages.status(), StatusCode::OK);
    assert!(value(root_messages).await["messages"].as_array().is_some_and(|rows| !rows.is_empty()), "root transcript must survive restart");

    let child_session = app.clone().oneshot(request("GET", format!("/api/sessions/{child_id}"), json!({}))).await.unwrap();
    assert_eq!(child_session.status(), StatusCode::OK, "old child session history must remain accessible");
    let child_messages = app.clone().oneshot(request("GET", format!("/api/sessions/{child_id}/messages?limit=50"), json!({}))).await.unwrap();
    assert_eq!(child_messages.status(), StatusCode::OK);
    assert!(value(child_messages).await["messages"].as_array().is_some_and(|rows| !rows.is_empty()), "child transcript must survive restart");
    assert!(sessions.messages(child_id.parse().unwrap(), 100).await.unwrap().iter()
        .any(|message| message.role == opencode_rk_contracts::MessageRole::Assistant), "completed child assistant history must be durable");

    // The fresh daemon registry is explicitly empty: no silent resumption and
    // no silent "active" classification of the old child.
    let listing = value(app.clone().oneshot(request("GET", format!("/api/sessions/{parent_id}/children"), json!({}))).await.unwrap()).await;
    assert_eq!(listing["children"], json!([]), "old child must not be restored as an active/current-daemon child");
    assert_eq!(listing["registry_scope"], REGISTRY_SCOPE);
    assert_eq!(fixture.requests.load(Ordering::SeqCst), requests_before_restart, "restart reads must not replay provider work");

    // Old-registry controls hit the defined 404 and cause no durable/provider
    // effects: no replay, no replacement child, no history mutation.
    let child_messages_before = sessions.messages(child_id.parse().unwrap(), 100).await.unwrap().len();
    for (operation, body) in [
        ("stop", json!({})),
        ("retry", json!({})),
        ("continue", json!({"text":"resume old child"})),
    ] {
        let response = app.clone().oneshot(request("POST", format!("/api/sessions/{parent_id}/children/{child_id}/{operation}"), body)).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{operation} on a pre-restart child must return the defined 404");
        let response = value(response).await;
        assert!(response.to_string().contains("child task not found"), "{operation}: {response}");
    }
    assert_eq!(fixture.requests.load(Ordering::SeqCst), requests_before_restart, "old-registry controls must never replay provider work");
    assert_eq!(sessions.list(true).await.unwrap().len(), 2, "old-registry controls must never create a replacement child session");
    assert_eq!(sessions.messages(child_id.parse().unwrap(), 100).await.unwrap().len(), child_messages_before,
        "old-registry controls must not mutate durable child history");
    let listing = value(app.clone().oneshot(request("GET", format!("/api/sessions/{parent_id}/children"), json!({}))).await.unwrap()).await;
    assert_eq!(listing["children"], json!([]), "failed old-registry controls must not resurrect the child");

    mock.abort();
}
