use std::sync::Arc;

use axum::{body::{to_bytes, Body}, http::{Request, StatusCode}};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::{catalog_v2::WorkspaceRegistration, CatalogV2, Storage};
use serde_json::Value;
use tempfile::tempdir;
use tower::ServiceExt;

#[tokio::test]
async fn web_015_t01_workspace_registry_is_read_from_canonical_catalog() {
    let dir = tempdir().unwrap();
    let catalog_path = dir.path().join("catalog.db");
    let mut catalog = CatalogV2::initialize_catalog(&catalog_path, [7_u8; 16], 10).unwrap();
    CatalogV2::register_workspace(
        &mut catalog,
        &WorkspaceRegistration {
            id: [9_u8; 16],
            label: "Main project".to_owned(),
            project_root: Some("/workspace/main".to_owned()),
            status: 0,
            created_at_us: 11,
        },
    )
    .unwrap();
    drop(catalog);

    let storage = Storage::open_in_memory(dir.path().join("blobs")).unwrap();
    let sessions = SessionService::new(Arc::new(storage))
        .with_workspace_catalog_path(&catalog_path)
        .unwrap();
    let app = router(AppState {
        sessions,
        catalog: Arc::new(Catalog::default()),
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/workspaces")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["available"], true);
    assert_eq!(body["workspaces"].as_array().unwrap().len(), 1);
    assert_eq!(body["workspaces"][0]["label"], "Main project");
    assert_eq!(body["workspaces"][0]["project_root"], "/workspace/main");
    assert_eq!(body["workspaces"][0]["status"], 0);
    assert_eq!(body["session_scope_available"], false);
    assert_eq!(body["memory_available"], false);
}

#[tokio::test]
async fn web_015_t02_missing_catalog_is_explicitly_unavailable() {
    let dir = tempdir().unwrap();
    let storage = Storage::open_in_memory(dir.path().join("blobs")).unwrap();
    let app = router(AppState {
        sessions: SessionService::new(Arc::new(storage)),
        catalog: Arc::new(Catalog::default()),
    });
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/workspaces")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["available"], false);
    assert!(body["workspaces"].as_array().unwrap().is_empty());
}
