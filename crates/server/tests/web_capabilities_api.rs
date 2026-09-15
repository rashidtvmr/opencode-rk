use std::sync::Arc;

use axum::{body::to_bytes, body::Body, http::Request, http::StatusCode};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::Value;
use tempfile::tempdir;
use tower::ServiceExt;

#[tokio::test]
async fn web_012_t01_daemon_reports_real_tools_and_unavailable_web_adapters() {
    let dir = tempdir().unwrap();
    let storage = Storage::open_in_memory(dir.path().join("blobs")).unwrap();
    let app = router(AppState {
        sessions: SessionService::new(Arc::new(storage)),
        catalog: Arc::new(Catalog::default()),
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/capabilities")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 128 * 1024).await.unwrap();
    let value: Value = serde_json::from_slice(&body).unwrap();

    let ids = value["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["bash", "edit", "file", "grep", "read", "write"]);
    for tool in value["tools"].as_array().unwrap() {
        assert_eq!(tool["enabled"], true);
        assert_eq!(tool["available_for_web_turn"], false);
        assert!(tool["reason"].as_str().unwrap().contains("turn adapter"));
    }

    assert_eq!(value["attachments"]["draft_ingest"], true);
    assert_eq!(value["attachments"]["available_for_web_turn"], false);
    assert_eq!(value["search"]["available"], false);
    assert_eq!(value["deep_research"]["available"], false);
    assert_eq!(value["voice"]["available"], false);
    assert_eq!(value["plugins"]["available_for_web_turn"], false);
    assert_eq!(value["approvals"]["available_for_web_turn"], false);
}
