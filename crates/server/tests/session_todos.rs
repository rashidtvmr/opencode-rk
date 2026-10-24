//! RED contract for persisted per-session context todos.
//!
//! This is intentionally an integration test against the real public router,
//! storage fixture, and session creation endpoint.  It must fail on the
//! current server because the todo route is not registered yet; do not replace
//! the request with a handler/unit mock.

#![forbid(unsafe_code)]

use std::sync::Arc;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use tempfile::{tempdir, TempDir};
use tower::ServiceExt;

fn app() -> (axum::Router, SessionService, TempDir) {
    let dir = tempdir().expect("disposable storage fixture");
    let storage = Storage::open_in_memory(dir.path().join("blobs")).expect("storage fixture");
    let sessions = SessionService::new(Arc::new(storage));
    let app = router(AppState {
        sessions: sessions.clone(),
        catalog: Arc::new(Catalog::default()),
    });
    (app, sessions, dir)
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("bounded JSON response");
    serde_json::from_slice(&bytes).expect("JSON response")
}

async fn create_session(app: &axum::Router) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .body(Body::from(json!({"title":"Todo RED"}).to_string()))
                .expect("create request"),
        )
        .await
        .expect("create response");
    assert_eq!(response.status(), StatusCode::CREATED);
    json_body(response).await["session"]["id"]
        .as_str()
        .expect("created session id")
        .to_owned()
}

/// RED: the native API must expose a real persisted todo collection for every
/// existing session, including a newly created session with no items.
#[tokio::test]
async fn session_todo_get_fresh_session_returns_empty_collection() {
    let (app, _sessions, _dir) = app();
    let session_id = create_session(&app).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/sessions/{session_id}/todo"))
                .body(Body::empty())
                .expect("todo request"),
        )
        .await
        .expect("todo response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body, json!({"todos": []}));
}
