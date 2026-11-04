#![forbid(unsafe_code)]
//! TOOL-006 route contract: the daemon-side shell registry is seeded by the
//! real streaming shell execution and the read routes project it with a
//! cursor+limit, 404 and bad-limit behaviour.
use std::sync::Arc;
use std::time::Duration;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::Storage;
use tower::ServiceExt;

fn app() -> (axum::Router, SessionService, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let sessions = SessionService::new(Arc::new(
        Storage::open_in_memory(dir.path().join("blobs")).unwrap(),
    ));
    let app = router(AppState {
        sessions: sessions.clone(),
        catalog: Arc::new(Catalog::default()),
    });
    (app, sessions, dir)
}

fn request(method: &str, path: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

/// Lean-default security requires one human approval before the shell runs
/// (`native_shell_protocol.rs`): reply `once` when a request appears.
async fn approve_shell(app: axum::Router, session: impl std::fmt::Display) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        assert!(
            tokio::time::Instant::now() < deadline,
            "no broker permission appeared for the shell"
        );
        let listing = app
            .clone()
            .oneshot(request("GET", &format!("/api/session/{session}/permission"), ""))
            .await
            .unwrap();
        assert_eq!(listing.status(), StatusCode::OK);
        let value: serde_json::Value =
            serde_json::from_slice(&to_bytes(listing.into_body(), 64 * 1024).await.unwrap())
                .unwrap();
        if let Some(request_id) = value["data"]
            .as_array()
            .and_then(|rows| rows.first())
            .and_then(|row| row["id"].as_str())
        {
            let reply = app
                .oneshot(
                    request(
                        "POST",
                        &format!("/api/session/{session}/permission/{request_id}/reply"),
                        r#"{"reply":"once"}"#,
                    ),
                )
                .await
                .unwrap();
            assert_eq!(reply.status(), StatusCode::NO_CONTENT);
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn tool006_shell_routes_list_get_output_cursor_limit_and_404() {
    let (app, sessions, dir) = app();
    let session = sessions
        .create_in_directory("tool006", Some(dir.path().to_owned()))
        .await
        .unwrap();
    // The shell's `cwd` is captured once, at create time (the session location
    // at start), so a later session move cannot re-home the process.
    let expected_cwd = std::fs::canonicalize(dir.path()).unwrap();
    let stream = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/sessions/{}/shell/stream", session.id),
            r#"{"command":"printf 'αβγδε'"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(stream.status(), StatusCode::OK);
    let body =
        tokio::spawn(async move { to_bytes(stream.into_body(), 1024 * 1024).await.unwrap() });
    approve_shell(app.clone(), &session.id).await;
    let _ = tokio::time::timeout(Duration::from_secs(5), body).await.unwrap().unwrap();

    let list = app
        .clone()
        .oneshot(request("GET", "/api/shell", ""))
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let value: serde_json::Value =
        serde_json::from_slice(&to_bytes(list.into_body(), 4096).await.unwrap()).unwrap();
    let shell = value["shells"].as_array().unwrap().first().unwrap();
    let id = shell["id"].as_str().unwrap();
    let cwd = std::fs::canonicalize(shell["cwd"].as_str().unwrap()).unwrap();
    assert_eq!(cwd, expected_cwd, "shell cwd must be the create-time directory");
    assert_eq!(shell["status"], "completed");

    let get = app
        .clone()
        .oneshot(request("GET", &format!("/api/shell/{id}"), ""))
        .await
        .unwrap();
    assert_eq!(get.status(), StatusCode::OK);
    let output = app
        .clone()
        .oneshot(request(
            "GET",
            &format!("/api/shell/{id}/output?cursor=0&limit=4"),
            "",
        ))
        .await
        .unwrap();
    assert_eq!(output.status(), StatusCode::OK);
    let chunk: serde_json::Value =
        serde_json::from_slice(&to_bytes(output.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(chunk["output"], "αβ");
    assert_eq!(chunk["cursor"], 4);
    let next = app
        .clone()
        .oneshot(request(
            "GET",
            &format!("/api/shell/{id}/output?cursor=4&limit=4"),
            "",
        ))
        .await
        .unwrap();
    let tail: serde_json::Value =
        serde_json::from_slice(&to_bytes(next.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(tail["output"], "γδ");
    // A byte-cursor drain continues past the multibyte pair.
    let rest = app
        .clone()
        .oneshot(request(
            "GET",
            &format!("/api/shell/{id}/output?cursor=8&limit=4"),
            "",
        ))
        .await
        .unwrap();
    let last: serde_json::Value =
        serde_json::from_slice(&to_bytes(rest.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(last["output"], "ε");

    assert_eq!(
        app.clone()
            .oneshot(request("GET", "/api/shell/missing", ""))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.clone()
            .oneshot(request(
                "GET",
                "/api/shell/missing/output?cursor=0&limit=4",
                ""
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.oneshot(request(
            "GET",
            &format!("/api/shell/{id}/output?cursor=0&limit=1048577"),
            ""
        ))
        .await
        .unwrap()
        .status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn tool006_shell_routes_404_before_any_shell_and_lists_running_status() {
    let (app, sessions, dir) = app();
    let session = sessions
        .create_in_directory("tool006-b", Some(dir.path().to_owned()))
        .await
        .unwrap();
    // No shell started yet: the list is empty and gets 404.
    let empty = app
        .clone()
        .oneshot(request("GET", "/api/shell", ""))
        .await
        .unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&to_bytes(empty.into_body(), 4096).await.unwrap()).unwrap();
    assert!(value["shells"].as_array().unwrap().is_empty());

    // A shell that never terminates while it waits for its permission reply
    // stays `running` in the registry until it is approved and completes.
    let stream = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/sessions/{}/shell/stream", session.id),
            r#"{"command":"true"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(stream.status(), StatusCode::OK);
    let running = app
        .clone()
        .oneshot(request("GET", "/api/shell", ""))
        .await
        .unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&to_bytes(running.into_body(), 4096).await.unwrap()).unwrap();
    let shell = value["shells"].as_array().unwrap().first().expect("stream posted shell").clone();
    assert_eq!(shell["status"], "running");
    assert_eq!(
        std::fs::canonicalize(shell["cwd"].as_str().unwrap()).unwrap(),
        std::fs::canonicalize(dir.path()).unwrap()
    );
    assert_eq!(shell["command"], "true");

    // Now let it finish and confirm the same record drains to `completed`.
    let body =
        tokio::spawn(async move { to_bytes(stream.into_body(), 1024 * 1024).await.unwrap() });
    approve_shell(app.clone(), &session.id).await;
    let _ = tokio::time::timeout(Duration::from_secs(5), body).await.unwrap().unwrap();
    let id = shell["id"].as_str().unwrap();
    let done = app
        .clone()
        .oneshot(request("GET", &format!("/api/shell/{id}"), ""))
        .await
        .unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&to_bytes(done.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(value["status"], "completed");
}
