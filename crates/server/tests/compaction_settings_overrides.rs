//! Session compaction settings: environment overrides and strict validation.

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
const COMPACTION_KEYS: [&str; 4] = [
    "OPENCODE_RK_COMPACTION__ENABLED",
    "OPENCODE_RK_COMPACTION__THRESHOLD_TOKENS",
    "OPENCODE_RK_COMPACTION__KEEP_RECENT_TURNS",
    "OPENCODE_RK_COMPACTION__STRATEGY",
];

struct EnvGuard(Vec<(&'static str, Option<std::ffi::OsString>)>);
impl EnvGuard {
    fn set(values: &[(&'static str, String)]) -> Self {
        let mut old = Vec::new();
        for key in COMPACTION_KEYS {
            old.push((key, env::var_os(key)));
            env::remove_var(key);
        }
        for (key, value) in values {
            if COMPACTION_KEYS.contains(key) {
                env::set_var(key, value);
                continue;
            }
            old.push((*key, env::var_os(key)));
            env::set_var(key, value);
        }
        Self(old)
    }
    fn remove(&self, key: &'static str) {
        env::remove_var(key);
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

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 256 * 1024).await.unwrap();
    if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    }
}
async fn call(app: &axum::Router, method: &str, uri: String, value: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(value.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    (status, json_body(response).await)
}
async fn fixture() -> (String, mpsc::Receiver<Value>, Task) {
    let (tx, rx) = mpsc::channel(8);
    let service = Router::new().route("/responses", post(move |Json(request): Json<Value>| {
        let tx = tx.clone();
        async move {
            tx.send(request.clone()).await.unwrap();
            let summary = request["input"].to_string().contains("compaction summary");
            Json(json!({"id":if summary {"summary"} else {"final"},"status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":if summary {"FALLBACK_SUMMARY"} else {"OVERRIDE_FINAL"}}]}]}))
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = Task(tokio::spawn(async move {
        axum::serve(listener, service).await.unwrap();
    }));
    (format!("http://{address}"), rx, task)
}
async fn seed(
    sessions: &SessionService,
    root: &std::path::Path,
) -> opencode_rk_contracts::SessionSummary {
    let session = sessions
        .create_in_directory("override fixture", Some(root.to_owned()))
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
    session
}

#[tokio::test]
async fn environment_override_is_reported_and_then_disabled_policy_stops_summary_io() {
    let _lock = ENV_LOCK.lock().await;
    let root = tempdir().unwrap();
    let (base, mut requests, _task) = fixture().await;
    let env_guard = EnvGuard::set(&[
        ("OPENAI_BASE_URL", base),
        ("OPENAI_API_KEY", "override-key".into()),
        ("OPENCODE_RK_HOME", root.path().display().to_string()),
        ("OPENCODE_RK_COMPACTION__ENABLED", "true".into()),
        ("OPENCODE_RK_COMPACTION__THRESHOLD_TOKENS", "1".into()),
        ("OPENCODE_RK_COMPACTION__KEEP_RECENT_TURNS", "1".into()),
        ("OPENCODE_RK_COMPACTION__STRATEGY", "fallback".into()),
    ]);
    let paths = StoragePaths::under(root.path().join("storage"));
    let sessions = SessionService::new(Arc::new(Storage::open(paths.clone()).unwrap()));
    let session = seed(&sessions, root.path()).await;
    let app = router(AppState {
        sessions,
        catalog: Arc::new(Catalog::default()),
    });
    let policy =
        json!({"enabled":false,"threshold_tokens":1,"keep_recent_turns":1,"strategy":"fallback"});
    assert_eq!(
        call(
            &app,
            "PUT",
            format!("/api/sessions/{}/compaction/settings", session.id),
            policy.clone()
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, metadata) = call(
        &app,
        "GET",
        format!("/api/sessions/{}/compaction/settings", session.id),
        json!({}),
    )
    .await;
    assert_eq!(metadata["configured"], policy);
    assert_eq!(metadata["effective"]["enabled"], true);
    assert!(metadata["overridden_by_environment"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "enabled"));
    assert_eq!(
        call(
            &app,
            "POST",
            format!("/api/sessions/{}/turns", session.id),
            json!({"model":"openai/fixture","text":"override turn"})
        )
        .await
        .0,
        StatusCode::CREATED
    );
    let summary = tokio::time::timeout(Duration::from_secs(2), requests.recv())
        .await
        .unwrap()
        .unwrap();
    let final_request = tokio::time::timeout(Duration::from_secs(2), requests.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(summary["input"].to_string().contains("compaction summary"));
    assert!(final_request["input"]
        .to_string()
        .contains("FALLBACK_SUMMARY"));
    env_guard.remove("OPENCODE_RK_COMPACTION__ENABLED");
    assert_eq!(
        call(
            &app,
            "PUT",
            format!("/api/sessions/{}/compaction/settings", session.id),
            policy
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "POST",
            format!("/api/sessions/{}/turns", session.id),
            json!({"model":"openai/fixture","text":"disabled turn"})
        )
        .await
        .0,
        StatusCode::CREATED
    );
    let only_final = tokio::time::timeout(Duration::from_secs(2), requests.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(!only_final["input"]
        .to_string()
        .contains("compaction summary"));
    assert!(
        tokio::time::timeout(Duration::from_millis(100), requests.recv())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn invalid_environment_and_payload_bounds_preserve_previous_policy() {
    let _lock = ENV_LOCK.lock().await;
    let root = tempdir().unwrap();
    let env_guard = EnvGuard::set(&[
        ("OPENCODE_RK_HOME", root.path().display().to_string()),
        ("OPENCODE_RK_COMPACTION__ENABLED", "maybe".into()),
    ]);
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
    let valid =
        json!({"enabled":false,"threshold_tokens":1,"keep_recent_turns":1,"strategy":"fallback"});
    let uri = format!("/api/sessions/{}/compaction/settings", session.id);
    assert!(call(&app, "PUT", uri.clone(), valid.clone())
        .await
        .0
        .is_client_error());
    env_guard.remove("OPENCODE_RK_COMPACTION__ENABLED");
    assert_eq!(
        call(&app, "PUT", uri.clone(), valid.clone()).await.0,
        StatusCode::OK
    );
    for bad in [
        json!({"enabled":false,"threshold_tokens":0,"keep_recent_turns":1,"strategy":"fallback"}),
        json!({"enabled":false,"threshold_tokens":10000001,"keep_recent_turns":1,"strategy":"fallback"}),
        json!({"enabled":false,"threshold_tokens":1,"keep_recent_turns":0,"strategy":"fallback"}),
        json!({"enabled":false,"threshold_tokens":1,"keep_recent_turns":1001,"strategy":"fallback"}),
        json!({"enabled":false,"threshold_tokens":1,"keep_recent_turns":1,"strategy":"unknown"}),
        json!({"enabled":false,"threshold_tokens":1,"keep_recent_turns":1}),
        json!({"enabled":false,"threshold_tokens":1,"keep_recent_turns":1,"strategy":"fallback","unknown":true}),
    ] {
        assert!(call(&app, "PUT", uri.clone(), bad)
            .await
            .0
            .is_client_error());
        assert_eq!(
            call(&app, "GET", uri.clone(), json!({})).await.1["configured"],
            valid
        );
    }
}
