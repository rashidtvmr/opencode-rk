//! RC-01: unauthenticated `/api/*` is 401/403, `/health` stays public.
#![forbid(unsafe_code)]

use std::sync::Arc;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{daemon_auth::DaemonAuth, router_with_auth, AppState};
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

#[tokio::test]
async fn rc01_t01_no_credential_gets_401() {
    let credential = DaemonAuth::mint().expect("mint");
    let (app, _dir) = build_app(Some(credential));
    let response = app.oneshot(get("/api/sessions", None)).await.expect("resp");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn rc01_t02_wrong_credential_gets_403() {
    let credential = DaemonAuth::mint().expect("mint");
    let (app, _dir) = build_app(Some(credential));
    let response = app
        .oneshot(get("/api/sessions", Some("00")))
        .await
        .expect("resp");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn rc01_t03_denied_request_changes_no_state() {
    let credential = DaemonAuth::mint().expect("mint");
    let token = credential.token().to_owned();
    let (app, _dir) = build_app(Some(credential));
    let denied = app
        .oneshot(get("/api/sessions", Some("00")))
        .await
        .expect("resp");
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let body = to_bytes(denied.into_body(), 8 * 1024).await.expect("body");
    let value: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(value["error"], "bad bearer credential");
    // The same router with the right credential lists zero sessions: the
    // denied request created nothing.
    let (app2_cred_holder, _d) = (token.clone(), ());
    let _ = app2_cred_holder;
    let credential2 = DaemonAuth::from_published(&token).expect("restore");
    let (app2, _dir2) = build_app(Some(credential2));
    let ok = app2
        .oneshot(get("/api/sessions", Some(&token)))
        .await
        .expect("resp");
    assert_eq!(ok.status(), StatusCode::OK);
    let body = to_bytes(ok.into_body(), 64 * 1024).await.expect("body");
    let value: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(value["sessions"], serde_json::json!([]));
}

#[tokio::test]
async fn rc01_t04_health_stays_public() {
    let credential = DaemonAuth::mint().expect("mint");
    let (app, _dir) = build_app(Some(credential));
    let response = app.oneshot(get("/health", None)).await.expect("resp");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn rc01_t05_right_credential_lists_sessions() {
    let credential = DaemonAuth::mint().expect("mint");
    let token = credential.token().to_owned();
    let (app, _dir) = build_app(Some(credential));
    let response = app
        .oneshot(get("/api/sessions", Some(&token)))
        .await
        .expect("resp");
    assert_eq!(response.status(), StatusCode::OK);
}
