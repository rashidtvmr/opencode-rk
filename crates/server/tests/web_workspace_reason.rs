use std::sync::Arc;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::{catalog_v2::WorkspaceRegistration, CatalogV2, Storage};
use serde_json::Value;
use tempfile::tempdir;
use tower::ServiceExt;

async fn get_workspaces(app: axum::Router, uri: &str) -> Value {
    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn app_without_catalog(dir: &tempfile::TempDir) -> axum::Router {
    let storage = Storage::open_in_memory(dir.path().join("blobs")).unwrap();
    router(AppState {
        sessions: SessionService::new(Arc::new(storage)),
        catalog: Arc::new(Catalog::default()),
    })
}

#[tokio::test]
async fn web_015_reason_missing_catalog_carries_explicit_reason() {
    let dir = tempdir().unwrap();
    let body = get_workspaces(app_without_catalog(&dir), "/api/workspaces").await;
    assert_eq!(body["available"], false);
    assert!(body["workspaces"].as_array().unwrap().is_empty());
    assert_eq!(body["session_scope_available"], false);
    assert_eq!(body["memory_available"], false);
    let reason = body["reason"].as_str().unwrap();
    assert!(!reason.is_empty(), "missing-catalog reason must be non-empty");
    assert!(
        reason.contains("not configured"),
        "missing-catalog reason must state the catalog is not configured"
    );
}

#[tokio::test]
async fn web_015_reason_present_catalog_carries_boundary_reason() {
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
    let body = get_workspaces(app, "/api/workspaces").await;
    assert_eq!(body["available"], true);
    assert_eq!(body["session_scope_available"], false);
    assert_eq!(body["memory_available"], false);
    assert!(
        !body["reason"].as_str().unwrap().is_empty(),
        "present-catalog reason must be non-empty"
    );
}

#[tokio::test]
async fn web_015_reason_unknown_scope_projects_no_membership() {
    let dir = tempdir().unwrap();
    let body = get_workspaces(app_without_catalog(&dir), "/api/workspaces").await;
    assert!(body.get("sessions").is_none());
    assert!(body.get("memory").is_none());
    let scoped =
        get_workspaces(app_without_catalog(&dir), "/api/workspaces?scope=no-such-scope").await;
    assert_eq!(scoped["available"], false);
    assert!(scoped["workspaces"].as_array().unwrap().is_empty());
    assert_eq!(scoped["session_scope_available"], false);
    assert_eq!(scoped["memory_available"], false);
}
