//! Fallback compaction must not replace an established checkpoint when its
//! ordinary Responses request fails, is interrupted, or races an append.
use axum::{body::Body, extract::Json, http::{Request, StatusCode}, routing::post, Router};
use opencode_rk_catalog::Catalog;
use opencode_rk_contracts::{MessageRole, SessionSummary};
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::{Storage, StoragePaths};
use serde_json::{json, Value};
use std::{env, sync::{atomic::{AtomicUsize, Ordering}, Arc}, time::Duration};
use tempfile::tempdir;
use tokio::sync::{mpsc, Mutex, Notify};
use tower::ServiceExt;

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

struct EnvGuard(Vec<(&'static str, Option<std::ffi::OsString>)>);
impl EnvGuard {
    fn set(values: &[(&'static str, String)]) -> Self {
        Self(values.iter().map(|(key, value)| {
            let old = env::var_os(key); env::set_var(key, value); (*key, old)
        }).collect())
    }
}
impl Drop for EnvGuard {
    fn drop(&mut self) { for (key, old) in &self.0 { match old { Some(v) => env::set_var(key, v), None => env::remove_var(key) } } }
}
struct Task(tokio::task::JoinHandle<()>);
impl Drop for Task { fn drop(&mut self) { self.0.abort(); } }

#[derive(Clone, Copy, Debug)]
enum Mode { Failure, Interrupt, Append }

async fn provider(
    axum::extract::State((mode, calls, gate, seen)): axum::extract::State<(Mode, Arc<AtomicUsize>, Arc<Notify>, mpsc::Sender<Value>)>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let call = calls.fetch_add(1, Ordering::SeqCst);
    seen.send(body).await.unwrap();
    if call == 0 { return (StatusCode::OK, Json(json!({"id":"initial","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"INITIAL_SUMMARY"}]}]}))); }
    gate.notified().await;
    if matches!(mode, Mode::Failure) { return (StatusCode::BAD_REQUEST, Json(json!({"error":{"message":"deliberate fallback failure"}}))); }
    (StatusCode::OK, Json(json!({"id":"second","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"MUST_NOT_COMMIT"}]}]})))
}

async fn post_json(app: Router, uri: String, body: Value) -> axum::response::Response {
    app.oneshot(Request::builder().method("POST").uri(uri).header("content-type", "application/json").body(Body::from(body.to_string())).unwrap()).await.unwrap()
}

async fn session(root: &std::path::Path) -> (SessionService, SessionSummary) {
    let service = SessionService::new(Arc::new(Storage::open(StoragePaths::under(root.join("storage"))).unwrap()));
    let summary = service.create_in_directory("fallback adversarial", Some(root.to_owned())).await.unwrap();
    (service, summary)
}

#[tokio::test]
async fn fallback_failure_interrupt_and_append_preserve_checkpoint_atomically() {
    let _lock = ENV_LOCK.lock().await;
    for mode in [Mode::Failure, Mode::Interrupt, Mode::Append] {
        let root = tempdir().unwrap();
        let (seen_tx, mut seen_rx) = mpsc::channel(8);
        let calls = Arc::new(AtomicUsize::new(0));
        let gate = Arc::new(Notify::new());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let fixture = Router::new().route("/responses", post(provider)).with_state((mode, calls.clone(), gate.clone(), seen_tx));
        let _server = Task(tokio::spawn(async move { axum::serve(listener, fixture).await.unwrap(); }));
        let _env = EnvGuard::set(&[("OPENAI_BASE_URL", format!("http://{address}")), ("OPENAI_API_KEY", "disposable-fallback-adversarial".into()), ("OPENCODE_RK_HOME", root.path().display().to_string()), ("OPENCODE_RK_COMPACTION__KEEP_RECENT_TURNS", "2".into())]);
        let (sessions, current) = session(root.path()).await;
        sessions.append_text(current.id, MessageRole::User, "durable question").await.unwrap();
        sessions.append_text(current.id, MessageRole::Assistant, "durable answer").await.unwrap();
        let app = router(AppState { sessions: sessions.clone(), catalog: Arc::new(Catalog::default()) });
        assert_eq!(post_json(app.clone(), format!("/api/sessions/{}/compact", current.id), json!({"model":"openai/fixture","strategy":"fallback"})).await.status(), StatusCode::OK);
        let _ = tokio::time::timeout(Duration::from_secs(2), seen_rx.recv()).await.unwrap().unwrap();
        let checkpoint_before = sessions.compaction_checkpoint(current.id).await.unwrap().unwrap();
        sessions.append_text(current.id, MessageRole::User, "tail question").await.unwrap();
        sessions.append_text(current.id, MessageRole::Assistant, "tail answer").await.unwrap();
        let history_before = sessions.messages(current.id, 20).await.unwrap();
        let second_app = app.clone();
        let second = tokio::spawn(async move { post_json(second_app, format!("/api/sessions/{}/compact", current.id), json!({"model":"openai/fixture","strategy":"fallback"})).await });
        tokio::time::timeout(Duration::from_secs(2), seen_rx.recv()).await.unwrap().unwrap();
        match mode {
            Mode::Interrupt => { assert_eq!(post_json(app.clone(), format!("/api/session/{}/interrupt", current.id), json!({})).await.status(), StatusCode::NO_CONTENT); }
            Mode::Append => { assert_eq!(post_json(app.clone(), format!("/api/sessions/{}/messages", current.id), json!({"text":"external concurrent message"})).await.status(), StatusCode::CREATED); }
            Mode::Failure => {}
        }
        gate.notify_waiters();
        let response = tokio::time::timeout(Duration::from_secs(2), second).await.unwrap().unwrap();
        assert!(!response.status().is_success(), "mode must fail: {mode:?}");
        let checkpoint_after = sessions.compaction_checkpoint(current.id).await.unwrap().unwrap();
        assert_eq!(checkpoint_after, checkpoint_before);
        let history_after = sessions.messages(current.id, 20).await.unwrap();
        if matches!(mode, Mode::Append) { assert_eq!(&history_after[..history_before.len()], history_before.as_slice()); assert_eq!(history_after.len(), history_before.len() + 1); } else { assert_eq!(history_after, history_before); }
    }
}
