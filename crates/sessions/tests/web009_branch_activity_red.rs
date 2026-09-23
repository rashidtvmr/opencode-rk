use std::sync::Arc;

use opencode_rk_contracts::{
    AssistantReference, AssistantToolCall, MessageRole, PayloadRef, MAX_REASONING_SUMMARY_BYTES,
};
use opencode_rk_sessions::{SessionManager, SessionService};
use opencode_rk_storage::Storage;

fn activity_fixture() -> (Vec<AssistantToolCall>, Vec<AssistantReference>) {
    (
        vec![AssistantToolCall {
            call_id: "call-search-1".to_owned(),
            name: "search".to_owned(),
            state: "completed".to_owned(),
            ok: true,
        }],
        vec![AssistantReference {
            label: "Rust reference".to_owned(),
            url: "https://doc.rust-lang.org/book/".to_owned(),
        }],
    )
}

fn inline_body(message: &opencode_rk_contracts::MessageRecord) -> &str {
    match &message.body {
        PayloadRef::Inline { text } => text,
        PayloadRef::Blob { .. } => panic!("test messages must use inline bodies"),
    }
}

async fn service(
    storage: Arc<Storage>,
    branch_path: &std::path::Path,
) -> (SessionService, Arc<SessionManager>) {
    let manager = Arc::new(SessionManager::open_branch_workspace(branch_path).unwrap());
    (
        SessionService::with_branch_manager(storage, Arc::clone(&manager)),
        manager,
    )
}

#[tokio::test]
async fn legacy_assistant_activity_is_copied_to_branch_without_reexecution() {
    let dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(Storage::open_in_memory(dir.path().join("legacy-blobs")).unwrap());
    let branch_path = dir.path().join("branch.db");
    let (sessions, _manager) = service(Arc::clone(&storage), &branch_path).await;
    let (tool_calls, references) = activity_fixture();

    let parent = sessions.create("Parent").await.unwrap();
    sessions
        .append_text(parent.id, MessageRole::User, "request")
        .await
        .unwrap();
    let parent_assistant = sessions
        .append_assistant_with_activity(
            parent.id,
            "answer",
            Some("Checked the relevant constraints.".to_owned()),
            tool_calls.clone(),
            references.clone(),
        )
        .await
        .unwrap();

    let parent_activity = sessions.assistant_activity(parent.id, 50).await.unwrap();
    assert_eq!(parent_activity.len(), 1);
    assert_eq!(parent_activity[0].message_id, parent_assistant.id);
    assert_eq!(
        parent_activity[0].reasoning_summary,
        "Checked the relevant constraints."
    );
    assert_eq!(parent_activity[0].tool_calls, tool_calls);
    assert_eq!(parent_activity[0].references, references);

    let (child, provenance) = sessions
        .branch_from_message(parent.id, parent_assistant.id)
        .await
        .unwrap();
    assert_eq!(provenance.parent_session_id, parent.id);
    assert_eq!(provenance.boundary_message_id, Some(parent_assistant.id));

    let child_messages = sessions.messages(child.id, 50).await.unwrap();
    assert_eq!(child_messages.len(), 2);
    assert_eq!(child_messages[0].role, MessageRole::User);
    assert_eq!(child_messages[1].role, MessageRole::Assistant);
    assert_ne!(child_messages[1].id, parent_assistant.id);
    assert_eq!(inline_body(&child_messages[1]), "answer");

    let child_activity = sessions.assistant_activity(child.id, 50).await.unwrap();
    assert_eq!(child_activity.len(), 1);
    assert_eq!(child_activity[0].message_id, child_messages[1].id);
    assert_eq!(
        child_activity[0].reasoning_summary,
        "Checked the relevant constraints."
    );
    assert_eq!(child_activity[0].tool_calls, tool_calls);
    assert_eq!(child_activity[0].references, references);
}

