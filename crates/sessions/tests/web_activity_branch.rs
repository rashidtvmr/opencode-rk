use std::sync::Arc;

use opencode_rk_contracts::MessageRole;
use opencode_rk_sessions::{SessionManager, SessionService};
use opencode_rk_storage::Storage;

#[tokio::test]
async fn web_009_t05_branch_inherits_persisted_reasoning_summary_without_reexecution() {
    let dir = tempfile::tempdir().unwrap();
    let storage = Arc::new(Storage::open_in_memory(dir.path().join("legacy-blobs")).unwrap());
    let branch_manager = Arc::new(
        SessionManager::open_branch_workspace(&dir.path().join("web-branches-v2.db")).unwrap(),
    );
    let sessions = SessionService::with_branch_manager(storage, branch_manager);

    let parent = sessions.create("Parent").await.unwrap();
    sessions
        .append_text(parent.id, MessageRole::User, "request")
        .await
        .unwrap();
    let assistant = sessions
        .append_assistant_with_reasoning(
            parent.id,
            "answer",
            Some("Checked the relevant constraints.".to_owned()),
        )
        .await
        .unwrap();

    let (child, _) = sessions
        .branch_from_message(parent.id, assistant.id)
        .await
        .unwrap();
    let activity = sessions.assistant_activity(child.id, 50).await.unwrap();
    assert_eq!(activity.len(), 1);
    assert_eq!(
        activity[0].reasoning_summary,
        "Checked the relevant constraints."
    );

    let child_messages = sessions.messages(child.id, 50).await.unwrap();
    assert_eq!(child_messages.len(), 2);
    assert_eq!(child_messages[1].role, MessageRole::Assistant);
}
