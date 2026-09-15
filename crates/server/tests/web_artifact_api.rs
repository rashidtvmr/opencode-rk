use std::sync::Arc;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_contracts::{MessageRole, PayloadRef};
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::SessionService;
use opencode_rk_storage::{Storage, StoragePaths};
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

const EXPECTED_MAX_ARTIFACT_BYTES: usize = 64 * 1024;

async fn body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 512 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn router_for(sessions: SessionService) -> axum::Router {
    router(AppState {
        sessions,
        catalog: Arc::new(Catalog::default()),
    })
}

#[tokio::test]
async fn web_017_t01_artifact_versions_survive_reload_without_mutating_source_message() {
    let dir = tempdir().unwrap();
    let paths = StoragePaths::under(dir.path().join("data"));
    let storage = Storage::open(paths.clone()).unwrap();
    let sessions = SessionService::new(Arc::new(storage));
    let session = sessions.create("Artifact chat").await.unwrap();
    let source = sessions
        .append_text(session.id, MessageRole::Assistant, "original assistant output")
        .await
        .unwrap();
    let app = router_for(sessions.clone());

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/sessions/{}/messages/{}/artifacts",
                    session.id, source.id
                ))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "kind":"writing",
                        "title":"Draft",
                        "language":null,
                        "content":"first draft"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let created = body(create).await;
    let artifact_id = created["artifact"]["id"].as_str().unwrap().to_owned();
    assert_eq!(created["artifact"]["source_message_id"], source.id.to_string());
    assert_eq!(created["artifact"]["current_version"], 1);
    assert_eq!(created["artifact"]["content"], "first draft");
    assert_eq!(created["run_available"], false);
    assert_eq!(created["apply_available"], false);

    let save = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/sessions/{}/artifacts/{}/versions",
                    session.id, artifact_id
                ))
                .header("content-type", "application/json")
                .body(Body::from(json!({"content":"second draft"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(save.status(), StatusCode::CREATED);
    let saved = body(save).await;
    assert_eq!(saved["artifact"]["current_version"], 2);
    assert_eq!(saved["artifact"]["content"], "second draft");

    let messages = sessions.messages(session.id, 10).await.unwrap();
    let unchanged = messages.iter().find(|message| message.id == source.id).unwrap();
    assert_eq!(
        unchanged.body,
        PayloadRef::Inline {
            text: "original assistant output".to_owned()
        }
    );

    drop(app);
    drop(sessions);
    let reopened = SessionService::new(Arc::new(Storage::open(paths).unwrap()));
    let reloaded = router_for(reopened)
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/sessions/{}/artifacts/{}",
                    session.id, artifact_id
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reloaded.status(), StatusCode::OK);
    let reloaded = body(reloaded).await;
    assert_eq!(reloaded["artifact"]["current_version"], 2);
    assert_eq!(reloaded["artifact"]["content"], "second draft");
    assert_eq!(reloaded["artifact"]["versions"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn web_017_t02_artifact_source_and_content_bounds_fail_closed() {
    let dir = tempdir().unwrap();
    let storage = Storage::open_in_memory(dir.path().join("blobs")).unwrap();
    let sessions = SessionService::new(Arc::new(storage));
    let session = sessions.create("Artifact bounds").await.unwrap();
    let user = sessions
        .append_text(session.id, MessageRole::User, "user source")
        .await
        .unwrap();
    let assistant = sessions
        .append_text(session.id, MessageRole::Assistant, "assistant source")
        .await
        .unwrap();
    let app = router_for(sessions);

    let invalid_source = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/sessions/{}/messages/{}/artifacts",
                    session.id, user.id
                ))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"kind":"code","title":"Nope","language":"text","content":"x"})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_source.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let oversized = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/sessions/{}/messages/{}/artifacts",
                    session.id, assistant.id
                ))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "kind":"writing",
                        "title":"Too large",
                        "language":null,
                        "content":"x".repeat(EXPECTED_MAX_ARTIFACT_BYTES + 1)
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(oversized.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
