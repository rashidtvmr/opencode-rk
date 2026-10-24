//! RED contract for per-session loaded-instruction (`/memory`) inspection.
//!
//! Integration test against the real public router, storage fixture, and session
//! creation endpoint. Must fail (404 from absent route) on current server
//! because `GET /api/sessions/{id}/memory` is not registered yet. Do not
//! replace the request with a handler/unit mock.
//!
//! Upstream behavior: `packages/opencode/src/session/instruction.ts`
//! `systemPaths()` selects `AGENTS.md` / `CLAUDE.md` from the workspace root
//! (first project-level match wins via `findUp`). The memory inspector must
//! return the actually loaded source files with their canonical path, content,
//! and inclusion state. Provenance follows `Instructions from: <path>` prefix
//! shape from `Instruction.system()`.
//!
//! No new production structs. Uses only existing public APIs:
//! `router`, `AppState`, `SessionService`, `Storage`, `Catalog`,
//! `daemon_auth::DaemonAuth`, `router_with_auth`.

#![forbid(unsafe_code)]

use std::sync::Arc;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{daemon_auth::DaemonAuth, router_with_auth, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

/// Sentinel content written into AGENTS.md to verify provenance in the memory
/// response. Not a real user instruction.
const AGENTS_SENTINEL: &str = "You are a helpful assistant. Follow the project rules.";

/// Build an auth-gated app + temp workspace with AGENTS.md.
///
/// The TempDir is returned so the caller keeps the fixture alive for the
/// duration of the test (AGENTS.md must exist when the memory endpoint reads
/// it). workspace_root is the workspace root path string.
fn build_app_with_workspace(
    auth: DaemonAuth,
) -> (axum::Router, String, DaemonAuth, tempfile::TempDir) {
    let dir = tempdir().expect("disposable storage fixture");
    let storage = Storage::open_in_memory(dir.path().join("blobs")).expect("storage fixture");
    let sessions = SessionService::new(Arc::new(storage));
    let app = router_with_auth(
        AppState {
            sessions: sessions.clone(),
            catalog: Arc::new(Catalog::default()),
        },
        Some(auth.clone()),
    );
    // AGENTS.md at workspace root -- the only discoverable instruction file
    // with no ancestor chain above a tempdir.
    std::fs::write(dir.path().join("AGENTS.md"), AGENTS_SENTINEL)
        .expect("write AGENTS.md fixture");
    let workspace_path = dir
        .path()
        .to_str()
        .expect("workspace path is utf-8")
        .to_owned();
    (app, workspace_path, auth, dir)
}

/// Create a session in the given workspace directory via the real router API
/// with an explicit bearer token.
async fn create_session_in_dir_with_token(
    app: &axum::Router,
    directory: &str,
    token: &str,
) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sessions")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(
                    json!({
                        "title": "Memory RED",
                        "directory": directory,
                    })
                    .to_string(),
                ))
                .expect("create request"),
        )
        .await
        .expect("create response");
    assert_eq!(response.status(), StatusCode::CREATED);
    let bytes = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("bounded JSON response");
    let value: Value = serde_json::from_slice(&bytes).expect("JSON response");
    value["session"]["id"]
        .as_str()
        .expect("created session id")
        .to_owned()
}

/// RED: GET /api/sessions/{id}/memory returns 200 with loaded instruction
/// sources including the workspace AGENTS.md content.
#[tokio::test]
async fn session_memory_get_returns_loaded_sources() {
    let credential = DaemonAuth::mint().expect("daemon auth");
    let token = credential.token().to_owned();
    let (app, workspace_path, _auth, _dir) = build_app_with_workspace(credential);
    let session_id = create_session_in_dir_with_token(&app, &workspace_path, &token).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/sessions/{session_id}/memory"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("memory request"),
        )
        .await
        .expect("memory response");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "route must be registered and return 200"
    );
    let bytes = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("bounded JSON response");
    let body: Value = serde_json::from_slice(&bytes).expect("JSON response");

    // The response must use the upstream-defined `sources` shape.
    let sources = body["sources"]
        .as_array()
        .expect("response has `sources` array");
    assert!(
        !sources.is_empty(),
        "at least one loaded instruction source expected"
    );

    // Every source must have path, content, and included fields.
    for src in sources {
        assert!(src["path"].is_string(), "source has `path` string");
        assert!(src["content"].is_string(), "source has `content` string");
        assert!(src["included"].is_boolean(), "source has `included` boolean");
    }

    // Exactly one source should be the AGENTS.md at workspace root.
    let agents_entry = sources
        .iter()
        .find(|s| {
            s["path"]
                .as_str()
                .map(|p| p.ends_with("AGENTS.md"))
                .unwrap_or(false)
        })
        .expect("AGENTS.md should appear in loaded sources");

    assert_eq!(
        agents_entry["content"].as_str().expect("content string"),
        AGENTS_SENTINEL,
        "content must match actually read file bytes"
    );
    assert!(
        agents_entry["included"].as_bool().expect("included boolean"),
        "AGENTS.md should be included by default"
    );
}

