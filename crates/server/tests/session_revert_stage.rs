//! SIDE-10: `POST /api/sessions/{id}/revert/stage` records the revert
//! boundary and `get()` serializes `revert.messageID`.
use std::sync::Arc;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use tower::ServiceExt;

const STAGED_MESSAGE_ID: &str = "00000000-0000-4000-8000-000000000002";

#[tokio::test]
async fn stage_records_revert_message_id() {
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::open_in_memory(dir.path().join("blobs")).unwrap();
    let app = router(AppState {
        sessions: SessionService::new(Arc::new(storage)),
        catalog: Arc::new(Catalog::default()),
    });
    let post = |uri: String, value: Value| {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(value.to_string()))
            .unwrap()
    };

    let response = app
        .clone()
        .oneshot(post(
            "/api/sessions".into(),
            json!({"title": "revert", "directory": dir.path()}),
        ))
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), 65536).await.unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    let id = value["session"]["id"].as_str().unwrap().to_owned();

    let response = app
        .clone()
        .oneshot(post(
            format!("/api/sessions/{id}/revert/stage"),
            json!({"messageID": STAGED_MESSAGE_ID}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 65536).await.unwrap();
    let staged: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        staged["session"]["revert"]["messageID"],
        json!(STAGED_MESSAGE_ID)
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/sessions/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 65536).await.unwrap();
    let fetched: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        fetched["session"]["revert"]["messageID"],
        json!(STAGED_MESSAGE_ID)
    );
}
