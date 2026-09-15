use std::sync::Arc;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_contracts::MessageRole;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::{SessionManager, SessionService};
use opencode_rk_storage::Storage;
use serde_json::Value;
use tempfile::tempdir;
use tower::ServiceExt;

async fn fixture() -> (axum::Router, SessionService, tempfile::TempDir) {
    let dir = tempdir().expect("temporary branch fixture");
    let storage = Storage::open_in_memory(dir.path().join("blobs")).expect("storage fixture");
    let branch_manager = SessionManager::open_branch_workspace(&dir.path().join("branches-v2.db"))
        .expect("format-2 branch workspace");
    let sessions = SessionService::with_branch_manager(Arc::new(storage), Arc::new(branch_manager));
    (
        router(AppState {
            sessions: sessions.clone(),
            catalog: Arc::new(Catalog::default()),
        }),
        sessions,
        dir,
    )
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 128 * 1024)
        .await
        .expect("bounded json response");
    serde_json::from_slice(&bytes).expect("json response")
}

#[tokio::test]
async fn web_008_t01_branch_from_user_or_assistant_is_inclusive_and_source_is_unchanged() {
    let (app, sessions, _dir) = fixture().await;
    let parent = sessions.create("Parent chat").await.unwrap();
    let user = sessions
        .append_text(parent.id, MessageRole::User, "first request")
        .await
        .unwrap();
    let assistant = sessions
        .append_text(parent.id, MessageRole::Assistant, "first answer")
        .await
        .unwrap();
    let later = sessions
        .append_text(parent.id, MessageRole::User, "later request")
        .await
        .unwrap();

    for (boundary_id, expected_len) in [(user.id, 1usize), (assistant.id, 2usize)] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/api/sessions/{}/messages/{boundary_id}/branch",
                        parent.id
                    ))
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = json_body(response).await;
        assert_eq!(status, StatusCode::CREATED, "branch response: {body}");
        let child_id = body["session"]["id"].as_str().expect("child session id");
        assert_ne!(child_id, parent.id.to_string());
        assert_eq!(body["session"]["title"], "Branch: Parent chat");
        assert_eq!(body["fork"]["parent_session_id"], parent.id.to_string());
        assert_eq!(body["fork"]["fork_message_seq"], expected_len as u64);
        assert_eq!(body["fork"]["boundary_message_id"], boundary_id.to_string());

        let child = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/sessions/{child_id}/messages?limit=50"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(child.status(), StatusCode::OK);
        let child = json_body(child).await;
        let messages = child["messages"].as_array().unwrap();
        assert_eq!(messages.len(), expected_len);

        let lineage = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/sessions/{child_id}/fork"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(lineage.status(), StatusCode::OK);
        let lineage = json_body(lineage).await;
        assert_eq!(lineage["fork"]["parent_session_id"], parent.id.to_string());
        assert_eq!(lineage["fork"]["fork_message_seq"], expected_len as u64);
        assert_eq!(lineage["fork"]["boundary_message_id"], boundary_id.to_string());
        assert_eq!(messages[0]["body"]["text"], "first request");
        assert_ne!(messages[0]["id"], user.id.to_string());
        if expected_len == 2 {
            assert_eq!(messages[1]["body"]["text"], "first answer");
            assert_ne!(messages[1]["id"], assistant.id.to_string());
        }
    }

    let source = sessions.messages(parent.id, 50).await.unwrap();
    assert_eq!(source.len(), 3);
    assert_eq!(source[0].id, user.id);
    assert_eq!(source[1].id, assistant.id);
    assert_eq!(source[2].id, later.id);
}