#[tokio::test]
async fn child_activity_append_survives_manager_and_service_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(Storage::open_in_memory(dir.path().join("legacy-blobs")).unwrap());
    let branch_path = dir.path().join("branch.db");
    let (sessions, manager) = service(Arc::clone(&storage), &branch_path).await;

    let parent = sessions.create("Parent").await.unwrap();
    sessions
        .append_text(parent.id, MessageRole::User, "request")
        .await
        .unwrap();
    let boundary = sessions
        .append_text(parent.id, MessageRole::Assistant, "boundary answer")
        .await
        .unwrap();
    let (child, _) = sessions
        .branch_from_message(parent.id, boundary.id)
        .await
        .unwrap();
    assert!(sessions
        .assistant_activity(child.id, 50)
        .await
        .unwrap()
        .is_empty());

    drop(sessions);
    drop(manager);

    let reopened_manager = Arc::new(SessionManager::open_branch_workspace(&branch_path).unwrap());
    let reopened = SessionService::with_branch_manager(Arc::clone(&storage), reopened_manager);
    let (tool_calls, references) = activity_fixture();
    let appended = reopened
        .append_assistant_with_activity(
            child.id,
            "child answer",
            Some("Summarized the completed operation.".to_owned()),
            tool_calls.clone(),
            references.clone(),
        )
        .await
        .unwrap();

    let messages = reopened.messages(child.id, 50).await.unwrap();
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[2].id, appended.id);
    assert_eq!(messages[2].role, MessageRole::Assistant);
    assert_eq!(inline_body(&messages[2]), "child answer");

    let activity = reopened.assistant_activity(child.id, 50).await.unwrap();
    assert_eq!(activity.len(), 1);
    assert_eq!(activity[0].message_id, appended.id);
    assert_eq!(
        activity[0].reasoning_summary,
        "Summarized the completed operation."
    );
    assert_eq!(activity[0].tool_calls, tool_calls);
    assert_eq!(activity[0].references, references);

    drop(reopened);
    let final_manager = Arc::new(SessionManager::open_branch_workspace(&branch_path).unwrap());
    let final_service = SessionService::with_branch_manager(storage, final_manager);
    let reloaded_activity = final_service
        .assistant_activity(child.id, 50)
        .await
        .unwrap();
    assert_eq!(reloaded_activity, activity);
}

#[tokio::test]
async fn malformed_or_over_bound_child_activity_has_no_message_side_effect() {
    let dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(Storage::open_in_memory(dir.path().join("legacy-blobs")).unwrap());
    let branch_path = dir.path().join("branch.db");
    let (sessions, _manager) = service(Arc::clone(&storage), &branch_path).await;

    let parent = sessions.create("Parent").await.unwrap();
    sessions
        .append_text(parent.id, MessageRole::User, "request")
        .await
        .unwrap();
    let boundary = sessions
        .append_text(parent.id, MessageRole::Assistant, "boundary answer")
        .await
        .unwrap();
    let (child, _) = sessions
        .branch_from_message(parent.id, boundary.id)
        .await
        .unwrap();
    let before = sessions.messages(child.id, 50).await.unwrap();

    let malformed = AssistantToolCall {
        call_id: "call-invalid".to_owned(),
        name: "search".to_owned(),
        state: "running".to_owned(),
        ok: true,
    };
    assert!(sessions
        .append_assistant_with_activity(
            child.id,
            "must not persist",
            Some("valid summary".to_owned()),
            vec![malformed],
            Vec::new(),
        )
        .await
        .is_err());
    assert_eq!(sessions.messages(child.id, 50).await.unwrap(), before);

    let over_bound = vec![
        AssistantToolCall {
            call_id: "call-over-bound".to_owned(),
            name: "search".to_owned(),
            state: "completed".to_owned(),
            ok: true,
        };
        129
    ];
    assert!(sessions
        .append_assistant_with_activity(
            child.id,
            "must not persist",
            Some("valid summary".to_owned()),
            over_bound,
            Vec::new(),
        )
        .await
        .is_err());
    assert_eq!(sessions.messages(child.id, 50).await.unwrap(), before);

    let over_bound_reasoning = "r".repeat(MAX_REASONING_SUMMARY_BYTES + 1);
    assert!(sessions
        .append_assistant_with_activity(
            child.id,
            "must not persist",
            Some(over_bound_reasoning),
            Vec::new(),
            Vec::new(),
        )
        .await
        .is_err());
    assert_eq!(sessions.messages(child.id, 50).await.unwrap(), before);
}
