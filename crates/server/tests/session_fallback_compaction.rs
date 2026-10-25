//! 004-D: fallback compaction uses an ordinary Responses request, not the
//! provider-native `/responses/compact` capability.  These are real loopback
//! HTTP fixtures and deliberately start RED while only native compaction is
//! accepted by the server.

use axum::{
    body::{to_bytes, Body},
    extract::Json,
    http::{Request, StatusCode},
    routing::post,
    Router,
};
use opencode_rk_catalog::Catalog;
use opencode_rk_contracts::MessageRole;
use opencode_rk_contracts::SessionSummary;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::{Storage, StoragePaths};
use serde_json::{json, Value};
use std::{
    env,
    sync::{atomic::{AtomicBool, AtomicUsize, Ordering}, Arc},
    time::Duration,
};
use tempfile::tempdir;
use tokio::sync::mpsc;
use tower::ServiceExt;

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct EnvGuard(Vec<(&'static str, Option<std::ffi::OsString>)>);
impl EnvGuard {
    fn set(values: &[(&'static str, String)]) -> Self {
        Self(values.iter().map(|(key, value)| {
            let old = env::var_os(key);
            env::set_var(key, value);
            (*key, old)
        }).collect())
    }
}
impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, old) in &self.0 {
            match old { Some(value) => env::set_var(key, value), None => env::remove_var(key) }
        }
    }
}

struct Task(tokio::task::JoinHandle<()>);
impl Drop for Task { fn drop(&mut self) { self.0.abort(); } }

#[derive(Clone)]
struct Fixture {
    requests: mpsc::Sender<Value>,
    calls: Arc<AtomicUsize>,
    fail: Arc<AtomicBool>,
}

async fn ordinary_response(fixture: axum::extract::State<Fixture>, Json(body): Json<Value>) -> (StatusCode, Json<Value>) {
    fixture.calls.fetch_add(1, Ordering::SeqCst);
    let _ = fixture.requests.send(body.clone()).await;
    if fixture.fail.load(Ordering::SeqCst) {
        return (StatusCode::BAD_GATEWAY, Json(json!({"error":{"message":"fallback fixture failure"}})));
    }
    let input = body["input"].as_array().cloned().unwrap_or_default();
    let is_summary = input.iter().any(|item| item.to_string().to_ascii_lowercase().contains("compaction summary")
        || item.to_string().contains("Summarize"));
    let text = if is_summary { "FALLBACK_SUMMARY_MARKER" } else { "FALLBACK_FINAL_MARKER" };
    (StatusCode::OK, Json(json!({
        "id": if is_summary { "fallback-summary" } else { "fallback-final" },
        "status":"completed",
        "output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":text}]}]
    })))
}

async fn fixture_server(fixture: Fixture) -> (String, Task) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new().route("/responses", post(ordinary_response)).with_state(fixture);
    let task = Task(tokio::spawn(async move { axum::serve(listener, app).await.unwrap(); }));
    (format!("http://{address}"), task)
}

async fn make_session(root: &std::path::Path) -> (SessionService, SessionSummary) {
    let sessions = SessionService::new(Arc::new(Storage::open(StoragePaths::under(root.join("storage"))).unwrap()));
    let session = sessions.create_in_directory("fallback fixture", Some(root.to_owned())).await.unwrap();
    (sessions, session)
}

async fn post_json(app: axum::Router, uri: String, body: Value) -> axum::response::Response {
    app.oneshot(Request::builder().method("POST").uri(uri).header("content-type", "application/json")
        .body(Body::from(body.to_string())).unwrap()).await.unwrap()
}

#[tokio::test]
async fn fallback_manual_compaction_restart_keeps_durable_history_and_bounds_next_input() {
    let _lock = ENV_LOCK.lock().await;
    let root = tempdir().unwrap();
    std::fs::write(root.path().join("AGENTS.md"), "FALLBACK_AGENTS_INSTRUCTIONS").unwrap();
    let (tx, mut rx) = mpsc::channel(8);
    let fixture = Fixture { requests: tx, calls: Arc::new(AtomicUsize::new(0)), fail: Arc::new(AtomicBool::new(false)) };
    let (base, _provider) = fixture_server(fixture).await;
    let _env = EnvGuard::set(&[("OPENAI_BASE_URL", base), ("OPENAI_API_KEY", "disposable-fallback-key".into()), ("OPENCODE_RK_HOME", root.path().display().to_string()), ("OPENCODE_RK_COMPACTION__KEEP_RECENT_TURNS", "2".into())]);
    let (sessions, session) = make_session(root.path()).await;
    for i in 0..100 { sessions.append_text(session.id, if i % 2 == 0 { MessageRole::User } else { MessageRole::Assistant }, format!("old-{i}")).await.unwrap(); }
    let app = router(AppState { sessions: sessions.clone(), catalog: Arc::new(Catalog::default()) });
    let response = post_json(app, format!("/api/sessions/{}/compact", session.id), json!({"model":"openai/fixture","strategy":"fallback"})).await;
    assert_eq!(response.status(), StatusCode::OK, "fallback must be accepted");
    let summary_request = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await.unwrap().unwrap();
    assert_eq!(summary_request["stream"], false);
    assert_eq!(summary_request["tools"], json!([]));
    assert!(summary_request["max_output_tokens"].as_u64().is_some_and(|limit| limit <= 4096));
    assert!(summary_request["input"].to_string().contains("old-0"));
    assert!(summary_request["input"].to_string().contains("old-99"));
    assert!(summary_request.to_string().contains("compaction summary") || summary_request.to_string().contains("Summarize"));
    assert_eq!(sessions.messages(session.id, 200).await.unwrap().len(), 100);
    drop(sessions);
    let (reopened, _) = make_session(root.path()).await;
    let app = router(AppState { sessions: reopened.clone(), catalog: Arc::new(Catalog::default()) });
    let turn = post_json(app, format!("/api/sessions/{}/turns", session.id), json!({"model":"openai/fixture","text":"latest user"})).await;
    assert_eq!(turn.status(), StatusCode::CREATED);
    let next = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await.unwrap().unwrap();
    let input = next["input"].as_array().unwrap();
    assert!(input.iter().any(|v| v.to_string().contains("FALLBACK_SUMMARY_MARKER")));
    assert!(input.iter().any(|v| v["content"] == "old-98"));
    assert!(input.iter().any(|v| v["content"] == "old-99"));
    assert!(input.iter().any(|v| v["content"] == "latest user"));
    assert!(!input.iter().any(|v| v.to_string().contains("old-0")));
    assert!(input.iter().any(|v| v["role"] == "system" && v["content"].as_str().unwrap().contains("FALLBACK_AGENTS_INSTRUCTIONS")));
    assert_eq!(reopened.messages(session.id, 200).await.unwrap().len(), 102);
}

