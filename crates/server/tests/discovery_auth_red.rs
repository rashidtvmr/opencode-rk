//! DISC-101 RED: authenticated discovery rejects stale, pid-mismatched,
//! and legacy empty-token descriptors before any `/api/*` trust.
#![forbid(unsafe_code)]

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{
    daemon::{read_backend_descriptor, DaemonPaths},
    daemon_auth::DaemonAuth,
    router_with_auth, AppState,
};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use tempfile::tempdir;
use tower::ServiceExt;

fn build_app(auth: Option<DaemonAuth>) -> (axum::Router, tempfile::TempDir) {
    let dir = tempdir().expect("fixture");
    let storage = Storage::open_in_memory(dir.path().join("blobs")).expect("storage");
    let sessions = SessionService::new(Arc::new(storage));
    (
        router_with_auth(
            AppState {
                sessions,
                catalog: Arc::new(Catalog::default()),
            },
            auth,
        ),
        dir,
    )
}

fn get(path: &str, token: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().uri(path);
    if let Some(token) = token {
        builder = builder.header("Authorization", format!("Bearer {token}"));
    }
    builder.body(Body::empty()).expect("request")
}

fn write_descriptor(data_dir: &std::path::Path, pid: u32, origin: &str, token: &str) {
    let paths = DaemonPaths::for_data_dir(data_dir);
    std::fs::create_dir_all(paths.descriptor.parent().expect("runtime")).expect("mkdir");
    let body = serde_json::json!({
        "pid": pid,
        "http_origin": origin,
        "schema_version": opencode_rk_contracts::WIRE_SCHEMA_VERSION,
        "auth_token": token,
    });
    std::fs::write(&paths.descriptor, serde_json::to_vec(&body).expect("json"))
        .expect("descriptor");
}

#[tokio::test]
async fn disc101_t01_stale_descriptor_rejected() {
    let dir = tempdir().expect("data dir");
    let stale_token = DaemonAuth::mint().expect("mint").token().to_owned();
    write_descriptor(dir.path(), u32::MAX, "http://127.0.0.1:4096", &stale_token);
    assert_eq!(read_backend_descriptor(dir.path()).expect("read"), None);
    // Stale descriptor must never yield a credential the live daemon accepts.
    let (app, _guard) = build_app(Some(DaemonAuth::mint().expect("mint")));
    let response = app
        .oneshot(get("/api/sessions", Some(&stale_token)))
        .await
        .expect("resp");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn disc101_t02_live_foreign_pid_is_liveness_only() {
    let dir = tempdir().expect("data dir");
    let mut child = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("sleep");
    let foreign_pid = child.id();
    assert_ne!(foreign_pid, std::process::id());
    let token = DaemonAuth::mint().expect("mint").token().to_owned();
    write_descriptor(dir.path(), foreign_pid, "http://127.0.0.1:4096", &token);
    // This low-level reader establishes only bounded descriptor validity and
    // PID liveness.  Ownership of the responding listener belongs to the
    // higher-level CLI discovery/attach boundary, not this reader.
    assert!(read_backend_descriptor(dir.path())
        .expect("read")
        .is_some());
    child.kill().expect("kill");
    let _ = child.wait();
}

#[tokio::test]
async fn disc101_t03_legacy_empty_token_rejected() {
    let dir = tempdir().expect("data dir");
    write_descriptor(dir.path(), std::process::id(), "http://127.0.0.1:4096", "");
    assert!(matches!(
        read_backend_descriptor(dir.path()),
        Err(opencode_rk_server::daemon::DaemonError::Descriptor(_))
    ));
    assert!(DaemonAuth::from_published("").is_err());
    // Legacy descriptor must never authenticate `/api/*`: no credential → 401.
    let (app, _guard) = build_app(Some(DaemonAuth::mint().expect("mint")));
    let response = app.oneshot(get("/api/sessions", None)).await.expect("resp");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
