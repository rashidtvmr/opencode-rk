//! 004-D: compaction policy is a per-session, durable setting.
//!
//! The endpoint is intentionally exercised through the real router and a real
//! ordinary Responses fixture.  In particular, the second request proves that
//! the persisted fallback policy is used after reopening the SQLite database.

use std::{env, sync::Arc, time::Duration};

use axum::{
    body::{to_bytes, Body},
    extract::Json,
    http::{Request, StatusCode},
    routing::post,
    Router,
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

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct EnvGuard(Vec<(&'static str, Option<std::ffi::OsString>)>);

impl EnvGuard {
    fn set(values: &[(&'static str, String)]) -> Self {
        const COMPACTION_KEYS: [&str; 4] = [
            "OPENCODE_RK_COMPACTION__ENABLED",
            "OPENCODE_RK_COMPACTION__THRESHOLD_TOKENS",
            "OPENCODE_RK_COMPACTION__KEEP_RECENT_TURNS",
            "OPENCODE_RK_COMPACTION__STRATEGY",
        ];
        let mut previous = Vec::with_capacity(values.len() + COMPACTION_KEYS.len());
        for key in COMPACTION_KEYS {
            if !values.iter().any(|(value_key, _)| *value_key == key) {
                previous.push((key, env::var_os(key)));
                env::remove_var(key);
            }
        }
        for (key, value) in values {
            previous.push((*key, env::var_os(key)));
            env::set_var(key, value);
        }
        Self(previous)
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

async fn body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 256 * 1024)
        .await
        .expect("bounded JSON response");
    if bytes.is_empty() {
        return Value::Null;
    }
    serde_json::from_slice(&bytes).expect("JSON response")
}

async fn request(
    app: &axum::Router,
    method: &str,
    uri: String,
    payload: Value,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    (status, body(response).await)
}

async fn fixture() -> (String, mpsc::Receiver<Value>, Task) {
    let (tx, rx) = mpsc::channel(8);
    let app = Router::new().route(
        "/responses",
        post(move |Json(input): Json<Value>| {
            let tx = tx.clone();
            async move {
                tx.send(input.clone()).await.unwrap();
                let summary = input["input"]
                    .to_string()
                    .contains("compaction summary");
                Json(json!({
                    "id": if summary { "summary" } else { "final" },
                    "status": "completed",
                    "output": [{"type":"message","role":"assistant","content":[
                        {"type":"output_text","text": if summary { "FALLBACK_SUMMARY" } else { "FIXTURE_FINAL" }}
                    ]}]
                }))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = Task(tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    }));
    (format!("http://{address}"), rx, task)
}

#[tokio::test]
async fn session_compaction_settings_round_trip_restart_and_drive_fallback() {
    let _lock = ENV_LOCK.lock().await;
    let root = tempdir().unwrap();
    std::fs::write(root.path().join("AGENTS.md"), "SESSION_SETTINGS_AGENTS").unwrap();
    let (base, mut requests, _fixture) = fixture().await;
    let _env = EnvGuard::set(&[
        ("OPENAI_BASE_URL", base),
        ("OPENAI_API_KEY", "disposable-session-settings".into()),
        ("OPENCODE_RK_HOME", root.path().display().to_string()),
    ]);
    let paths = StoragePaths::under(root.path().join("storage"));
    let sessions = SessionService::new(Arc::new(Storage::open(paths.clone()).unwrap()));
    let session = sessions
        .create_in_directory("settings fixture", Some(root.path().to_owned()))
        .await
        .unwrap();
    for i in 0..96 {
        sessions
            .append_text(
                session.id,
                if i % 2 == 0 {
                    MessageRole::User
                } else {
                    MessageRole::Assistant
                },
                format!("old-{i}"),
            )
            .await
            .unwrap();
    }
    let app = router(AppState {
        sessions,
        catalog: Arc::new(Catalog::default()),
    });
    let config =
        json!({"enabled":true,"threshold_tokens":1,"keep_recent_turns":1,"strategy":"fallback"});
    let (status, saved) = request(
        &app,
        "PUT",
        format!("/api/sessions/{}/compaction/settings", session.id),
        config.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved["scope"], "session");
    assert_eq!(saved["configured"], config);
    assert_eq!(
        saved["capabilities"]["native"]["availability"],
        "unverified"
    );
    assert_eq!(
        saved["capabilities"]["native"]["retention"],
        "provider-owned"
    );
    assert_eq!(
        saved["capabilities"]["fallback"]["availability"],
        "responses-summary"
    );
    assert_eq!(
        saved["capabilities"]["fallback"]["retention"],
        "keep_recent_turns"
    );

    drop(app);
    let reopened = SessionService::new(Arc::new(Storage::open(paths).unwrap()));
    let app = router(AppState {
        sessions: reopened,
        catalog: Arc::new(Catalog::default()),
    });
    let (status, readback) = request(
        &app,
        "GET",
        format!("/api/sessions/{}/compaction/settings", session.id),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(readback["configured"], config);
    assert_eq!(readback["scope"], "session");

    let (status, _) = request(
        &app,
        "POST",
        format!("/api/sessions/{}/turns", session.id),
        json!({"model":"openai/fixture","text":"new user"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let summary = tokio::time::timeout(Duration::from_secs(2), requests.recv())
        .await
        .unwrap()
        .unwrap();
    let final_request = tokio::time::timeout(Duration::from_secs(2), requests.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(summary["input"].to_string().contains("compaction summary"));
    let input = final_request["input"].as_array().unwrap();
    assert!(input
        .iter()
        .any(|item| item.to_string().contains("FALLBACK_SUMMARY")));
    assert!(input.iter().any(|item| item.to_string().contains("old-95")));
    assert!(input
        .iter()
        .any(|item| item.to_string().contains("new user")));
    assert!(!input.iter().any(|item| item.to_string().contains("old-0")));
}

#[tokio::test]
async fn session_compaction_settings_reject_bad_payload_and_unknown_session() {
    let _lock = ENV_LOCK.lock().await;
    let root = tempdir().unwrap();
    let _env = EnvGuard::set(&[("OPENCODE_RK_HOME", root.path().display().to_string())]);
    let sessions = SessionService::new(Arc::new(
        Storage::open_in_memory(root.path().join("blobs")).unwrap(),
    ));
    let session = sessions
        .create_in_directory("validation", Some(root.path().to_owned()))
        .await
        .unwrap();
    let app = router(AppState {
        sessions,
        catalog: Arc::new(Catalog::default()),
    });
    let config =
        json!({"enabled":false,"threshold_tokens":1,"keep_recent_turns":1,"strategy":"fallback"});
    let (status, _) = request(
        &app,
        "PUT",
        format!("/api/sessions/{}/compaction/settings", session.id),
        config.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = request(&app, "PUT", format!("/api/sessions/{}/compaction/settings", session.id), json!({"enabled":true,"threshold_tokens":0,"keep_recent_turns":1,"strategy":"fallback","unknown":true})).await;
    assert!(status.is_client_error());
    let (status, readback) = request(
        &app,
        "GET",
        format!("/api/sessions/{}/compaction/settings", session.id),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(readback["configured"], config);
    let (status, _) = request(
        &app,
        "GET",
        "/api/sessions/00000000-0000-4000-8000-000000000001/compaction/settings".into(),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
