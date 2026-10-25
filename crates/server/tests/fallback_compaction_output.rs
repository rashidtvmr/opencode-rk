//! A fallback summary is usable only when the Responses API completed with
//! plain assistant text.  Partial/error responses and tool-bearing/empty
//! completed responses must not mutate the session.

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    response::Response,
    routing::post,
    Json, Router,
};
use opencode_rk_catalog::Catalog;
use opencode_rk_contracts::{MessageRole, SessionSummary};
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::{Storage, StoragePaths};
use serde_json::{json, Value};
use std::{
    env,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tempfile::tempdir;
use tokio::sync::{mpsc, Mutex};
use tower::ServiceExt;

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

struct EnvGuard(Vec<(&'static str, Option<std::ffi::OsString>)>);

impl EnvGuard {
    fn set(values: &[(&'static str, String)]) -> Self {
        Self(values
            .iter()
            .map(|(key, value)| {
                let old = env::var_os(key);
                env::set_var(key, value);
                (*key, old)
            })
            .collect())
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, old) in &self.0 {
            match old {
                Some(value) => env::set_var(key, value),
                None => env::remove_var(key),
            }
        }
    }
}

struct Task(tokio::task::JoinHandle<()>);

impl Drop for Task {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[derive(Clone, Copy, Debug)]
enum OutputMode {
    Incomplete,
    Failed,
    CompletedWithFunctionCall,
    CompletedWithEmptyText,
}

#[derive(Clone)]
struct Fixture {
    mode: OutputMode,
    requests: mpsc::Sender<Value>,
    response_calls: Arc<AtomicUsize>,
    compact_calls: Arc<AtomicUsize>,
}

async fn responses(
    State(fixture): State<Fixture>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    fixture.response_calls.fetch_add(1, Ordering::SeqCst);
    fixture
        .requests
        .send(body)
        .await
        .expect("bounded fixture receiver must remain live");

    let response = match fixture.mode {
        OutputMode::Incomplete => json!({
            "id": "incomplete",
            "status": "incomplete",
            "output": [{"type":"message", "role":"assistant", "content":[{"type":"output_text", "text":"PARTIAL_SUMMARY"}]}]
        }),
        OutputMode::Failed => json!({
            "id": "failed",
            "status": "failed",
            "error": {"message":"fixture failure"},
            "output": [{"type":"message", "role":"assistant", "content":[{"type":"output_text", "text":"PARTIAL_SUMMARY"}]}]
        }),
        OutputMode::CompletedWithFunctionCall => json!({
            "id": "tool",
            "status": "completed",
            "output": [
                {"type":"function_call", "id":"fc_summary", "call_id":"summary", "name":"read", "arguments":"{}"},
                {"type":"message", "role":"assistant", "content":[{"type":"output_text", "text":"TOOL_SUMMARY"}]}
            ]
        }),
        OutputMode::CompletedWithEmptyText => json!({
            "id": "empty",
            "status": "completed",
            "output": [{"type":"message", "role":"assistant", "content":[{"type":"output_text", "text":""}]}]
        }),
    };
    (StatusCode::OK, Json(response))
}

async fn compact(State(fixture): State<Fixture>, Json(_body): Json<Value>) -> StatusCode {
    fixture.compact_calls.fetch_add(1, Ordering::SeqCst);
    StatusCode::INTERNAL_SERVER_ERROR
}

async fn post_json(app: Router, uri: String, body: Value) -> Response {
    app.oneshot(
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn make_session(root: &std::path::Path) -> (SessionService, SessionSummary) {
    let sessions = SessionService::new(Arc::new(
        Storage::open(StoragePaths::under(root.join("storage"))).unwrap(),
    ));
    let session = sessions
        .create_in_directory("fallback output fixture", Some(root.to_owned()))
        .await
        .unwrap();
    (sessions, session)
}

#[tokio::test]
async fn fallback_rejects_non_plain_completed_summary_output_without_mutation() {
    let _lock = ENV_LOCK.lock().await;

    for mode in [
        OutputMode::Incomplete,
        OutputMode::Failed,
        OutputMode::CompletedWithFunctionCall,
        OutputMode::CompletedWithEmptyText,
    ] {
        let root = tempdir().unwrap();
        let (request_tx, mut request_rx) = mpsc::channel(1);
        let response_calls = Arc::new(AtomicUsize::new(0));
        let compact_calls = Arc::new(AtomicUsize::new(0));
        let fixture = Fixture {
            mode,
            requests: request_tx,
            response_calls: response_calls.clone(),
            compact_calls: compact_calls.clone(),
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let provider = Router::new()
            .route("/responses", post(responses))
            .route("/responses/compact", post(compact))
            .with_state(fixture);
        let _server = Task(tokio::spawn(async move {
            axum::serve(listener, provider).await.unwrap();
        }));
        let _env = EnvGuard::set(&[
            ("OPENAI_BASE_URL", format!("http://{address}")),
            ("OPENAI_API_KEY", "disposable-fallback-output".into()),
            ("OPENCODE_RK_HOME", root.path().display().to_string()),
        ]);

        let (sessions, session) = make_session(root.path()).await;
        sessions
            .append_text(session.id, MessageRole::User, "history question")
            .await
            .unwrap();
        sessions
            .append_text(session.id, MessageRole::Assistant, "history answer")
            .await
            .unwrap();
        let before = sessions.messages(session.id, 20).await.unwrap();
        let app = router(AppState {
            sessions: sessions.clone(),
            catalog: Arc::new(Catalog::default()),
        });

        let response = tokio::time::timeout(
            Duration::from_secs(2),
            post_json(
                app,
                format!("/api/sessions/{}/compact", session.id),
                json!({"model":"openai/fixture", "strategy":"fallback"}),
            ),
        )
        .await
        .expect("manual fallback must settle");
        assert!(!response.status().is_success(), "invalid output must fail: {mode:?}");

        let request = tokio::time::timeout(Duration::from_secs(2), request_rx.recv())
            .await
            .expect("provider request must arrive")
            .expect("provider request channel must contain one request");
        assert_eq!(request["tools"], json!([]), "summary must advertise no tools");
        assert_eq!(response_calls.load(Ordering::SeqCst), 1);
        assert_eq!(compact_calls.load(Ordering::SeqCst), 0, "fallback must not call native route");
        assert!(tokio::time::timeout(Duration::from_millis(100), request_rx.recv())
            .await
            .is_err(), "provider queue must remain bounded and empty");
        assert_eq!(sessions.messages(session.id, 20).await.unwrap(), before);
        assert!(sessions
            .compaction_checkpoint(session.id)
            .await
            .unwrap()
            .is_none());
    }
}
