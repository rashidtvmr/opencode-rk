//! Context checkpoint selection: native opaque state is provider-only and
//! selecting it is atomic with the durable context projection.
#![forbid(unsafe_code)]

use std::{env, sync::{atomic::{AtomicUsize, Ordering}, Arc}, time::Duration};

use axum::{
    body::{to_bytes, Body},
    extract::Json,
    http::{Request, StatusCode},
    routing::post,
    Router,
};
use opencode_rk_catalog::Catalog;
use opencode_rk_contracts::MessageRole;
use opencode_rk_server::{daemon_auth::DaemonAuth, router, router_with_auth, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::{Storage, StoragePaths};
use serde_json::{json, Value};
use tempfile::tempdir;
use tokio::sync::mpsc;
use tower::ServiceExt;

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct EnvGuard(Vec<(&'static str, Option<std::ffi::OsString>)>);
impl EnvGuard {
    fn new(root: &std::path::Path, base: &str) -> Self {
        let values = [
            ("HOME", root.display().to_string()),
            ("OPENCODE_RK_HOME", root.join("home").display().to_string()),
            ("OPENCODE_RK_CONFIG_DIR", root.join("config").display().to_string()),
            ("OPENAI_BASE_URL", base.to_owned()),
            ("OPENAI_API_KEY", "context-checkpoint-fixture-key".to_owned()),
        ];
        let mut old = Vec::new();
        for (key, value) in values {
            old.push((key, env::var_os(key)));
            env::set_var(key, value);
        }
        for key in [
            "OPENCODE_RK_COMPACTION__ENABLED",
            "OPENCODE_RK_COMPACTION__THRESHOLD_TOKENS",
            "OPENCODE_RK_COMPACTION__KEEP_RECENT_TURNS",
            "OPENCODE_RK_COMPACTION__STRATEGY",
        ] {
            old.push((key, env::var_os(key)));
            env::remove_var(key);
        }
        Self(old)
    }
}
impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.0.drain(..).rev() {
            match value {
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

async fn response_body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 256 * 1024)
        .await
        .expect("bounded response body");
    serde_json::from_slice(&bytes).expect("JSON response")
}

async fn next_request(rx: &mut mpsc::Receiver<Value>) -> Value {
    tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("provider request timeout")
        .expect("provider request channel")
}

fn has_encrypted_content(value: &Value, expected: &str) -> bool {
    match value {
        Value::Object(object) => {
            object
                .get("encrypted_content")
                .and_then(Value::as_str)
                == Some(expected)
                || object
                    .values()
                    .any(|child| has_encrypted_content(child, expected))
        }
        Value::Array(values) => values
            .iter()
            .any(|child| has_encrypted_content(child, expected)),
        _ => false,
    }
}

fn app(sessions: SessionService) -> axum::Router {
    router(AppState {
        sessions,
        catalog: Arc::new(Catalog::default()),
    })
}

#[tokio::test]
async fn native_checkpoint_selection_is_provider_only_atomic_and_restart_durable() {
    let _lock = ENV_LOCK.lock().await;
    let root = tempdir().expect("disposable HOME");
    std::fs::write(root.path().join("AGENTS.md"), "CURRENT_INSTRUCTIONS")
        .expect("instructions fixture");
    let opaque = "opaque-secret-native-window".to_owned();
    let opaque_second = "opaque-secret-native-window-second".to_owned();
    let prefix = "PREFIX_OLD_MESSAGE_SECRET";
    let retained = json!({
        "type":"message", "role":"assistant",
        "content":[{"type":"output_text","text":"standalone NativeInput retained"}],
        "future_message_metadata":{"fixture":true}
    });
    let (tx, mut rx) = mpsc::channel::<Value>(16);
    let compact_tx = tx.clone();
    let response_tx = tx.clone();
    let compact_calls = Arc::new(AtomicUsize::new(0));
    let retained_for_provider = retained.clone();
    let opaque_for_provider = opaque.clone();
    let opaque_second_for_provider = opaque_second.clone();
    let compact_count = compact_calls.clone();
    let provider = Router::new()
        .route("/responses/compact", post(move |Json(body): Json<Value>| {
            let tx = compact_tx.clone();
            let retained = retained_for_provider.clone();
            let opaque = opaque_for_provider.clone();
            let opaque_second = opaque_second_for_provider.clone();
            let count = compact_count.fetch_add(1, Ordering::SeqCst);
            async move {
                tx.send(body).await.expect("compact channel");
                let (opaque, id) = if count == 0 {
                    (opaque, "checkpoint-boundary")
                } else {
                    (opaque_second, "checkpoint-boundary-second")
                };
                Json(json!({
                    "id":"response.compaction.fixture", "object":"response.compaction",
                    "created_at":1735000001,
                    "output":[retained,{"type":"compaction","id":id,"encrypted_content":opaque,"unknown_fixture_metadata":{"keep":true}}]
                }))
            }
        }))
        .route("/responses", post(move |Json(body): Json<Value>| {
            let tx = response_tx.clone();
            async move {
                tx.send(body).await.expect("turn channel");
                Json(json!({"id":"turn.fixture","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"FIXTURE_TURN_REPLY"}]}]}))
            }
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let _provider = ProviderTask(tokio::spawn(async move {
        axum::serve(listener, provider).await.expect("provider fixture");
    }));
    let _env = EnvGuard::new(root.path(), &format!("http://{address}"));
    let paths = StoragePaths::under(root.path().join("storage"));
    let sessions = SessionService::new(Arc::new(Storage::open(paths.clone()).unwrap()));
    let session = sessions
        .create_in_directory("checkpoint selection", Some(root.path().to_owned()))
        .await
        .unwrap();
    let prefix_id = sessions
        .append_text(session.id, MessageRole::User, prefix)
        .await
        .unwrap()
        .id;
    sessions
        .append_text(session.id, MessageRole::Assistant, "old assistant body")
        .await
        .unwrap();
    let boundary = sessions
        .append_text(session.id, MessageRole::User, "post-checkpoint tail")
        .await
        .unwrap()
        .id;

    let first_app = app(sessions.clone());
    let compact = first_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/compact", session.id))
                .header("content-type", "application/json")
                .body(Body::from(json!({"model":"openai/fixture","strategy":"native"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(compact.status(), StatusCode::OK);
    let compact_json = response_body(compact).await;
    assert!(!compact_json.to_string().contains(opaque.as_str()), "opaque state must not be UI response");
    let _compact_request = next_request(&mut rx).await;

    let context_path = format!("/api/sessions/{}/context", session.id);
    let listed = first_app
        .clone()
        .oneshot(Request::builder().uri(&context_path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_json = response_body(listed).await;
    assert!(!listed_json.to_string().contains(opaque.as_str()));
    let checkpoint = listed_json["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["kind"] == "checkpoint")
        .expect("checkpoint context item");
    assert_eq!(checkpoint["id"], format!("checkpoint:{boundary}"));
    assert_eq!(checkpoint["included"], true);
    assert_eq!(checkpoint["selectable"], true);
    assert_eq!(checkpoint["content_available"], false);
    assert!(!checkpoint.to_string().contains(opaque.as_str()));
    let compacted_prefix = listed_json["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["content"] == prefix)
        .expect("compacted durable message remains visible");
    assert_eq!(compacted_prefix["selectable"], false);
    assert_eq!(compacted_prefix["represented_by_checkpoint"], true);

    let exclude_prefix = first_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/context/exclude", session.id))
                .header("content-type", "application/json")
                .body(Body::from(json!({"item_id":prefix_id}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(exclude_prefix.status(), StatusCode::CONFLICT);

    let include_prefix = first_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/context/include", session.id))
                .header("content-type", "application/json")
                .body(Body::from(json!({"item_id":prefix_id}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(include_prefix.status(), StatusCode::CONFLICT);

    let exclude_checkpoint = first_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/context/exclude", session.id))
                .header("content-type", "application/json")
                .body(Body::from(json!({"item_id":format!("checkpoint:{boundary}")}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(exclude_checkpoint.status(), StatusCode::OK);

    let turn = first_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/turns", session.id))
                .header("content-type", "application/json")
                .body(Body::from(json!({"model":"openai/fixture","text":"CURRENT_INPUT_ONE"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(turn.status(), StatusCode::CREATED);
    let no_checkpoint = next_request(&mut rx).await;
    assert!(no_checkpoint.to_string().contains("CURRENT_INPUT_ONE"));
    assert!(no_checkpoint.to_string().contains("CURRENT_INSTRUCTIONS"));
    assert!(!no_checkpoint.to_string().contains(opaque.as_str()));
    assert!(!no_checkpoint.to_string().contains(prefix));

    drop(first_app);
    let reopened = SessionService::new(Arc::new(Storage::open(paths).unwrap()));
    let restarted_app = app(reopened.clone());
    let after_restart = restarted_app
        .clone()
        .oneshot(Request::builder().uri(&context_path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(after_restart.status(), StatusCode::OK);
    let after_restart_json = response_body(after_restart).await;
    assert_eq!(after_restart_json["checkpoint"]["included"], false);
    assert!(after_restart_json["items"].as_array().unwrap().iter().any(|item| {
        item["content"].as_str() == Some(prefix)
    }));
    assert!(!after_restart_json.to_string().contains(opaque.as_str()));

    let include = restarted_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/context/include", session.id))
                .header("content-type", "application/json")
                .body(Body::from(json!({"item_id":format!("checkpoint:{boundary}")}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(include.status(), StatusCode::OK);
    let second = restarted_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/turns", session.id))
                .header("content-type", "application/json")
                .body(Body::from(json!({"model":"openai/fixture","text":"CURRENT_INPUT_TWO"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::CREATED);
    let restored = next_request(&mut rx).await;
    assert!(has_encrypted_content(&restored, opaque.as_str()));
    assert!(restored.to_string().contains("CURRENT_INPUT_TWO"));
    assert!(restored.to_string().contains("standalone NativeInput retained"));
    assert!(restored.to_string().contains("CURRENT_INPUT_ONE"));
    assert!(restored.to_string().contains("CURRENT_INSTRUCTIONS"));

    // Regression phase: once the old checkpoint has been included again, an
    // explicit exclusion of that old checkpoint must not poison a subsequent
    // native re-compaction. The replacement checkpoint is a new atomic item.
    let exclude_again = restarted_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/context/exclude", session.id))
                .header("content-type", "application/json")
                .body(Body::from(json!({"item_id":format!("checkpoint:{boundary}")}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(exclude_again.status(), StatusCode::OK);

    let tail = restarted_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/turns", session.id))
                .header("content-type", "application/json")
                .body(Body::from(json!({"model":"openai/fixture","text":"NEW_COMPACTION_TAIL"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(tail.status(), StatusCode::CREATED);
    let tail_request = next_request(&mut rx).await;
    assert!(tail_request.to_string().contains("NEW_COMPACTION_TAIL"));
    assert!(!has_encrypted_content(&tail_request, opaque.as_str()));

    let compact_again = restarted_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/compact", session.id))
                .header("content-type", "application/json")
                .body(Body::from(json!({"model":"openai/fixture","strategy":"native"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(compact_again.status(), StatusCode::OK);
    let second_compact_request = next_request(&mut rx).await;
    assert!(!has_encrypted_content(&second_compact_request, opaque.as_str()));
    let current_head = reopened.messages(session.id, 200).await.unwrap().last().unwrap().id;

    let replaced_context = restarted_app
        .clone()
        .oneshot(Request::builder().uri(&context_path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(replaced_context.status(), StatusCode::OK);
    let replaced_context = response_body(replaced_context).await;
    assert_eq!(replaced_context["checkpoint"]["boundary_message_id"], current_head.to_string());
    assert_eq!(replaced_context["checkpoint"]["included"], true);
    assert!(!replaced_context.to_string().contains(opaque.as_str()));

    let final_turn = restarted_app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{}/turns", session.id))
                .header("content-type", "application/json")
                .body(Body::from(json!({"model":"openai/fixture","text":"AFTER_REPLACEMENT_CHECKPOINT"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(final_turn.status(), StatusCode::CREATED);
    let final_request = next_request(&mut rx).await;
    let final_input = final_request["input"].as_array().expect("final provider input");
    assert!(final_input
        .iter()
        .any(|item| has_encrypted_content(item, opaque_second.as_str())));
    assert!(final_input
        .iter()
        .all(|item| !has_encrypted_content(item, opaque.as_str())));
    assert!(final_request.to_string().contains("AFTER_REPLACEMENT_CHECKPOINT"));
}

#[tokio::test]
async fn context_routes_require_native_bearer_and_reject_unauthorized_mutation() {
    let _lock = ENV_LOCK.lock().await;
    let root = tempdir().unwrap();
    let sessions = SessionService::new(Arc::new(Storage::open_in_memory(root.path().join("blobs")).unwrap()));
    let session = sessions.create("auth context").await.unwrap();
    let auth = DaemonAuth::mint().unwrap();
    let token = auth.token().to_owned();
    let app = router_with_auth(AppState { sessions: sessions.clone(), catalog: Arc::new(Catalog::default()) }, Some(auth));
    let path = format!("/api/sessions/{}/context", session.id);
    for authorization in [None, Some("Bearer definitely-wrong") ] {
        let mut request = Request::builder().uri(&path);
        if let Some(value) = authorization { request = request.header("authorization", value); }
        let response = app.clone().oneshot(request.body(Body::empty()).unwrap()).await.unwrap();
        assert!(matches!(response.status(), StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN));
    }
    let before = sessions.context_selection(session.id).await.unwrap();
    let response = app.clone().oneshot(Request::builder().method("POST").uri(format!("/api/sessions/{}/context/exclude", session.id)).header("content-type", "application/json").body(Body::from(json!({"item_id":"00000000-0000-4000-8000-000000000000"}).to_string())).unwrap()).await.unwrap();
    assert!(matches!(response.status(), StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN));
    let after = sessions.context_selection(session.id).await.unwrap();
    assert_eq!(serde_json::to_value(&after).unwrap(), serde_json::to_value(&before).unwrap());
    let valid = app.oneshot(Request::builder().uri(&path).header("authorization", format!("Bearer {token}")).body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(valid.status(), StatusCode::OK);
}
