//! Actual router/session/broker/child execution contract; no provider stub required.
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_server::{AppState, router};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::{Storage, StoragePaths};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tower::ServiceExt;

async fn request(
    app: axum::Router,
    method: &str,
    uri: &str,
    body: Value,
) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method(method)
            .uri(uri)
            .header("Content-Type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
}
async fn value(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 256 * 1024).await.unwrap()).unwrap()
}
fn app(storage: Arc<Storage>) -> axum::Router {
    router(AppState {
        sessions: SessionService::new(storage),
        catalog: Arc::new(Catalog::default()),
    })
}
async fn pending(app: axum::Router, id: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let body = value(
                request(
                    app.clone(),
                    "GET",
                    &format!("/api/session/{id}/permission"),
                    json!({}),
                )
                .await,
            )
            .await;
            if let Some(request) = body["data"].as_array().and_then(|rows| rows.first()) {
                return request.clone();
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("real shell must install a pending broker permission")
}

#[tokio::test]
async fn session_directory_survives_storage_reopen() {
    let root = tempfile::tempdir().unwrap();
    let expected_root = std::fs::canonicalize(root.path()).unwrap();
    let paths = StoragePaths::under(root.path().join("data"));
    let first = app(Arc::new(Storage::open(paths.clone()).unwrap()));
    let response = request(
        first.clone(),
        "POST",
        "/api/sessions",
        json!({"title":"directory", "directory":root.path()}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let created = value(response).await;
    let id = created["session"]["id"].as_str().unwrap();
    assert_eq!(
        created["session"]["directory"],
        json!(expected_root),
        "session create dropped caller cwd"
    );
    drop(first);
    let second = app(Arc::new(Storage::open(paths).unwrap()));
    let restored =
        value(request(second, "GET", &format!("/api/sessions/{id}"), json!({})).await).await;
    assert_eq!(
        restored["session"]["directory"],
        json!(expected_root),
        "session cwd was not durable"
    );
}

#[tokio::test]
async fn native_shell_waits_for_approval_executes_once_and_hydrates_typed_history() {
    let root = tempfile::tempdir().unwrap();
    let app = app(Arc::new(
        Storage::open_in_memory(root.path().join("blobs")).unwrap(),
    ));
    let created = value(
        request(
            app.clone(),
            "POST",
            "/api/sessions",
            json!({"title":"shell", "directory":root.path()}),
        )
        .await,
    )
    .await;
    let id = created["session"]["id"].as_str().unwrap().to_owned();
    let marker = root.path().join("marker");
    let command = format!("/bin/pwd; printf executed > '{}'", marker.display());
    let response = request(
        app.clone(),
        "POST",
        &format!("/api/sessions/{id}/shell/stream"),
        json!({"command":command}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body =
        tokio::spawn(async move { to_bytes(response.into_body(), 256 * 1024).await.unwrap() });
    let permission = pending(app.clone(), &id).await;
    assert!(!marker.exists(), "shell executed before human approval");
    let request_id = permission["id"].as_str().unwrap();
    let reply = request(
        app.clone(),
        "POST",
        &format!("/api/session/{id}/permission/{request_id}/reply"),
        json!({"reply":"once"}),
    )
    .await;
    assert_eq!(reply.status(), StatusCode::NO_CONTENT);
    let bytes = tokio::time::timeout(Duration::from_secs(5), body)
        .await
        .unwrap()
        .unwrap();
    let events: Vec<Value> = String::from_utf8(bytes.to_vec())
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(events.first().unwrap()["type"], "user_message");
    assert!(events.iter().any(|event| event["type"] == "tool_call"));
    let output = events
        .iter()
        .find(|event| event["type"] == "tool_output")
        .expect("typed shell output");
    assert!(
        output["output"]
            .as_str()
            .unwrap()
            .contains(root.path().to_str().unwrap()),
        "pwd did not use durable session cwd"
    );
    assert_eq!(events.last().unwrap()["type"], "assistant_message");
    assert_eq!(std::fs::read_to_string(&marker).unwrap(), "executed");
    let duplicate = request(
        app.clone(),
        "POST",
        &format!("/api/session/{id}/permission/{request_id}/reply"),
        json!({"reply":"once"}),
    )
    .await;
    assert!(
        duplicate.status().is_client_error(),
        "approval was replayable"
    );
    let history = value(
        request(
            app,
            "GET",
            &format!("/api/sessions/{id}/messages"),
            json!({}),
        )
        .await,
    )
    .await;
    let parts: Vec<&Value> = history["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|row| row["parts"].as_array())
        .flatten()
        .collect();
    let call = parts
        .iter()
        .find(|part| part["type"] == "tool_call")
        .expect("call must hydrate");
    let result = parts
        .iter()
        .find(|part| part["type"] == "tool_output")
        .expect("result must hydrate");
    assert_eq!(call["call_id"], result["call_id"]);
    assert_eq!(call["arguments"]["command"], json!(command));
}

#[tokio::test]
async fn native_shell_rejection_is_side_effect_free() {
    let root = tempfile::tempdir().unwrap();
    let app = app(Arc::new(
        Storage::open_in_memory(root.path().join("blobs")).unwrap(),
    ));
    let created = value(
        request(
            app.clone(),
            "POST",
            "/api/sessions",
            json!({"title":"denied", "directory":root.path()}),
        )
        .await,
    )
    .await;
    let id = created["session"]["id"].as_str().unwrap();
    let marker = root.path().join("forbidden");
    let response = request(
        app.clone(),
        "POST",
        &format!("/api/sessions/{id}/shell/stream"),
        json!({"command":format!("printf forbidden > '{}'",marker.display())}),
    )
    .await;
    let body =
        tokio::spawn(async move { to_bytes(response.into_body(), 256 * 1024).await.unwrap() });
    let permission = pending(app.clone(), id).await;
    let reply = request(
        app,
        "POST",
        &format!(
            "/api/session/{id}/permission/{}/reply",
            permission["id"].as_str().unwrap()
        ),
        json!({"reply":"reject"}),
    )
    .await;
    assert_eq!(reply.status(), StatusCode::NO_CONTENT);
    let bytes = tokio::time::timeout(Duration::from_secs(5), body)
        .await
        .unwrap()
        .unwrap();
    assert!(!marker.exists(), "denied shell created a file");
    assert!(
        String::from_utf8(bytes.to_vec())
            .unwrap()
            .contains("rejected")
    );
}

#[tokio::test]
async fn disconnected_shell_client_releases_pending_owner_without_execution() {
    let root = tempfile::tempdir().unwrap();
    let app = app(Arc::new(
        Storage::open_in_memory(root.path().join("blobs")).unwrap(),
    ));
    let created = value(
        request(
            app.clone(),
            "POST",
            "/api/sessions",
            json!({"title":"disconnect", "directory":root.path()}),
        )
        .await,
    )
    .await;
    let id = created["session"]["id"].as_str().unwrap();
    let marker = root.path().join("forbidden");
    let response = request(
        app.clone(),
        "POST",
        &format!("/api/sessions/{id}/shell/stream"),
        json!({"command":format!("printf forbidden > '{}'",marker.display())}),
    )
    .await;
    let _pending = pending(app.clone(), id).await;
    drop(response);
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let value = value(
                request(
                    app.clone(),
                    "GET",
                    &format!("/api/session/{id}/permission"),
                    json!({}),
                )
                .await,
            )
            .await;
            if value["data"]
                .as_array()
                .is_some_and(|items| items.is_empty())
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("disconnected native shell retained pending permission owner");
    assert!(!marker.exists());
}

#[tokio::test]
async fn shell_large_output_is_durably_bounded_without_losing_terminal_ack() {
    let root = tempfile::tempdir().unwrap();
    let app = app(Arc::new(
        Storage::open_in_memory(root.path().join("blobs")).unwrap(),
    ));
    let created = value(
        request(
            app.clone(),
            "POST",
            "/api/sessions",
            json!({"title":"bounded", "directory":root.path()}),
        )
        .await,
    )
    .await;
    let id = created["session"]["id"].as_str().unwrap();
    let response = request(
        app.clone(),
        "POST",
        &format!("/api/sessions/{id}/shell/stream"),
        json!({"command":"printf '\\000%.0s' {1..20000}"}),
    )
    .await;
    let body =
        tokio::spawn(async move { to_bytes(response.into_body(), 256 * 1024).await.unwrap() });
    let permission = pending(app.clone(), id).await;
    let _reply = request(
        app.clone(),
        "POST",
        &format!(
            "/api/session/{id}/permission/{}/reply",
            permission["id"].as_str().unwrap()
        ),
        json!({"reply":"once"}),
    )
    .await;
    let bytes = tokio::time::timeout(Duration::from_secs(5), body)
        .await
        .unwrap()
        .unwrap();
    let events: Vec<Value> = String::from_utf8(bytes.to_vec())
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(
        events.last().unwrap()["type"],
        "assistant_message",
        "large output lost durable terminal ACK"
    );
    let history = value(
        request(
            app,
            "GET",
            &format!("/api/sessions/{id}/messages"),
            json!({}),
        )
        .await,
    )
    .await;
    let parts: Vec<_> = history["messages"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|message| message["parts"].as_array().into_iter().flatten())
        .collect();
    let result = parts
        .iter()
        .find(|part| part["type"] == "tool_output")
        .unwrap();
    assert_eq!(result["success"], true);
    assert!(result["output"].as_str().unwrap().contains("truncated"));
}

#[tokio::test]
async fn bounded_message_page_hydrates_its_own_tool_records() {
    let root = tempfile::tempdir().unwrap();
    let app = app(Arc::new(
        Storage::open_in_memory(root.path().join("blobs")).unwrap(),
    ));
    let created = value(
        request(
            app.clone(),
            "POST",
            "/api/sessions",
            json!({"title":"pages","directory":root.path()}),
        )
        .await,
    )
    .await;
    let id = created["session"]["id"].as_str().unwrap();
    for index in 0..5 {
        let response = request(
            app.clone(),
            "POST",
            &format!("/api/sessions/{id}/shell/stream"),
            json!({"command":format!("printf output-{index}")}),
        )
        .await;
        let body =
            tokio::spawn(async move { to_bytes(response.into_body(), 256 * 1024).await.unwrap() });
        let permission = pending(app.clone(), id).await;
        let reply = request(
            app.clone(),
            "POST",
            &format!(
                "/api/session/{id}/permission/{}/reply",
                permission["id"].as_str().unwrap()
            ),
            json!({"reply":"once"}),
        )
        .await;
        assert_eq!(reply.status(), StatusCode::NO_CONTENT);
        body.await.unwrap();
    }
    let history = value(
        request(
            app,
            "GET",
            &format!("/api/sessions/{id}/messages?limit=3"),
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(history["messages"].as_array().unwrap().len(), 3);
    let parts: Vec<_> = history["messages"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|message| message["parts"].as_array().into_iter().flatten())
        .collect();
    assert!(
        parts
            .iter()
            .any(|part| part["type"] == "tool_output" && part["output"] == "output-0"),
        "newer tool records displaced selected page hydration"
    );
}
