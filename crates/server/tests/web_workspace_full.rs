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

async fn get_body(app: axum::Router, uri: &str) -> Value {
    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn seeded_app(dir: &tempfile::TempDir) -> (axum::Router, std::path::PathBuf) {
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
    CatalogV2::register_workspace(
        &mut catalog,
        &WorkspaceRegistration {
            id: [8_u8; 16],
            label: "Side project".to_owned(),
            project_root: None,
            status: 1,
            created_at_us: 22,
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
    (app, catalog_path)
}

fn unconfigured_app(dir: &tempfile::TempDir) -> axum::Router {
    let storage = Storage::open_in_memory(dir.path().join("blobs")).unwrap();
    router(AppState {
        sessions: SessionService::new(Arc::new(storage)),
        catalog: Arc::new(Catalog::default()),
    })
}

#[tokio::test]
async fn web_015_full_t01_catalog_read_lists_registry_metadata() {
    let dir = tempdir().unwrap();
    let (app, _path) = seeded_app(&dir);
    let body = get_body(app, "/api/workspaces").await;
    assert_eq!(body["available"], true);
    let ws = body["workspaces"].as_array().unwrap();
    assert_eq!(ws.len(), 2);
    assert_eq!(ws[0]["label"], "Main project");
    assert_eq!(ws[0]["project_root"], "/workspace/main");
    assert_eq!(ws[1]["label"], "Side project");
    assert!(ws[1]["project_root"].is_null());
    assert_eq!(body["session_scope_available"], false);
    assert_eq!(body["memory_available"], false);
}

#[tokio::test]
async fn web_015_full_t02_missing_catalog_is_explicitly_unavailable() {
    let dir = tempdir().unwrap();
    let body = get_body(unconfigured_app(&dir), "/api/workspaces").await;
    assert_eq!(body["available"], false);
    assert!(body["workspaces"].as_array().unwrap().is_empty());
    assert_eq!(body["session_scope_available"], false);
    assert_eq!(body["memory_available"], false);
    let reason = body["reason"].as_str().unwrap();
    assert!(!reason.is_empty());
    assert!(reason.contains("not configured"));
}

#[tokio::test]
async fn web_015_full_t03_scope_param_projects_no_membership() {
    let dir = tempdir().unwrap();
    let (app, _path) = seeded_app(&dir);
    let scoped = get_body(app, "/api/workspaces?scope=no-such-scope").await;
    assert!(scoped.get("sessions").is_none());
    assert!(scoped.get("memory").is_none());
    assert_eq!(scoped["session_scope_available"], false);
    assert_eq!(scoped["memory_available"], false);
    // Unknown scope must not invent membership or filter the registry.
    assert_eq!(scoped["workspaces"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn web_015_full_t04_workspace_objects_expose_registry_fields_only() {
    let dir = tempdir().unwrap();
    let (app, _path) = seeded_app(&dir);
    let body = get_body(app, "/api/workspaces").await;
    for entry in body["workspaces"].as_array().unwrap() {
        let obj = entry.as_object().unwrap();
        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["created_at_us", "id", "label", "project_root", "status"]);
    }
    // project_root is registry metadata passthrough, never a live path grant.
    assert_eq!(body["workspaces"][0]["project_root"], "/workspace/main");
    assert!(body.get("sessions").is_none());
    assert!(body.get("memory").is_none());
}

#[tokio::test]
async fn web_015_full_t05_repeated_reads_are_deterministic() {
    let dir = tempdir().unwrap();
    let (app, _path) = seeded_app(&dir);
    let first = get_body(app, "/api/workspaces").await;
    let dir2 = tempdir().unwrap();
    let (app2, _path2) = seeded_app(&dir2);
    let second = get_body(app2, "/api/workspaces").await;
    assert_eq!(first, second);
}
