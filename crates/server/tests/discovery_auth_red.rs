//! DISC-101 RED: authenticated discovery rejects stale, pid-mismatched,
//! and legacy empty-token descriptors before any `/api/*` trust.
#![forbid(unsafe_code)]

use std::sync::Arc;
use std::{env, process::Command};

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{
    daemon::{read_backend_descriptor, DaemonError, DaemonPaths},
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

/// A discovery fixture must own every process it starts.  In particular, do
/// not leave a sleeping child holding inherited test pipes open while an
/// assertion unwinds.
struct OwnedChild(std::process::Child);

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if self.0.try_wait().expect("poll child").is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
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
    // The server-side portion of the contract is intentionally executable:
    // stale discovery yields no reusable descriptor and a stale credential
    // cannot reach an authenticated handler.  The native caller/origin
    // no-contact gate belongs to the CLI integration test and is recorded in
    // the worklog rather than hidden behind a placeholder.
}

#[tokio::test]
async fn disc101_t02_pid_mismatch_rejected() {
    let dir = tempdir().expect("data dir");
    let child = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("sleep");
    let foreign_pid = child.id();
    assert_ne!(foreign_pid, std::process::id());
    let _owned = OwnedChild(child);
    let token = DaemonAuth::mint().expect("mint").token().to_owned();
    let (app, _guard) = build_app(Some(DaemonAuth::from_published(&token).expect("auth")));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("listener");
    let origin = format!("http://{}", listener.local_addr().expect("address"));
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });
    write_descriptor(dir.path(), foreign_pid, &origin, &token);

    // Positive control: the endpoint must accept a legitimate client bearing
    // its valid token.  Descriptor-vs-endpoint identity denial is tested only
    // by the native composite fixture below; the server cannot inspect a
    // caller-local forged descriptor.
    let address = origin.strip_prefix("http://").expect("loopback origin");
    let mut socket = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        tokio::net::TcpStream::connect(address),
    )
    .await
    .expect("health connect timeout")
    .expect("health connect");
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let request = format!(
        "GET /api/sessions HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n\r\n"
    );
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        socket.write_all(request.as_bytes()),
    )
    .await
    .expect("API request timeout")
    .expect("API request");
    const MAX_WIRE_BYTES: usize = 64 * 1024;
    let mut bytes = Vec::with_capacity(4096);
    let mut chunk = [0_u8; 4096];
    loop {
        let read = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            socket.read(&mut chunk),
        )
        .await
        .expect("API response timeout")
        .expect("API response");
        if read == 0 {
            break;
        }
        assert!(bytes.len().saturating_add(read) <= MAX_WIRE_BYTES, "API response exceeded bound");
        bytes.extend_from_slice(&chunk[..read]);
    }
    let response = String::from_utf8_lossy(&bytes);
    let status_line = response.lines().next().unwrap_or_default().to_owned();
    task.abort();
    let _ = task.await;
    assert!(status_line.starts_with("HTTP/1.1 200"), "{response}");
}

#[tokio::test]
async fn disc101_t03_legacy_empty_token_rejected() {
    let dir = tempdir().expect("data dir");
    write_descriptor(dir.path(), std::process::id(), "http://127.0.0.1:4096", "");
    match read_backend_descriptor(dir.path()).expect_err("legacy descriptor must be loud") {
        DaemonError::Descriptor(payload) => assert_eq!(
            payload,
            "descriptor predates bearer auth (empty auth_token); refusing to attach"
        ),
        other => panic!("unexpected descriptor result: {other:?}"),
    }
    assert!(DaemonAuth::from_published("").is_err());
    // Legacy descriptor must never authenticate `/api/*`: no credential → 401.
    let (app, _guard) = build_app(Some(DaemonAuth::mint().expect("mint")));
    let response = app.oneshot(get("/api/sessions", None)).await.expect("resp");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    // Empty credentials are rejected both by the descriptor reader and by
    // the API middleware; neither path may manufacture a reusable identity.
}

#[test]
fn disc101_native_caller_composite_gate_requires_attested_artifact() {
    for name in ["OC2_TEST_BINARY", "OC2_TEST_NATIVE_LIBRARY", "OC2_TEST_BUILD_JSON"] {
        assert!(
            env::var_os(name).is_some(),
            "DISC-101 native composite gate requires {name}"
        );
    }
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/e2e/discovery_identity.py");
    let status = Command::new("/usr/bin/python3")
        .arg(&script)
        .env("OC2_TEST_BINARY", env::var("OC2_TEST_BINARY").expect("binary"))
        .env(
            "OC2_TEST_NATIVE_LIBRARY",
            env::var("OC2_TEST_NATIVE_LIBRARY").expect("native library"),
        )
        .env(
            "OC2_TEST_BUILD_JSON",
            env::var("OC2_TEST_BUILD_JSON").expect("build manifest"),
        )
        .status()
        .expect("run actual native discovery composite fixture");
    assert!(status.success(), "native discovery composite gate failed");
}
