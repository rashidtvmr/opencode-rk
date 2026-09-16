use std::sync::Arc;

use axum::{body::to_bytes, body::Body, http::Request, http::StatusCode};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::Value;
use tempfile::tempdir;
use tower::ServiceExt;

async fn get_capabilities(uri: &str) -> Value {
    let dir = tempdir().unwrap();
    let storage = Storage::open_in_memory(dir.path().join("blobs")).unwrap();
    let app = router(AppState {
        sessions: SessionService::new(Arc::new(storage)),
        catalog: Arc::new(Catalog::default()),
    });
    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 128 * 1024).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn web_013_reason_unavailable_entries_carry_non_empty_reasons() {
    let value = get_capabilities("/api/capabilities").await;
    for key in ["search", "deep_research", "voice"] {
        assert_eq!(value[key]["available"], false);
        let reason = value[key]["reason"].as_str().unwrap();
        assert!(!reason.is_empty(), "{key} reason must be non-empty");
    }
    for tool in value["tools"].as_array().unwrap() {
        assert_eq!(tool["available_for_web_turn"], false);
        let reason = tool["reason"].as_str().unwrap();
        assert!(!reason.is_empty(), "tool reason must be non-empty");
    }
    for key in ["plugins", "approvals", "attachments"] {
        assert_eq!(value[key]["available_for_web_turn"], false);
        let reason = value[key]["reason"].as_str().unwrap();
        assert!(!reason.is_empty(), "{key} reason must be non-empty");
    }
}

#[tokio::test]
async fn web_013_reason_names_adapter_boundary() {
    let value = get_capabilities("/api/capabilities").await;
    assert!(
        value["search"]["reason"].as_str().unwrap().contains("grep"),
        "search reason must name the grep boundary"
    );
    assert!(
            value["deep_research"]["reason"]
                .as_str()
                .unwrap()
                .contains("citation"),
        "research reason must name the citation gap"
    );
    let voice = value["voice"]["reason"].as_str().unwrap();
    assert!(
        voice.contains("transcription") || voice.contains("audio"),
        "voice reason must name the audio gap"
    );
}

#[tokio::test]
async fn web_013_unknown_scope_is_not_silently_available() {
    let value = get_capabilities("/api/capabilities").await;
    assert!(value.get("unknown_scope").is_none());
    assert!(value["search"].get("available_for_web_turn").is_none());
    let scoped = get_capabilities("/api/capabilities?scope=unknown_scope").await;
    assert_eq!(scoped["search"]["available"], false);
    assert_eq!(scoped["deep_research"]["available"], false);
    assert_eq!(scoped["voice"]["available"], false);
}
