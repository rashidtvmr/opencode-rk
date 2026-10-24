//! Real MCP runtime lifecycle RED-to-GREEN contract.
#![forbid(unsafe_code)]

use std::{process::Command, sync::Arc, thread, time::{Duration, Instant}};

use axum::{body::{to_bytes, Body}, http::{Request, StatusCode}};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{daemon_auth::DaemonAuth, router_with_auth, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use tempfile::{tempdir, TempDir};
use tower::ServiceExt;

fn fixture(dir: &TempDir) -> String {
    let path = dir.path().join("mcp-runtime.sh");
    let source = r###"#!/bin/sh
set -eu
echo "$$" > "${0%/*}/mcp-runtime.pid"
while IFS= read -r body; do
  id=$(printf '%s' "$body" | sed -n 's/.*"id"[[:space:]]*:[[:space:]]*\([0-9][0-9]*\).*/\1/p')
  case "$body" in
    *'"method":"initialize"'*) out="{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{\"tools\":{}},\"serverInfo\":{\"name\":\"runtime-fixture\",\"version\":\"1\"}}}";;
    *'"method":"tools/list"'*) out="{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"tools\":[{\"name\":\"runtime_probe\",\"description\":\"probe\",\"inputSchema\":{\"type\":\"object\"}}]}}";;
    *) continue;;
  esac
  printf '%s\n' "$out"
done
"###;
    std::fs::write(&path, source).expect("fixture");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&path, permissions).expect("permissions");
    }
    path.to_str().expect("utf8 path").to_owned()
}

#[cfg(unix)]
fn wait_for_process_exit(pid: u32) {
    assert!(pid > 1, "fixture child PID is not a safe process ID");
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let result = Command::new("/bin/kill")
            .args(["-0", &pid.to_string()])
            .status()
            .expect("check owned fixture process");
        if !result.success() {
            return;
        }
        assert!(Instant::now() < deadline, "owned fixture child {pid} survived disconnect");
        thread::sleep(Duration::from_millis(20));
    }
}

async fn body(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.expect("bounded body");
    (status, serde_json::from_slice(&bytes).expect("json"))
}

#[tokio::test]
async fn runtime_connects_lists_tools_and_disconnects_owned_child() {
    let fixture_dir = tempdir().expect("fixture dir");
    let fixture = fixture(&fixture_dir);
    let pid_path = fixture_dir.path().join("mcp-runtime.pid");
    let storage_dir = tempdir().expect("storage dir");
    let storage = Storage::open_in_memory(storage_dir.path().join("blobs")).expect("storage");
    let auth = DaemonAuth::mint().expect("auth");
    let token = auth.token().to_owned();
    let app = router_with_auth(
        AppState {
            sessions: SessionService::new(Arc::new(storage)),
            catalog: Arc::new(Catalog::default()),
        },
        Some(auth),
    );
    let add = Request::builder()
        .method("POST")
        .uri("/api/mcp")
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(Body::from(
            json!({"name":"runtime","config":{"type":"local","command":["/bin/sh",fixture],"environment":{},"enabled":true}}).to_string(),
        ))
        .expect("add");
    let (status, value) = body(app.clone().oneshot(add).await.expect("add response")).await;
    assert_eq!(status, StatusCode::OK, "enabled config must register: {value}");
    let connect = Request::builder()
        .method("POST")
        .uri("/api/mcp/runtime/connect")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .expect("connect");
    let (status, value) = body(app.clone().oneshot(connect).await.expect("connect response")).await;
    assert_eq!(status, StatusCode::OK, "real initialize/tools/list: {value}");
    assert_eq!(value["runtime"]["status"], "connected");
    assert_eq!(value["runtime"]["tools"][0]["name"], "runtime_probe");
    let disconnect = Request::builder()
        .method("POST")
        .uri("/api/mcp/runtime/disconnect")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .expect("disconnect");
    let response = app.oneshot(disconnect).await.expect("disconnect response");
    assert_eq!(response.status(), StatusCode::OK);
    #[cfg(unix)]
    {
        let pid: u32 = std::fs::read_to_string(&pid_path)
            .expect("fixture wrote child PID")
            .trim()
            .parse()
            .expect("fixture child PID is numeric");
        wait_for_process_exit(pid);
    }
}
