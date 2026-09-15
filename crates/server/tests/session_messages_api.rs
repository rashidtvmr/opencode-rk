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
use tempfile::tempdir;
use tower::ServiceExt;

fn app() -> (axum::Router, tempfile::TempDir) {
    let dir = tempdir().expect("temporary server fixture");
    let storage = Storage::open_in_memory(dir.path().join("blobs")).expect("storage fixture");
    let sessions = SessionService::new(Arc::new(storage));
    (
        router(AppState {
            sessions,
            catalog: Arc::new(Catalog::default()),
        }),
        dir,
    )
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("bounded response body");
    serde_json::from_slice(&bytes).expect("json response")
}

#[tokio::test]
async fn session_message_http_boundary_round_trips_user_text() {
    let (app, _dir) = app();

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .body(Body::from(json!({"title":"Web transcript"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let created = json_body(create).await;
    let session_id = created["session"]["id"].as_str().expect("session id");

    let post = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{session_id}/messages"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"text":"persist me"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(post.status(), StatusCode::CREATED);
    let posted = json_body(post).await;
    assert_eq!(posted["message"]["role"], "user");
    assert_eq!(posted["message"]["body"]["storage"], "inline");
    assert_eq!(posted["message"]["body"]["text"], "persist me");

    let get = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/sessions/{session_id}/messages?limit=50"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get.status(), StatusCode::OK);
    let listed = json_body(get).await;
    assert_eq!(listed["messages"].as_array().unwrap().len(), 1);
    assert_eq!(listed["messages"][0]["body"]["text"], "persist me");
}
