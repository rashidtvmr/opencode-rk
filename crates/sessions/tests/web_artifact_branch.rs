use std::sync::Arc;

use opencode_rk_contracts::{ArtifactKind, MessageRole};
use opencode_rk_sessions::{SessionManager, SessionService};
use opencode_rk_storage::Storage;
use tempfile::tempdir;

#[tokio::test]
async fn web_017_branch_sessions_can_link_artifacts_to_inherited_assistant_messages() {
    let dir = tempdir().unwrap();
    let storage = Arc::new(Storage::open_in_memory(dir.path().join("blobs")).unwrap());
    let manager = Arc::new(
        SessionManager::open_branch_workspace(&dir.path().join("branches.db")).unwrap(),
    );
    let sessions = SessionService::with_branch_manager(storage, manager);
    let parent = sessions.create("Parent").await.unwrap();
    sessions
        .append_text(parent.id, MessageRole::User, "request")
        .await
        .unwrap();
    let assistant = sessions
        .append_text(parent.id, MessageRole::Assistant, "answer")
        .await
        .unwrap();

    let (child, _) = sessions
        .branch_from_message(parent.id, assistant.id)
        .await
        .unwrap();
    let child_assistant = sessions
        .messages(child.id, 10)
        .await
        .unwrap()
        .into_iter()
        .find(|message| message.role == MessageRole::Assistant)
        .unwrap();
    let artifact = sessions
        .create_artifact(
            child.id,
            child_assistant.id,
            ArtifactKind::Writing,
            "Branch draft".to_owned(),
            None,
            "editable branch artifact".to_owned(),
        )
        .await
        .unwrap();

    assert_eq!(artifact.summary.session_id, child.id);
    assert_eq!(artifact.summary.source_message_id, child_assistant.id);
    assert_eq!(artifact.content, "editable branch artifact");
    assert_eq!(sessions.artifacts(child.id, 32).await.unwrap().len(), 1);
}