#[tokio::test]
async fn fallback_provider_error_preserves_checkpoint_and_history() {
    let _lock = ENV_LOCK.lock().await;
    let root = tempdir().unwrap();
    let (tx, mut rx) = mpsc::channel(8);
    let fixture = Fixture { requests: tx, calls: Arc::new(AtomicUsize::new(0)), fail: Arc::new(AtomicBool::new(true)) };
    let (base, _provider) = fixture_server(fixture).await;
    let _env = EnvGuard::set(&[("OPENAI_BASE_URL", base), ("OPENAI_API_KEY", "failure-key".into()), ("OPENCODE_RK_HOME", root.path().display().to_string())]);
    let (sessions, session) = make_session(root.path()).await;
    sessions.append_text(session.id, MessageRole::User, "before").await.unwrap();
    sessions.append_text(session.id, MessageRole::Assistant, "answer").await.unwrap();
    let before = sessions.messages(session.id, 20).await.unwrap();
    let app = router(AppState { sessions: sessions.clone(), catalog: Arc::new(Catalog::default()) });
    let response = post_json(app, format!("/api/sessions/{}/compact", session.id), json!({"model":"openai/fixture","strategy":"fallback"})).await;
    assert!(!response.status().is_success());
    let _ = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await.unwrap().unwrap();
    assert_eq!(sessions.messages(session.id, 20).await.unwrap(), before);
    assert!(sessions.compaction_checkpoint(session.id).await.unwrap().is_none());
}

#[tokio::test]
async fn automatic_fallback_settings_compact_before_append_and_disabled_does_no_summary_io() {
    let _lock = ENV_LOCK.lock().await;
    for enabled in ["true", "false"] {
        let root = tempdir().unwrap();
        let (tx, mut rx) = mpsc::channel(8);
        let fixture = Fixture { requests: tx, calls: Arc::new(AtomicUsize::new(0)), fail: Arc::new(AtomicBool::new(false)) };
        let (base, _provider) = fixture_server(fixture).await;
        let _env = EnvGuard::set(&[("OPENAI_BASE_URL", base), ("OPENAI_API_KEY", "auto-key".into()), ("OPENCODE_RK_HOME", root.path().display().to_string()), ("OPENCODE_RK_COMPACTION__ENABLED", enabled.into()), ("OPENCODE_RK_COMPACTION__STRATEGY", "fallback".into()), ("OPENCODE_RK_COMPACTION__THRESHOLD_TOKENS", "1".into()), ("OPENCODE_RK_COMPACTION__KEEP_RECENT_TURNS", "2".into())]);
        let (sessions, session) = make_session(root.path()).await;
        // Leave room under the provider's 100-item input cap for the new user
        // message and system instructions on the ordinary response request.
        for i in 0..96 { sessions.append_text(session.id, if i % 2 == 0 { MessageRole::User } else { MessageRole::Assistant }, format!("head-{i}")).await.unwrap(); }
        let app = router(AppState { sessions: sessions.clone(), catalog: Arc::new(Catalog::default()) });
        let response = post_json(app, format!("/api/sessions/{}/turns", session.id), json!({"model":"openai/fixture","text":"automatic latest"})).await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let first = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await.unwrap().unwrap();
        if enabled == "true" {
            assert!(first.to_string().contains("Summarize") || first.to_string().contains("compaction summary"));
            let second = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await.unwrap().unwrap();
            assert!(second["input"].to_string().contains("automatic latest"));
        } else {
            assert!(!first.to_string().contains("Summarize") && !first.to_string().contains("compaction summary"));
            assert!(tokio::time::timeout(Duration::from_millis(100), rx.recv()).await.is_err());
        }
    }
}
