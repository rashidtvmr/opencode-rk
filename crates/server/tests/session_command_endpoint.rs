//! Real router validation must reject commands before any durable prompt.
use std::sync::Arc;
use axum::{body::Body, http::{Request, StatusCode}};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use tower::ServiceExt;

#[tokio::test]
async fn empty_command_is_validated_and_unknown_command_has_no_history() {
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::open_in_memory(dir.path().join("blobs")).unwrap();
    let app = router(AppState { sessions: SessionService::new(Arc::new(storage)), catalog: Arc::new(Catalog::default()) });
    let post = |uri: String, value: Value| Request::builder().method("POST").uri(uri)
        .header("content-type", "application/json").body(Body::from(value.to_string())).unwrap();
    let response = app.clone().oneshot(post("/api/sessions".into(), json!({"title":"commands", "directory":dir.path()}))).await.unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), 65536).await.unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    let id = value["session"]["id"].as_str().unwrap();
    for (name, expected) in [("", StatusCode::BAD_REQUEST), ("missing-command-fixture", StatusCode::NOT_FOUND)] {
        let response = app.clone().oneshot(post(format!("/api/sessions/{id}/command"),
            json!({"command":name,"arguments":"untouched","model":"openai/gpt-5.6"}))).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 65536).await.unwrap();
        assert_eq!(status, expected, "{}", String::from_utf8_lossy(&bytes));
    }
    let response = app.oneshot(Request::builder().uri(format!("/api/sessions/{id}/messages")).body(Body::empty()).unwrap()).await.unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), 65536).await.unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["messages"], json!([]));
}
