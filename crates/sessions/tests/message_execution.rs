use std::sync::Arc;

use opencode_rk_contracts::MessageRole;
use opencode_rk_sessions::{MessageExecutionInput, SessionService};
use opencode_rk_storage::Storage;

fn service() -> (SessionService, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap());
    (SessionService::new(storage), dir)
}

fn facts() -> MessageExecutionInput {
    MessageExecutionInput {
        provider_id: Some("openai".into()),
        model_id: Some("gpt-5.6".into()),
        stop_reason: Some("completed".into()),
        input_tokens: Some(11),
        output_tokens: Some(7),
        cached_tokens: Some(0),
        reasoning_tokens: None,
        streamed_ms: Some(1_500),
    }
}

#[tokio::test]
async fn execution_facts_round_trip_with_the_message_they_belong_to() {
    let (sessions, _dir) = service();
    let session = sessions.create("execution facts").await.unwrap();
    let message = sessions
        .append_assistant_with_reasoning_and_execution(
            session.id,
            "answer",
            Some("summary".into()),
            Some(facts()),
        )
        .await
        .unwrap();
    let stored = sessions
        .message_execution(vec![message.id])
        .await
        .unwrap();
    assert_eq!(stored.len(), 1);
    let stored = &stored[0];
    assert_eq!(stored.message_id, message.id);
    assert_eq!(stored.provider_id.as_deref(), Some("openai"));
    assert_eq!(stored.model_id.as_deref(), Some("gpt-5.6"));
    assert_eq!(stored.stop_reason.as_deref(), Some("completed"));
    assert_eq!(stored.input_tokens, Some(11));
    assert_eq!(stored.output_tokens, Some(7));
    // A reported zero is a fact and must survive as zero.
    assert_eq!(stored.cached_tokens, Some(0));
    // An unreported counter stays absent instead of becoming zero.
    assert_eq!(stored.reasoning_tokens, None);
    // MSG-008: the provider-active streamed span survives the round trip.
    assert_eq!(stored.streamed_ms, Some(1_500));
}

#[tokio::test]
async fn a_message_without_reported_facts_stores_none() {
    let (sessions, _dir) = service();
    let session = sessions.create("plain").await.unwrap();
    let message = sessions
        .append_assistant_with_reasoning(session.id, "plain answer", None)
        .await
        .unwrap();
    assert!(sessions
        .message_execution(vec![message.id])
        .await
        .unwrap()
        .is_empty());
    let user = sessions
        .append_text(session.id, MessageRole::User, "hello")
        .await
        .unwrap();
    assert!(sessions
        .message_execution(vec![user.id])
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn id_scoped_lookup_returns_only_the_requested_messages() {
    let (sessions, _dir) = service();
    let session = sessions.create("scoped").await.unwrap();
    let first = sessions
        .append_assistant_with_reasoning_and_execution(
            session.id,
            "first",
            None,
            Some(facts()),
        )
        .await
        .unwrap();
    let second = sessions
        .append_assistant_with_reasoning_and_execution(
            session.id,
            "second",
            None,
            Some(MessageExecutionInput {
                stop_reason: Some("max_steps:3".into()),
                ..MessageExecutionInput::default()
            }),
        )
        .await
        .unwrap();
    let only_second = sessions
        .message_execution(vec![second.id])
        .await
        .unwrap();
    assert_eq!(only_second.len(), 1);
    assert_eq!(only_second[0].stop_reason.as_deref(), Some("max_steps:3"));
    assert_eq!(only_second[0].provider_id, None);
    let both = sessions
        .message_execution(vec![first.id, second.id])
        .await
        .unwrap();
    assert_eq!(both.len(), 2);
}
