//! 004-D: the explicit `auto` and omitted manual strategies resolve through
//! the configured fallback policy; they must not silently select native.

use axum::{
    Router,
    body::Body,
    extract::Json,
    http::{Request, StatusCode},
    routing::post,
};
use opencode_rk_catalog::Catalog;
use opencode_rk_contracts::{MessageRole, SessionSummary};
use opencode_rk_server::{AppState, router};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::{Storage, StoragePaths};
use serde_json::{Value, json};
use std::{env, sync::Arc, time::Duration};
use tempfile::tempdir;
use tokio::sync::mpsc;
use tower::ServiceExt;

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct EnvGuard(Vec<(&'static str, Option<std::ffi::OsString>)>);
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

async fn provider(
    tx: axum::extract::State<mpsc::Sender<Value>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let _ = tx.send(body.clone()).await;
    let summary = body["input"]
        .to_string()
        .to_ascii_lowercase()
        .contains("compaction")
        || body["input"].to_string().contains("summarize");
    Json(json!({
        "id": if summary { "strategy-summary" } else { "strategy-final" },
        "status":"completed",
        "output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":if summary { "STRATEGY_FALLBACK_SUMMARY" } else { "STRATEGY_FINAL" }}]}]
    }))
}

async fn fixture(tx: mpsc::Sender<Value>) -> (String, Task) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/responses", post(provider))
        .with_state(tx);
    let task = Task(tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    }));
    (format!("http://{address}"), task)
}

async fn session(root: &std::path::Path) -> (SessionService, SessionSummary) {
    let service = SessionService::new(Arc::new(
        Storage::open(StoragePaths::under(root.join("storage"))).unwrap(),
    ));
    let summary = service
        .create_in_directory("strategy fixture", Some(root.to_owned()))
        .await
        .unwrap();
    (service, summary)
}

async fn post_json(app: Router, uri: String, value: Value) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(value.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn auto_and_omitted_manual_strategy_resolve_to_configured_fallback() {
    let _lock = ENV_LOCK.lock().await;
    for body in [
        json!({"model":"openai/fixture","strategy":"auto"}),
        json!({"model":"openai/fixture"}),
    ] {
        let root = tempdir().unwrap();
        let (tx, mut requests) = mpsc::channel(8);
        let (base, _server) = fixture(tx).await;
        let _env = EnvGuard::set(&[
            ("OPENAI_BASE_URL", base),
            ("OPENAI_API_KEY", "strategy-fixture-key".into()),
            ("OPENCODE_RK_HOME", root.path().display().to_string()),
            ("OPENCODE_RK_COMPACTION__STRATEGY", "fallback".into()),
            ("OPENCODE_RK_COMPACTION__KEEP_RECENT_TURNS", "1".into()),
        ]);
        let (sessions, current) = session(root.path()).await;
        for (role, text) in [
            (MessageRole::User, "first question"),
            (MessageRole::Assistant, "first answer"),
            (MessageRole::User, "second question"),
            (MessageRole::Assistant, "second answer"),
        ] {
            sessions.append_text(current.id, role, text).await.unwrap();
        }
        let app = router(AppState {
            sessions: sessions.clone(),
            catalog: Arc::new(Catalog::default()),
        });
        let compact = post_json(
            app.clone(),
            format!("/api/sessions/{}/compact", current.id),
            body,
        )
        .await;
        assert_eq!(
            compact.status(),
            StatusCode::OK,
            "configured fallback must resolve auto/omitted"
        );
        let ack = compact.into_body();
        let ack = axum::body::to_bytes(ack, 32 * 1024).await.unwrap();
        let ack: Value = serde_json::from_slice(&ack).unwrap();
        assert_eq!(ack, json!({"compacted":true,"strategy":"fallback"}));
        let summary = tokio::time::timeout(Duration::from_secs(2), requests.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(summary["stream"], false);
        assert_eq!(summary["tools"], json!([]));
        assert!(
            summary["max_output_tokens"]
                .as_u64()
                .is_some_and(|value| value <= 4096)
        );
        assert!(!summary["input"].as_array().unwrap().is_empty());
        assert!(
            summary["input"]
                .to_string()
                .to_ascii_lowercase()
                .contains("compaction")
        );
        let old_last_id = sessions
            .messages(current.id, 20)
            .await
            .unwrap()
            .last()
            .unwrap()
            .id;

        let turn = post_json(
            app,
            format!("/api/sessions/{}/turns", current.id),
            json!({"model":"openai/fixture","text":"continuation question"}),
        )
        .await;
        assert_eq!(turn.status(), StatusCode::CREATED);
        let continuation = tokio::time::timeout(Duration::from_secs(2), requests.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(
            continuation["input"]
                .to_string()
                .contains("STRATEGY_FALLBACK_SUMMARY")
        );
        assert!(
            continuation["input"]
                .to_string()
                .contains("continuation question")
        );
        let checkpoint = sessions
            .compaction_checkpoint(current.id)
            .await
            .unwrap()
            .unwrap();
        assert!(checkpoint.window_json.contains("STRATEGY_FALLBACK_SUMMARY"));
        assert_eq!(checkpoint.boundary_message_id, old_last_id);
        let durable = sessions.messages(current.id, 20).await.unwrap();
        assert_eq!(durable.len(), 6);
        assert!(durable.iter().any(|item| {
            serde_json::to_string(&item.body)
                .unwrap()
                .contains("first question")
        }));
        assert!(durable.iter().any(|item| {
            serde_json::to_string(&item.body)
                .unwrap()
                .contains("first answer")
        }));
        assert!(durable.iter().any(|item| {
            serde_json::to_string(&item.body)
                .unwrap()
                .contains("STRATEGY_FINAL")
        }));
        assert!(!durable.iter().any(|item| {
            serde_json::to_string(&item.body)
                .unwrap()
                .contains("STRATEGY_FALLBACK_SUMMARY")
        }));
    }
}
