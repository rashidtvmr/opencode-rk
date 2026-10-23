//! NP-02 RED: provider model discovery must retain safe context metadata.
//!
//! This is intentionally an installed-route test rather than a parser-only
//! test.  It exercises the authenticated `/api/providers/{id}/models` route,
//! the imported own-config model map, and a disposable OpenAI-compatible HTTP
//! `/models` endpoint together.
#![forbid(unsafe_code)]

use std::sync::{Mutex, MutexGuard};

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{daemon_auth::DaemonAuth, router_with_auth, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tower::ServiceExt;

/// Environment variables are process-global, so serialize this fixture with
/// the other provider/config integration tests and restore every value.
struct EnvGuard {
    _lock: MutexGuard<'static, ()>,
    dir: TempDir,
    previous_rk: Option<std::ffi::OsString>,
    previous_config: Option<std::ffi::OsString>,
    previous_xdg: Option<std::ffi::OsString>,
    previous_home: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn new() -> Self {
        static ENV_LOCK: Mutex<()> = Mutex::new(());
        let lock = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let dir = tempfile::tempdir().expect("disposable provider config directory");
        let guard = Self {
            _lock: lock,
            dir,
            previous_rk: std::env::var_os("OPENCODE_RK_CONFIG_DIR"),
            previous_config: std::env::var_os("OPENCODE_CONFIG_DIR"),
            previous_xdg: std::env::var_os("XDG_CONFIG_HOME"),
            previous_home: std::env::var_os("HOME"),
        };
        std::env::set_var("OPENCODE_RK_CONFIG_DIR", guard.dir.path());
        std::env::set_var("OPENCODE_CONFIG_DIR", guard.dir.path());
        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("HOME");
        guard
    }

    fn write_config(&self, models: &str, base_url: &str) {
        std::fs::write(
            self.dir.path().join("opencode.json"),
            format!(
                r#"{{"providers":{{"fixture":{{"name":"Fixture","base_url":"{base_url}","models":{models}}}}}}}"#
            ),
        )
        .expect("write own config fixture");
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        restore_env("OPENCODE_RK_CONFIG_DIR", self.previous_rk.take());
        restore_env("OPENCODE_CONFIG_DIR", self.previous_config.take());
        restore_env("XDG_CONFIG_HOME", self.previous_xdg.take());
        restore_env("HOME", self.previous_home.take());
    }
}

fn restore_env(name: &str, value: Option<std::ffi::OsString>) {
    match value {
        Some(value) => std::env::set_var(name, value),
        None => std::env::remove_var(name),
    }
}

async fn model_stub(body: &'static str) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind disposable model stub");
    let origin = format!("http://{}", listener.local_addr().expect("stub address"));
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept model request");
        let mut request = Vec::new();
        let mut chunk = [0_u8; 1024];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let count = stream.read(&mut chunk).await.expect("read model request");
            if count == 0 {
                break;
            }
            request.extend_from_slice(&chunk[..count]);
            assert!(request.len() <= 64 * 1024, "stub request header is bounded");
        }
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("write model response");
    });
    (origin, task)
}

fn app_and_token() -> (axum::Router, String, TempDir) {
    let dir = tempfile::tempdir().expect("disposable storage directory");
    let storage = Storage::open_in_memory(dir.path().join("blobs")).expect("storage");
    let auth = DaemonAuth::mint().expect("daemon auth");
    let token = auth.token().to_owned();
    let app = router_with_auth(
        AppState {
            sessions: SessionService::new(std::sync::Arc::new(storage)),
            catalog: std::sync::Arc::new(Catalog::default()),
        },
        Some(auth),
    );
    (app, token, dir)
}

async fn discover(app: axum::Router, token: &str) -> (StatusCode, serde_json::Value) {
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/providers/fixture/models")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("discovery request"),
        )
        .await
        .expect("discovery response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 256 * 1024)
        .await
        .expect("bounded discovery body");
    let body = serde_json::from_slice(&bytes).expect("JSON discovery response");
    (status, body)
}

