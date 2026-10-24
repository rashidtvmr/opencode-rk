//! NATIVE-CONTEXT-CONTROLS-004 / package004A RED contract.
//!
//! This is deliberately an installed public-router probe, not a unit test of
//! `McpStatusRegistry`.  It configures two real local MCP command entries,
//! checks that both remain visible with their requested enabled flag even
//! before transport readiness, then asks the real `/connect` route to perform
//! an initialize + tools/list handshake against a disposable stdio fixture.
//!
//! The current implementation is expected to fail this test: enabled local
//! configs are rejected with `501 mcp_config_unsupported`, the registry has no
//! transport, and the status projection has no `enabled` field.  Do not turn
//! this RED into a source-presence or UI-only assertion.
#![forbid(unsafe_code)]

use std::sync::Arc;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router_with_auth, AppState, daemon_auth::DaemonAuth};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use tempfile::{tempdir, TempDir};
use tower::ServiceExt;

/// A minimal argv-launched MCP stdio fixture. It implements newline-delimited
/// JSON-RPC and the initialize, initialized notification, then tools/list
/// sequence. The fixture has no network, filesystem, or tool side effect; its
/// one tool only supplies a schema for provider-definition discovery.
fn write_stdio_fixture(dir: &TempDir) -> String {
    let path = dir.path().join("mcp-fixture.sh");
    let script = r###"#!/bin/sh
set -eu

while IFS= read -r body; do
  id=$(printf '%s' "$body" | sed -n 's/.*"id"[[:space:]]*:[[:space:]]*\([0-9][0-9]*\).*/\1/p')
  case "$body" in
    *'"method":"initialize"'*)
      out="{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{\"tools\":{}},\"serverInfo\":{\"name\":\"package004a-fixture\",\"version\":\"1\"}}}"
      ;;
    *'"method":"tools/list"'*)
      out="{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"tools\":[{\"name\":\"fixture_probe\",\"description\":\"bounded fixture probe\",\"inputSchema\":{\"type\":\"object\",\"properties\":{\"value\":{\"type\":\"string\"}},\"required\":[\"value\"]}}]}}"
      ;;
    *) continue;;
  esac
  printf '%s\n' "$out"
done
"###;
    std::fs::write(&path, script).expect("write disposable MCP fixture");
    let mut permissions = std::fs::metadata(&path)
        .expect("fixture metadata")
        .permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o700);
        std::fs::set_permissions(&path, permissions).expect("make fixture executable");
    }
    path.to_str().expect("fixture path is UTF-8").to_owned()
}

fn app_and_token() -> (axum::Router, String, TempDir, String) {
    let storage_dir = tempdir().expect("temporary storage");
    let fixture_dir = tempdir().expect("temporary MCP fixture directory");
    let fixture = write_stdio_fixture(&fixture_dir);
    let storage = Storage::open_in_memory(storage_dir.path().join("blobs")).expect("storage");
    let auth = DaemonAuth::mint().expect("daemon auth");
    let token = auth.token().to_owned();
    let app = router_with_auth(
        AppState {
            sessions: SessionService::new(Arc::new(storage)),
            catalog: Arc::new(Catalog::default()),
        },
        Some(auth),
    );
    // Keep the fixture directory alive through the request sequence by
    // returning it alongside the router and storage directory.
    let _ = storage_dir;
    (app, token, fixture_dir, fixture)
}

async fn json_response(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 128 * 1024)
        .await
        .expect("bounded response body");
    let value = serde_json::from_slice(&bytes).expect("JSON response");
    (status, value)
}

fn add_request(token: &str, name: &str, fixture: &str, enabled: bool) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/api/mcp")
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(Body::from(
            json!({
                "name": name,
                "config": {
                    "type": "local",
                    "command": ["/bin/sh", fixture],
                    "environment": {},
                    "enabled": enabled,
                }
            })
            .to_string(),
        ))
        .expect("MCP add request")
}

#[tokio::test]
async fn mcp_context_lists_configured_flags_and_connects_real_stdio_fixture() {
    let (app, token, _fixture_dir, fixture) = app_and_token();

    // Both entries must be visible in the real authenticated status map.  The
    // disabled entry is intentionally included: listing is configuration
    // visibility, not a connected-only projection.
    let (status, first) = json_response(
        app.clone()
            .oneshot(add_request(&token, "alpha", &fixture, true))
            .await
            .expect("alpha add response"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "enabled config must be accepted: {first}");
    assert_eq!(first["alpha"]["enabled"], true);

    let (status, second) = json_response(
        app.clone()
            .oneshot(add_request(&token, "beta", &fixture, false))
            .await
            .expect("beta add response"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "disabled config must be accepted: {second}");
    assert_eq!(second["alpha"]["enabled"], true);
    assert_eq!(second["beta"]["enabled"], false);
    assert!(second["alpha"].get("status").is_some());
    assert!(second["beta"].get("status").is_some());

    // Connecting is required to be a genuine stdio initialize + tools/list
    // operation, not a UI acknowledgement or synthetic connected status.
    let (status, connected) = json_response(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/mcp/alpha/connect")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .expect("MCP connect request"),
            )
            .await
            .expect("connect response"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "real fixture connection: {connected}");
    assert_eq!(connected["alpha"]["status"], "connected");
    assert_eq!(connected["alpha"]["tools"][0]["name"], "fixture_probe");
    assert_eq!(
        connected["alpha"]["tools"][0]["inputSchema"]["properties"]["value"]["type"],
        "string"
    );

    // The disabled server must remain listed but must not become connected or
    // contribute tools merely because another server was enabled.
    let (status, snapshot) = json_response(
        app.oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/mcp")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("MCP status request"),
        )
        .await
        .expect("status response"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(snapshot["alpha"]["status"], "connected");
    assert_eq!(snapshot["beta"]["enabled"], false);
    assert_ne!(snapshot["beta"]["status"], "connected");
}
