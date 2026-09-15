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

const EXPECTED_MAX_DRAFT_ATTACHMENT_BYTES: usize = 8 * 1024 * 1024;

fn app() -> (axum::Router, tempfile::TempDir) {
    let dir = tempdir().expect("temporary attachment fixture");
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
    let bytes = to_bytes(response.into_body(), 256 * 1024)
        .await
        .expect("bounded json response");
    serde_json::from_slice(&bytes).expect("json response")
}

async fn create_session(app: &axum::Router) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .body(Body::from(json!({"title":"WEB-011 attachments"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    json_body(response).await["session"]["id"]
        .as_str()
        .expect("session id")
        .to_owned()
}

#[tokio::test]
async fn web_011_t01_attachment_upload_is_bounded_persisted_and_removable() {
    let (app, _dir) = app();
    let session_id = create_session(&app).await;
    let payload = b"small attachment";

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/sessions/{session_id}/attachments?name=notes.txt"
                ))
                .header("content-type", "text/plain")
                .body(Body::from(payload.as_slice()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let created = json_body(response).await["attachment"].clone();
    assert_eq!(created["name"], "notes.txt");
    assert_eq!(created["mime"], "text/plain");
    assert_eq!(created["bytes"], payload.len() as u64);
    assert_eq!(created["hash"].as_str().unwrap_or_default().len(), 64);
    let attachment_id = created["id"].as_str().expect("attachment id");

    let listed = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/sessions/{session_id}/attachments"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    let listed = json_body(listed).await;
    assert_eq!(listed["attachments"].as_array().unwrap().len(), 1);
    assert_eq!(listed["attachments"][0], created);

    let deleted = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/sessions/{session_id}/attachments/{attachment_id}"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);

    let listed = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/sessions/{session_id}/attachments"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(json_body(listed).await["attachments"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn web_011_t02_oversize_attachment_is_rejected_before_metadata_persistence() {
    let (app, _dir) = app();
    let session_id = create_session(&app).await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/sessions/{session_id}/attachments?name=too-large.bin"
                ))
                .header("content-type", "application/octet-stream")
                .body(Body::from(vec![0_u8; EXPECTED_MAX_DRAFT_ATTACHMENT_BYTES + 1]))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        response.status(),
        StatusCode::PAYLOAD_TOO_LARGE | StatusCode::UNPROCESSABLE_ENTITY
    ));

    let listed = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/sessions/{session_id}/attachments"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(json_body(listed).await["attachments"]
        .as_array()
        .unwrap()
        .is_empty());
}
