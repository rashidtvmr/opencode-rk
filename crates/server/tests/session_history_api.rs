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
    let dir = tempdir().unwrap();
    let storage = Storage::open_in_memory(dir.path().join("blobs")).unwrap();
    let sessions = SessionService::new(Arc::new(storage));
    (
        router(AppState {
            sessions,
            catalog: Arc::new(Catalog::default()),
        }),
        dir,
    )
}

async fn body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 128 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn create_session(app: &axum::Router) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .body(Body::from(json!({"title":"History paging"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    body(response).await["session"]["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn append(app: &axum::Router, session_id: &str, text: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{session_id}/messages"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"text":text}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    body(response).await["message"]["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn web_014_t01_recent_history_pages_without_gaps_or_duplicates() {
    let (app, _dir) = app();
    let session_id = create_session(&app).await;
    let mut ids = Vec::new();
    for index in 1..=5 {
        ids.push(append(&app, &session_id, &format!("message {index}")).await);
    }

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/sessions/{session_id}/history?limit=2"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let first = body(first).await;
    let texts = first["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|message| message["body"]["text"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(texts, vec!["message 4", "message 5"]);
    let before = first["next_before"].as_str().expect("older cursor");
    assert_eq!(before, ids[3]);

    let second = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/sessions/{session_id}/history?limit=2&before={before}"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::OK);
    let second = body(second).await;
    let texts = second["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|message| message["body"]["text"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(texts, vec!["message 2", "message 3"]);
    assert_eq!(second["next_before"], ids[1]);

    let third = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/sessions/{session_id}/history?limit=2&before={}",
                    ids[1]
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let third = body(third).await;
    assert_eq!(third["messages"].as_array().unwrap().len(), 1);
    assert_eq!(third["messages"][0]["body"]["text"], "message 1");
    assert!(third["next_before"].is_null());
}

#[tokio::test]
async fn web_014_t02_unknown_history_cursor_is_not_found() {
    let (app, _dir) = app();
    let session_id = create_session(&app).await;
    append(&app, &session_id, "only message").await;
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/sessions/{session_id}/history?limit=2&before=0195f36a-2997-7a89-a11a-fc3359b0ffff"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
