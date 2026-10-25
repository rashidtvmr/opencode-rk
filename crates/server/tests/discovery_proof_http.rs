#![forbid(unsafe_code)]

use std::sync::Arc;
use axum::{body::Body, http::{Request, StatusCode}};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{discovery_proof, daemon_auth::DaemonAuth, router_with_auth, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use tower::ServiceExt;

fn app(auth: DaemonAuth) -> (axum::Router, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("fixture");
    let storage = Storage::open_in_memory(dir.path().join("blobs")).expect("storage");
    (router_with_auth(AppState { sessions: SessionService::new(Arc::new(storage)), catalog: Arc::new(Catalog::default()) }, Some(auth)), dir)
}

fn request(path: &str, host: &str) -> Request<Body> {
    Request::builder().uri(path).header("Host", host).body(Body::empty()).unwrap()
}

#[tokio::test]
async fn malformed_unicode_nonce_is_bad_request_without_panic() {
    let auth = DaemonAuth::mint().unwrap().with_discovery_origin("http://127.0.0.1:4096".into());
    let decoded = format!("aé{}", "a".repeat(61));
    assert_eq!(decoded.as_bytes().len(), 64);
    let malformed = format!("/discovery-proof?nonce=a%C3%A9{}", "a".repeat(61));
    let (app, _fixture) = app(auth);
    let response = app.oneshot(request(&malformed, "127.0.0.1:4096")).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn valid_proof_uses_configured_origin_not_request_host() {
    let auth = DaemonAuth::mint().unwrap().with_discovery_origin("http://127.0.0.1:4096".into());
    let token = auth.token().to_owned();
    let nonce = [7u8; discovery_proof::NONCE_BYTES];
    let nonce_hex = nonce.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let (app, _fixture) = app(auth);
    let response = app.oneshot(request(&format!("/discovery-proof?nonce={nonce_hex}"), "127.0.0.1:9999")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 1024).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["http_origin"], "http://127.0.0.1:4096");
    assert_eq!(value["mac"], discovery_proof::mac(&token, &nonce, "http://127.0.0.1:4096", 1, std::process::id()).unwrap());
}

#[tokio::test]
async fn unconfigured_origin_fails_closed() {
    let (app, _fixture) = app(DaemonAuth::mint().unwrap());
    let response = app.oneshot(request("/discovery-proof?nonce=0000000000000000000000000000000000000000000000000000000000000000", "127.0.0.1:4096")).await.unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