#[tokio::test]
async fn web_008_t01_earlier_boundary_remains_valid_after_a_later_shadow_sync() {
    let (app, sessions, _dir) = fixture().await;
    let parent = sessions.create("Parent chat").await.unwrap();
    let first_user = sessions
        .append_text(parent.id, MessageRole::User, "first request")
        .await
        .unwrap();
    let first_assistant = sessions
        .append_text(parent.id, MessageRole::Assistant, "first answer")
        .await
        .unwrap();
    let later_user = sessions
        .append_text(parent.id, MessageRole::User, "later request")
        .await
        .unwrap();
    let later_assistant = sessions
        .append_text(parent.id, MessageRole::Assistant, "later answer")
        .await
        .unwrap();

    let later = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/sessions/{}/messages/{}/branch",
                    parent.id, later_assistant.id
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(later.status(), StatusCode::CREATED);

    let earlier = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/sessions/{}/messages/{}/branch",
                    parent.id, first_assistant.id
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(earlier.status(), StatusCode::CREATED);
    let earlier = json_body(earlier).await;
    assert_eq!(earlier["fork"]["fork_message_seq"], 2);
    let earlier_id = earlier["session"]["id"].as_str().unwrap();
    let messages = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/sessions/{earlier_id}/messages?limit=50"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let messages = json_body(messages).await;
    let messages = messages["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["body"]["text"], "first request");
    assert_eq!(messages[1]["body"]["text"], "first answer");

    let source = sessions.messages(parent.id, 50).await.unwrap();
    assert_eq!(source.len(), 4);
    assert_eq!(source[0].id, first_user.id);
    assert_eq!(source[1].id, first_assistant.id);
    assert_eq!(source[2].id, later_user.id);
    assert_eq!(source[3].id, later_assistant.id);
}

#[tokio::test]
async fn web_008_t02_wrong_session_boundary_is_not_found_and_creates_no_partial_child() {
    let (app, sessions, _dir) = fixture().await;
    let parent = sessions.create("Parent").await.unwrap();
    let other = sessions.create("Other").await.unwrap();
    let foreign = sessions
        .append_text(other.id, MessageRole::User, "belongs elsewhere")
        .await
        .unwrap();
    let before = sessions.list(false).await.unwrap().len();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/sessions/{}/messages/{}/branch",
                    parent.id, foreign.id
                ))
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(sessions.list(false).await.unwrap().len(), before);
}

#[tokio::test]
async fn web_008_t02_retry_branch_starts_before_the_user_request() {
    let (app, sessions, _dir) = fixture().await;
    let parent = sessions.create("Retry parent").await.unwrap();
    let first_user = sessions
        .append_text(parent.id, MessageRole::User, "first request")
        .await
        .unwrap();
    let first_assistant = sessions
        .append_text(parent.id, MessageRole::Assistant, "first answer")
        .await
        .unwrap();
    let later_user = sessions
        .append_text(parent.id, MessageRole::User, "later request")
        .await
        .unwrap();

    let assistant_retry = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/sessions/{}/messages/{}/retry-branch",
                    parent.id, first_assistant.id
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(assistant_retry.status(), StatusCode::CREATED);
    let assistant_retry = json_body(assistant_retry).await;
    assert_eq!(assistant_retry["request_text"], "first request");
    assert_eq!(assistant_retry["fork"]["fork_message_seq"], 0);
    assert!(assistant_retry["fork"]["boundary_message_id"].is_null());
    let child_id = assistant_retry["session"]["id"].as_str().unwrap();
    let child = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/sessions/{child_id}/messages?limit=50"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(json_body(child).await["messages"].as_array().unwrap().len(), 0);

    let user_retry = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/sessions/{}/messages/{}/retry-branch",
                    parent.id, later_user.id
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(user_retry.status(), StatusCode::CREATED);
    let user_retry = json_body(user_retry).await;
    assert_eq!(user_retry["request_text"], "later request");
    assert_eq!(user_retry["fork"]["fork_message_seq"], 2);

    let source = sessions.messages(parent.id, 50).await.unwrap();
    assert_eq!(source.len(), 3);
    assert_eq!(source[0].id, first_user.id);
    assert_eq!(source[1].id, first_assistant.id);
    assert_eq!(source[2].id, later_user.id);
}
