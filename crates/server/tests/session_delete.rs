//! DIALOG-13: `DELETE /api/session/:sessionID` (`session.remove`,
//! `packages/protocol/src/groups/session.ts:236`) returns `204 NoContent`,
//! removes the session and its dependent rows (schema `ON DELETE CASCADE`),
//! publishes `session.deleted`, and returns `404` for an unknown id
//! (`SessionNotFoundError`).
use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_contracts::{ArtifactKind, MessageId, MessageRole, SessionId, Timestamp};
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use tower::ServiceExt;

fn app_with_sessions() -> (axum::Router, SessionService, Arc<Storage>) {
    let dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap());
    let sessions = SessionService::new(Arc::clone(&storage));
    let app = router(AppState {
        sessions: sessions.clone(),
        catalog: Arc::new(Catalog::default()),
    });
    // The tempdir is leaked into the process for the lifetime of the test; the
    // in-memory storage keeps the blobs dir alive via its own handle.
    std::mem::forget(dir);
    (app, sessions, storage)
}

async fn create_session(app: &axum::Router, title: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .body(Body::from(json!({"title": title}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(response.status().is_success(), "create failed: {}", response.status());
    let bytes = axum::body::to_bytes(response.into_body(), 65536).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    body["session"]["id"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn delete_session_returns_no_content_and_removes_the_row() {
    let (app, _sessions, _storage) = app_with_sessions();
    let id = create_session(&app, "delete me").await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/sessions/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // The row is gone: GET now 404s.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/sessions/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // And it is absent from the list.
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/sessions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), 65536).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    let ids: Vec<&str> = body["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|row| row["id"].as_str())
        .collect();
    assert!(!ids.contains(&id.as_str()), "deleted session still listed: {ids:?}");
}

#[tokio::test]
async fn delete_unknown_session_returns_not_found() {
    let (app, _sessions, _storage) = app_with_sessions();
    let missing = SessionId::new();
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/sessions/{missing}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_session_cascades_dependent_messages() {
    let (app, sessions, _storage) = app_with_sessions();
    let id = create_session(&app, "with messages").await;
    let session: SessionId = id.parse().unwrap();

    sessions
        .append_text(session, MessageRole::User, "hello".to_owned())
        .await
        .unwrap();
    sessions
        .append_text(session, MessageRole::Assistant, "world".to_owned())
        .await
        .unwrap();
    assert_eq!(sessions.messages(session, 100).await.unwrap().len(), 2);

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/sessions/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // The dependent messages are gone with the row (no orphaned history).
    let remaining = sessions.messages(session, 100).await;
    assert!(
        remaining.is_err() || remaining.unwrap().is_empty(),
        "dependent messages survived the session delete"
    );
}

#[tokio::test]
async fn delete_session_removes_artifacts_and_their_versions() {
    let (app, _sessions, storage) = app_with_sessions();
    let id = create_session(&app, "with artifact").await.parse::<SessionId>().unwrap();
    let artifact = storage
        .create_artifact(
            id,
            MessageId::new(),
            ArtifactKind::Code,
            "example",
            Some("rs"),
            "fn main() {}",
            Timestamp::now(),
        )
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/sessions/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(storage.list_artifacts(id, 10).unwrap().is_empty());
    assert!(storage.get_artifact(id, artifact.summary.id).is_err());
}
