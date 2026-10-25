//! 004-D: fallback compaction must also feed the native streaming turn path.

use std::{
    env,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{
    Router,
    body::{Body, to_bytes},
    extract::Json,
    http::{Request, StatusCode},
    routing::post,
};
use opencode_rk_catalog::Catalog;
use opencode_rk_contracts::MessageRole;
use opencode_rk_server::{AppState, router};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::{Storage, StoragePaths};
use serde_json::{Value, json};
use tempfile::tempdir;
use tokio::sync::mpsc;
use tower::ServiceExt;

struct EnvGuard(Vec<(&'static str, Option<std::ffi::OsString>)>);
static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

impl EnvGuard {
    fn set(values: &[(&'static str, String)]) -> Self {
        Self(
            values
                .iter()
                .map(|(key, value)| {
                    let old = env::var_os(key);
                    env::set_var(key, value);
                    (*key, old)
                })
                .collect(),
        )
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

#[tokio::test]
async fn fallback_compaction_streaming_turn_preserves_summary_tail_and_typed_todos() {
    let _lock = ENV_LOCK.lock().await;
    let root = tempdir().unwrap();
    std::fs::write(root.path().join("AGENTS.md"), "FALLBACK_STREAM_AGENTS").unwrap();
    let (tx, mut rx) = mpsc::channel::<Value>(8);
    let calls = Arc::new(AtomicUsize::new(0));
    let server_calls = calls.clone();
    let provider = Router::new().route("/responses", post(move |Json(body): Json<Value>| {
        let tx = tx.clone();
        let call = server_calls.fetch_add(1, Ordering::SeqCst);
        async move {
            tx.send(body.clone()).await.unwrap();
            if body["stream"] != true {
                return axum::response::Response::new(Body::from(json!({
                    "id":"fallback-summary-response", "status":"completed",
                    "output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"FALLBACK_STREAM_SUMMARY_MARKER"}]}]
                }).to_string()));
            }
            let event = if call == 1 {
                json!({"type":"response.output_item.done","item":{"type":"function_call","id":"fc_fallback_todo","call_id":"call_fallback_todo","name":"todowrite","arguments":json!({"todos":[{"id":"fallback-stream-todo","content":"Keep streamed fallback state","status":"in_progress","priority":"high"}]}).to_string()}})
            } else {
                json!({"type":"response.output_text.delta","delta":"FALLBACK_STREAM_FINAL_MARKER"})
            };
            let done = json!({"type":"response.completed","response":{"id":"fallback-stream-turn","status":"completed"}});
            let mut response = axum::response::Response::new(Body::from(format!("data: {event}\n\ndata: {done}\n\n")));
            response.headers_mut().insert("content-type", "text/event-stream".parse().unwrap());
            response
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let _task = Task(tokio::spawn(async move {
        axum::serve(listener, provider).await.unwrap();
    }));
    let _env = EnvGuard::set(&[
        ("OPENAI_BASE_URL", format!("http://{address}")),
        ("OPENAI_API_KEY", "disposable-fallback-stream-key".into()),
        ("OPENCODE_RK_HOME", root.path().display().to_string()),
        ("OPENCODE_RK_TURN_TOOLS", "todowrite".into()),
        ("OPENCODE_RK_COMPACTION__KEEP_RECENT_TURNS", "1".into()),
    ]);
    let sessions = SessionService::new(Arc::new(
        Storage::open(StoragePaths::under(root.path().join("storage"))).unwrap(),
    ));
    let session = sessions
        .create_in_directory("fallback streaming", Some(root.path().to_owned()))
        .await
        .unwrap();
    for (role, text) in [
        (MessageRole::User, "fallback old user one"),
        (MessageRole::Assistant, "fallback old assistant one"),
        (MessageRole::User, "fallback recent user"),
        (MessageRole::Assistant, "fallback recent assistant"),
    ] {
        sessions.append_text(session.id, role, text).await.unwrap();
    }
    let app = router(AppState {
        sessions: sessions.clone(),
        catalog: Arc::new(Catalog::default()),
    });
    let compact = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/compact", session.id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"model":"openai/fixture","strategy":"fallback"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(compact.status(), StatusCode::OK);
    let summary_request = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(summary_request["stream"], false);
    assert_eq!(summary_request["tools"], json!([]));
    assert!(
        summary_request["input"]
            .to_string()
            .contains("fallback old user one")
    );
    assert!(
        summary_request["input"]
            .to_string()
            .contains("fallback recent assistant")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let stream = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/turns/stream", session.id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"model":"openai/fixture","text":"fallback new user"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(stream.status().is_success());
    let body = to_bytes(stream.into_body(), 256 * 1024).await.unwrap();
    let visible = String::from_utf8_lossy(&body);
    assert!(visible.contains("FALLBACK_STREAM_FINAL_MARKER"));
    assert!(!visible.contains("FALLBACK_STREAM_SUMMARY_MARKER"));
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    let first = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    let second = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first["stream"], true);
    assert_eq!(second["stream"], true);
    let first_input = first["input"].as_array().unwrap();
    assert!(
        first_input
            .iter()
            .any(|v| v.to_string().contains("FALLBACK_STREAM_SUMMARY_MARKER"))
    );
    assert!(
        first_input
            .iter()
            .any(|v| v["content"] == "fallback recent assistant")
    );
    assert!(
        first_input
            .iter()
            .any(|v| v["content"] == "fallback new user")
    );
    assert!(first_input.iter().any(|v| {
        v["role"] == "system"
            && v["content"]
                .as_str()
                .unwrap()
                .contains("FALLBACK_STREAM_AGENTS")
    }));
    assert!(
        !first_input
            .iter()
            .any(|v| v.to_string().contains("fallback old user one"))
    );
    let second_input = second["input"].as_array().unwrap();
    assert!(
        second_input
            .iter()
            .any(|v| v["type"] == "function_call" && v["call_id"] == "call_fallback_todo")
    );
    assert!(
        second_input
            .iter()
            .any(|v| v["type"] == "function_call_output" && v["call_id"] == "call_fallback_todo")
    );
    assert!(
        second_input
            .iter()
            .any(|v| v.to_string().contains("FALLBACK_STREAM_SUMMARY_MARKER"))
    );
    assert_eq!(sessions.todos(session.id).await.unwrap().len(), 1);
    let history = sessions
        .bounded_history(session.id, 256, 256 * 1024)
        .await
        .unwrap();
    assert!(history.iter().any(|item| matches!(item, opencode_rk_storage::HistoryItem::Tool(tool) if tool.call_id == "call_fallback_todo")));
    let messages = sessions.messages(session.id, 50).await.unwrap();
    for text in [
        "fallback old user one",
        "fallback old assistant one",
        "fallback recent user",
        "fallback recent assistant",
        "fallback new user",
        "FALLBACK_STREAM_FINAL_MARKER",
    ] {
        assert!(
            messages
                .iter()
                .any(|message| serde_json::to_string(&message.body).unwrap().contains(text)),
            "missing durable text: {text}"
        );
    }
}
