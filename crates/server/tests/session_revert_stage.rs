//! SIDE-10: `POST /api/sessions/{id}/revert/stage` records the revert
//! boundary and `get()` serializes `revert.messageID`.
//! MSG-009: the same route also reports the real boundary-onward user-message
//! count (`index.tsx:1561`) the TUI revert card paints.
use std::sync::Arc;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_contracts::{MessageRole, SessionId};
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use tower::ServiceExt;

const STAGED_MESSAGE_ID: &str = "00000000-0000-4000-8000-000000000002";

/// MSG-009: the revert card count is the number of user-role messages from the
/// boundary (inclusive) onward, matching upstream
/// `messagesFromRevert().filter(type === "user").length` (`index.tsx:186-190`).
#[tokio::test]
async fn stage_reports_real_user_message_count_from_boundary() {
    let dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap());
    let sessions = SessionService::new(Arc::clone(&storage));
    let app = router(AppState {
        sessions: sessions.clone(),
        catalog: Arc::new(Catalog::default()),
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .body(Body::from(json!({"title": "count"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), 65536).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    let id = body["session"]["id"].as_str().unwrap().to_owned();
    let session: SessionId = id.parse().unwrap();

    // user, assistant, user — a boundary at the first user counts both users.
    let first = sessions
        .append_text(session, MessageRole::User, "one".to_owned())
        .await
        .unwrap();
    sessions
        .append_text(session, MessageRole::Assistant, "reply".to_owned())
        .await
        .unwrap();
    sessions
        .append_text(session, MessageRole::User, "two".to_owned())
        .await
        .unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{id}/revert/stage"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"messageID": first.id.to_string()}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 65536).await.unwrap();
    let staged: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(staged["session"]["revert"]["count"], json!(2));

    // A boundary id absent from the window reports 0 rather than a fabricated
    // count.
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{id}/revert/stage"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"messageID": STAGED_MESSAGE_ID}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), 65536).await.unwrap();
    let staged: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(staged["session"]["revert"]["count"], json!(0));
}

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
