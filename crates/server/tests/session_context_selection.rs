//! RED contract for session-scoped context inspection and durable exclusions.
//!
//! This deliberately uses the public router and a disposable Responses API
//! fixture.  It is not a unit test of a storage helper: the exclusion must be
//! visible to context selection, provider input, and a reopened session.

use std::{env, sync::Arc, time::Duration};

use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{Request, StatusCode},
    routing::post,
    Json, Router,
};
use opencode_rk_catalog::Catalog;
use opencode_rk_contracts::MessageRole;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::{Storage, StoragePaths};
use serde_json::{json, Value};
use tempfile::tempdir;
use tokio::sync::mpsc;
use tower::ServiceExt;

struct EnvGuard(Vec<(&'static str, Option<std::ffi::OsString>)>);

impl EnvGuard {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn set(&mut self, values: &[(&'static str, String)]) {
        for (key, value) in values {
            self.capture(*key);
            env::set_var(key, value);
        }
    }

    fn remove(&mut self, keys: &[&'static str]) {
        for key in keys {
            self.capture(*key);
            env::remove_var(key);
        }
    }

    fn capture(&mut self, key: &'static str) {
        if !self.0.iter().any(|(captured, _)| *captured == key) {
            self.0.push((key, env::var_os(key)));
        }
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

struct ProviderTask(tokio::task::JoinHandle<()>);

impl Drop for ProviderTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 256 * 1024)
        .await
        .expect("bounded response body");
    serde_json::from_slice(&bytes).expect("JSON response")
}

async fn provider(
    State(seen): State<mpsc::Sender<Value>>,
    Json(request): Json<Value>,
) -> Json<Value> {
    let _ = seen.send(request).await;
    Json(json!({
        "id": "context-fixture-response",
        "status": "completed",
        "output": [{"type":"message","role":"assistant","content":[{"type":"output_text","text":"FIXTURE_REPLY"}]}]
    }))
}

async fn wait_provider(seen: &mut mpsc::Receiver<Value>) -> Value {
    tokio::time::timeout(Duration::from_secs(2), seen.recv())
        .await
        .expect("provider fixture response")
        .expect("provider fixture channel")
}

#[tokio::test]
async fn session_context_exclusion_is_http_scoped_durable_and_applied_to_turns() {
    let root = tempdir().expect("disposable fixture root");
    std::fs::write(root.path().join("AGENTS.md"), "CURRENT_INSTRUCTIONS").expect("AGENTS fixture");

    let (seen_tx, mut seen_rx) = mpsc::channel::<Value>(8);
    let provider = Router::new()
        .route("/v1/responses", post(provider))
        .with_state(seen_tx);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let _provider_task = ProviderTask(tokio::spawn(async move {
        axum::serve(listener, provider)
            .await
            .expect("provider fixture");
    }));
    let mut _env = EnvGuard::new();
    _env.remove(&[
        "OPENCODE_RK_COMPACTION__ENABLED",
        "OPENCODE_RK_COMPACTION__THRESHOLD_TOKENS",
        "OPENCODE_RK_COMPACTION__KEEP_RECENT_TURNS",
        "OPENCODE_RK_COMPACTION__STRATEGY",
    ]);
    _env.set(&[
        ("HOME", root.path().display().to_string()),
        (
            "OPENCODE_RK_HOME",
            root.path().join("home").display().to_string(),
        ),
        (
            "OPENCODE_RK_CONFIG_DIR",
            root.path().join("config").display().to_string(),
        ),
        ("OPENAI_BASE_URL", format!("http://{address}/v1")),
        ("OPENAI_API_KEY", "context-fixture-key".to_owned()),
        ("OPENCODE_RK_COMPACTION__ENABLED", "false".to_owned()),
    ]);

    let storage = Storage::open(StoragePaths::under(root.path().join("state"))).unwrap();
    let sessions = SessionService::new(Arc::new(storage));
    let first = sessions
        .create_in_directory("context", Some(root.path().to_owned()))
        .await
        .unwrap();
    let second = sessions.create("other").await.unwrap();
    let old_user = sessions
        .append_text(first.id, MessageRole::User, "OLD_REMOVE")
        .await
        .unwrap();
    let old_assistant = sessions
        .append_text(first.id, MessageRole::Assistant, "OLD_REPLY_REMOVE")
        .await
        .unwrap();
    sessions
        .append_text(first.id, MessageRole::User, "KEEP_RECENT")
        .await
        .unwrap();
    sessions
        .append_text(first.id, MessageRole::Assistant, "KEEP_REPLY")
        .await
        .unwrap();
    let system = sessions
        .append_text(first.id, MessageRole::System, "CURRENT_INSTRUCTIONS")
        .await
        .unwrap();
    assert_eq!(system.role, MessageRole::System);

    let app = router(AppState {
        sessions: sessions.clone(),
        catalog: Arc::new(Catalog::default()),
    });
    let path = format!("/api/sessions/{}/context", first.id);
    let listed = app
        .clone()
        .oneshot(Request::builder().uri(&path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    let listed = body(listed).await;
    assert_eq!(listed["scope"], "session");
    let items = listed["items"].as_array().expect("context items");
    let old = items
        .iter()
        .find(|item| item["id"] == old_user.id.to_string())
        .expect("old user context item");
    assert_eq!(old["kind"], "message");
    assert_eq!(old["included"], true);
    assert_eq!(old["role"], "user");
    assert!(old["content"].as_str().unwrap().contains("OLD_REMOVE"));

    for id in [old_user.id, old_assistant.id] {
        let excluded = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/sessions/{}/context/exclude", first.id))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"item_id": id.to_string()}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(excluded.status(), StatusCode::OK);
        let ack = body(excluded).await;
        assert_eq!(ack["scope"], "session");
        assert_eq!(ack["excluded"], true);
    }
    let listed = app
        .clone()
        .oneshot(Request::builder().uri(&path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    let listed = body(listed).await;
    for id in [old_user.id, old_assistant.id] {
        assert_eq!(
            listed["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|item| item["id"] == id.to_string())
                .unwrap()["included"],
            false
        );
    }
    let system_excluded = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/context/exclude", first.id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"item_id": system.id.to_string()}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(system_excluded.status().is_client_error());
    assert_eq!(
        listed["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == system.id.to_string())
            .unwrap()["included"],
        true
    );
    let forged = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/context/exclude", second.id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"item_id": old_user.id.to_string()}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(forged.status().is_client_error());
    let unknown = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/sessions/00000000-0000-4000-8000-000000000000/context")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unknown.status(), StatusCode::NOT_FOUND);

    let turn = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/turns", first.id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"model":"openai/fixture","text":"NEW_PROMPT"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(turn.status(), StatusCode::CREATED);
    let request = wait_provider(&mut seen_rx).await;
    let input = serde_json::to_string(&request["input"]).unwrap();
    assert!(!input.contains("OLD_REMOVE") && !input.contains("OLD_REPLY_REMOVE"));
    for expected in [
        "KEEP_RECENT",
        "KEEP_REPLY",
        "NEW_PROMPT",
        "CURRENT_INSTRUCTIONS",
    ] {
        assert!(
            input.contains(expected),
            "provider input missing {expected}"
        );
    }

    let durable = sessions.messages(first.id, 100).await.unwrap();
    assert!(durable.iter().any(|m| serde_json::to_string(&m.body)
        .unwrap()
        .contains("OLD_REMOVE")));
    drop(app);
    drop(sessions);
    let reopened = SessionService::new(Arc::new(
        Storage::open(StoragePaths::under(root.path().join("state"))).unwrap(),
    ));
    let app = router(AppState {
        sessions: reopened.clone(),
        catalog: Arc::new(Catalog::default()),
    });
    let listed_response = app
        .clone()
        .oneshot(Request::builder().uri(&path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(listed_response.status(), StatusCode::OK);
    let listed = body(listed_response).await;
    assert_eq!(
        listed["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == old_user.id.to_string())
            .unwrap()["included"],
        false
    );
    let include = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/context/include", first.id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"item_id": old_user.id.to_string()}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(include.status(), StatusCode::OK);
    let turn = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/turns", first.id))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"model":"openai/fixture","text":"RESTORED_PROMPT"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(turn.status(), StatusCode::CREATED);
    let request = wait_provider(&mut seen_rx).await;
    let restored_input = serde_json::to_string(&request["input"]).unwrap();
    assert!(restored_input.contains("OLD_REMOVE"));
    assert!(!restored_input.contains("OLD_REPLY_REMOVE"));
    let durable = reopened.messages(first.id, 100).await.unwrap();
    assert_eq!(durable.len(), 9);
    assert!(durable
        .iter()
        .any(|message| serde_json::to_string(&message.body)
            .unwrap()
            .contains("OLD_REMOVE")));
    assert!(durable
        .iter()
        .any(|message| serde_json::to_string(&message.body)
            .unwrap()
            .contains("OLD_REPLY_REMOVE")));
}