/// RED: unknown session returns 404.
#[tokio::test]
async fn session_memory_unknown_session_returns_404() {
    let credential = DaemonAuth::mint().expect("daemon auth");
    let token = credential.token().to_owned();
    let (app, _workspace, _auth, _dir) = build_app_with_workspace(credential);

    let fake_id = "00000000-0000-0000-0000-000000000000";
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/sessions/{fake_id}/memory"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("memory request"),
        )
        .await
        .expect("memory response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// 401 for missing bearer token (fixture security middleware already handles
/// `/api/sessions/{id}/memory` under `/api/` since the route will be registered
/// under that prefix). This test passes against the existing middleware.
#[tokio::test]
async fn session_memory_missing_bearer_returns_401() {
    let credential = DaemonAuth::mint().expect("daemon auth");
    let (app, _workspace, _auth, _dir) = build_app_with_workspace(credential);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sessions/nonexistent/memory")
                .body(Body::empty())
                .expect("memory request"),
        )
        .await
        .expect("memory response");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// RED: excluded source is marked `included: false` but content is still
/// available for inspection (source file is never deleted or modified).
///
/// This test documents the exclusion contract. It will remain RED until the
/// `POST /api/sessions/{id}/memory/exclude` endpoint is implemented and
/// session-level selection is wired into prompt assembly.
#[tokio::test]
async fn session_memory_excluded_source_retains_content() {
    let credential = DaemonAuth::mint().expect("daemon auth");
    let token = credential.token().to_owned();
    let (app, workspace_path, _auth, _dir) = build_app_with_workspace(credential);
    let session_id = create_session_in_dir_with_token(&app, &workspace_path, &token).await;
    let agents_path = format!("{workspace_path}/AGENTS.md");

    // Exclusion requires a production control endpoint that does not exist yet.
    let exclude_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{session_id}/memory/exclude"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "path": agents_path,
                        "scope": "session",
                    })
                    .to_string(),
                ))
                .expect("exclude request"),
        )
        .await
        .expect("exclude response");

    // The exclude endpoint must be registered (RED until then: 404).
    assert_eq!(
        exclude_response.status(),
        StatusCode::OK,
        "exclude endpoint must be registered"
    );

    // After exclusion, the memory endpoint should still report the source
    // but with `included: false`, and content must still be present.
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/sessions/{session_id}/memory"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("memory request"),
        )
        .await
        .expect("memory response");

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("bounded JSON response");
    let body: Value = serde_json::from_slice(&bytes).expect("JSON response");
    let sources = body["sources"]
        .as_array()
        .expect("response has `sources` array");

    let agents_entry = sources
        .iter()
        .find(|s| s["path"].as_str().map(|p| p.ends_with("AGENTS.md")).unwrap_or(false))
        .expect("AGENTS.md should still appear after exclusion");

    assert_eq!(
        agents_entry["included"].as_bool().expect("included boolean"),
        false,
        "excluded source must be marked not included"
    );
    assert_eq!(
        agents_entry["content"].as_str().expect("content string"),
        AGENTS_SENTINEL,
        "content must remain available for inspection"
    );

    // Source file must still exist byte-for-byte (never deleted).
    let original_bytes = std::fs::read_to_string(
        std::path::Path::new(&workspace_path).join("AGENTS.md"),
    )
    .expect("AGENTS.md fixture file must still exist");
    assert_eq!(
        original_bytes, AGENTS_SENTINEL,
        "source file must be unchanged by session-scoped exclusion"
    );
}

/// RED: re-enable after exclusion restores inclusion in the memory
/// response. "Re-enable" means restoring the session-level selection, not
/// reloading file content from disk — the memory endpoint reads fresh each
/// time.
#[tokio::test]
async fn session_memory_reload_restores_inclusion() {
    let credential = DaemonAuth::mint().expect("daemon auth");
    let token = credential.token().to_owned();
    let (app, workspace_path, _auth, _dir) = build_app_with_workspace(credential);
    let session_id = create_session_in_dir_with_token(&app, &workspace_path, &token).await;
    let agents_path = format!("{workspace_path}/AGENTS.md");

    // Exclude.
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{session_id}/memory/exclude"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "path": agents_path, "scope": "session" }).to_string(),
                ))
                .expect("exclude request"),
        )
        .await
        .expect("exclude response");

    // Re-enable (POST /api/sessions/{id}/memory/include).
    let include_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{session_id}/memory/include"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "path": agents_path, "scope": "session" }).to_string(),
                ))
                .expect("include request"),
        )
        .await
        .expect("include response");

    assert_eq!(
        include_response.status(),
        StatusCode::OK,
        "include endpoint must be registered"
    );

    // After re-enable, AGENTS.md should be included again.
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/sessions/{session_id}/memory"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("memory request"),
        )
        .await
        .expect("memory response");

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("bounded JSON response");
    let body: Value = serde_json::from_slice(&bytes).expect("JSON response");
    let sources = body["sources"]
        .as_array()
        .expect("response has `sources` array");

    let agents_entry = sources
        .iter()
        .find(|s| s["path"].as_str().map(|p| p.ends_with("AGENTS.md")).unwrap_or(false))
        .expect("AGENTS.md should appear in loaded sources after reload");

    assert!(
        agents_entry["included"].as_bool().expect("included boolean"),
        "AGENTS.md should be included again after reload"
    );
}