#[tokio::test]
async fn np02_route_preserves_configured_and_discovered_context_window() {
    let env = EnvGuard::new();
    let (stub, server) = model_stub(
        r#"{"data":[{"id":"discovered-rich","name":"Discovered Rich","limit":{"context":200000}},{"id":"discovered-unknown","name":"Discovered Unknown"}]}"#,
    )
    .await;
    env.write_config(
        r#"{"configured-rich":{"name":"Configured Rich","limit":{"context":200000}},"configured-unknown":{"name":"Configured Unknown"}}"#,
        &format!("{stub}/v1"),
    );
    let (app, token, _storage) = app_and_token();

    let (status, body) = discover(app, &token).await;
    assert_eq!(status, StatusCode::OK, "authenticated discovery: {body}");
    let models = body["models"].as_array().expect("models array");
    let model = |id: &str| models.iter().find(|entry| entry["id"] == id);
    assert_eq!(model("configured-rich").expect("configured model")["context_window"], 200000);
    assert_eq!(model("discovered-rich").expect("discovered model")["context_window"], 200000);
    for id in ["configured-unknown", "discovered-unknown"] {
        let entry = model(id).expect("unknown-context model");
        assert!(entry.get("context_window").is_none() || entry["context_window"].is_null());
    }
    server.await.expect("model stub");
}

#[tokio::test]
async fn np02_invalid_context_metadata_is_rejected_or_unknown_not_zero() {
    let env = EnvGuard::new();
    let (stub, server) = model_stub(
        r#"{"data":[{"id":"zero","limit":{"context":0}},{"id":"negative","limit":{"context":-1}},{"id":"huge","limit":{"context":18446744073709551615}}]}"#,
    )
    .await;
    env.write_config("{}", &format!("{stub}/v1"));
    let (app, token, _storage) = app_and_token();
    let (status, body) = discover(app, &token).await;
    assert!(
        status == StatusCode::BAD_GATEWAY || status == StatusCode::OK,
        "invalid metadata must fail discovery or be treated as unknown: {status} {body}"
    );
    if status == StatusCode::OK {
        let models = body["models"].as_array().expect("models array");
        assert_eq!(models.len(), 3, "invalid metadata must not drop models: {body}");
        for id in ["zero", "negative", "huge"] {
            let entry = models
                .iter()
                .find(|entry| entry["id"] == id)
                .unwrap_or_else(|| panic!("invalid metadata model {id} was dropped: {body}"));
            assert!(
                entry.get("context_window").is_none()
                    || entry["context_window"].is_null(),
                "invalid metadata must remain unknown rather than become a capacity: {entry}"
            );
        }
    }
    server.await.expect("model stub");
}

#[tokio::test]
async fn np02_nonobject_configured_models_keep_legacy_fallback_ids() {
    let env = EnvGuard::new();
    env.write_config(
        r#"{"scalar-model":"ignored label","null-model":null,"object-model":{"name":"Object Label"}}"#,
        "http://127.0.0.1:9/v1",
    );
    let (app, token, _storage) = app_and_token();
    let (status, body) = discover(app, &token).await;
    assert_eq!(status, StatusCode::OK, "configured fallback: {body}");
    let models = body["models"].as_array().expect("models array");
    assert_eq!(models.iter().map(|model| model["id"].as_str().unwrap()).collect::<Vec<_>>(), [
        "null-model", "object-model", "scalar-model"
    ]);
    assert_eq!(models.iter().find(|model| model["id"] == "scalar-model").unwrap()["name"], "scalar-model");
    assert_eq!(models.iter().find(|model| model["id"] == "null-model").unwrap()["name"], "null-model");
}

#[tokio::test]
async fn np02_configured_metadata_wins_and_missing_context_fills_from_discovery() {
    let env = EnvGuard::new();
    let (stub, server) = model_stub(
        r#"{"data":[{"id":"configured-wins","name":"Discovered Name","limit":{"context":12345}},{"id":"configured-fills","name":"Discovered Fill","limit":{"context":200000}}]}"#,
    )
    .await;
    env.write_config(
        r#"{"configured-wins":{"name":"Configured Name","limit":{"context":99999}},"configured-fills":{"name":"Configured Fill"}}"#,
        &format!("{stub}/v1"),
    );
    let (app, token, _storage) = app_and_token();
    let (status, body) = discover(app, &token).await;
    assert_eq!(status, StatusCode::OK, "metadata merge: {body}");
    let models = body["models"].as_array().expect("models array");
    let wins = models.iter().find(|model| model["id"] == "configured-wins").unwrap();
    assert_eq!(wins["name"], "Configured Name");
    assert_eq!(wins["context_window"], 99999);
    let fills = models.iter().find(|model| model["id"] == "configured-fills").unwrap();
    assert_eq!(fills["name"], "Configured Fill");
    assert_eq!(fills["context_window"], 200000);
    server.await.expect("model stub");
}
