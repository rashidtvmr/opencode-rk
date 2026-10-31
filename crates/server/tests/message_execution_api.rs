//! The two message-listing routes carry the execution facts a completed turn
//! reported, and nothing when none were stored.

use std::sync::Arc;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use opencode_rk_catalog::Catalog;
use opencode_rk_contracts::MessageRole;
use opencode_rk_server::{router, AppState};
use opencode_rk_sessions::{MessageExecutionInput, SessionService};
use opencode_rk_storage::Storage;
use serde_json::{json, Value};
use tempfile::TempDir;
use tower::ServiceExt;

struct Fixture {
    app: axum::Router,
    session_id: String,
    assistant_id: String,
    _dir: TempDir,
}

async fn fixture(with_facts: bool) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap());
    let sessions = SessionService::new(Arc::clone(&storage));
    let session = sessions.create("execution api").await.unwrap();
    sessions
        .append_text(session.id, MessageRole::User, "probe")
        .await
        .unwrap();
    let execution = with_facts.then(|| MessageExecutionInput {
        provider_id: Some("openai".into()),
        model_id: Some("gpt-5.6".into()),
        stop_reason: Some("completed".into()),
        input_tokens: Some(5),
        output_tokens: Some(9),
        cached_tokens: Some(2),
        reasoning_tokens: Some(1),
    });
    let assistant = sessions
        .append_assistant_with_reasoning_and_execution(session.id, "answer", None, execution)
        .await
        .unwrap();
    // A later message keeps the facts-bearing assistant off the newest page.
    sessions
        .append_assistant_with_reasoning(session.id, "tail", None)
        .await
        .unwrap();
    let app = router(AppState {
        sessions,
        catalog: Arc::new(Catalog::default()),
    });
    Fixture {
        app,
        session_id: session.id.to_string(),
        assistant_id: assistant.id.to_string(),
        _dir: dir,
    }
}

async fn get(app: &axum::Router, uri: &str) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "{uri}");
    let bytes = to_bytes(response.into_body(), 128 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn find<'a>(messages: &'a [Value], id: &str) -> &'a Value {
    messages
        .iter()
        .find(|message| message["id"] == json!(id))
        .unwrap_or_else(|| panic!("message {id} missing"))
}

#[tokio::test]
async fn history_and_messages_carry_the_stored_execution_facts() {
    let fixture = fixture(true).await;
    for uri in [
        format!("/api/sessions/{}/history?limit=100", fixture.session_id),
        format!("/api/sessions/{}/messages?limit=100", fixture.session_id),
    ] {
        let body = get(&fixture.app, &uri).await;
        let messages = body["messages"].as_array().unwrap();
        let assistant = find(messages, &fixture.assistant_id);
        assert_eq!(
            assistant["execution"],
            json!({
                "provider_id": "openai",
                "model_id": "gpt-5.6",
                "stop_reason": "completed",
                "usage": {
                    "input_tokens": 5,
                    "output_tokens": 9,
                    "cached_tokens": 2,
                    "reasoning_tokens": 1,
                },
            }),
            "{uri}"
        );
        // The user message of the same page has no stored facts, so the key is
        // absent rather than null or zeroed.
        let user = messages
            .iter()
            .find(|message| message["role"] == json!("user"))
            .unwrap();
        assert!(user.get("execution").is_none(), "{uri}: {user}");
    }
}

#[tokio::test]
async fn messages_without_stored_facts_carry_no_execution_key() {
    let fixture = fixture(false).await;
    for uri in [
        format!("/api/sessions/{}/history?limit=100", fixture.session_id),
        format!("/api/sessions/{}/messages?limit=100", fixture.session_id),
    ] {
        let body = get(&fixture.app, &uri).await;
        let messages = body["messages"].as_array().unwrap();
        let assistant = find(messages, &fixture.assistant_id);
        assert!(assistant.get("execution").is_none(), "{uri}: {assistant}");
    }
}

#[tokio::test]
async fn older_history_pages_keep_their_own_execution_rows() {
    let fixture = fixture(true).await;
    // Two pages of one message each: the assistant is on the older page, so an
    // ascending-limit lookup would miss it.
    let first = get(
        &fixture.app,
        &format!("/api/sessions/{}/history?limit=1", fixture.session_id),
    )
    .await;
    let next = first["next_before"].as_str().expect("older page cursor");
    let older = get(
        &fixture.app,
        &format!(
            "/api/sessions/{}/history?limit=1&before={next}",
            fixture.session_id
        ),
    )
    .await;
    let messages = older["messages"].as_array().unwrap();
    let assistant = find(messages, &fixture.assistant_id);
    assert_eq!(assistant["execution"]["model_id"], json!("gpt-5.6"));
}
