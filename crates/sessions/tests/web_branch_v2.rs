use opencode_rk_contracts::MessageRole;
use opencode_rk_sessions::SessionManager;

#[test]
fn web_008_t05_fork_provenance_survives_workspace_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("web-branches-v2.db");
    let manager = SessionManager::open_branch_workspace(&path).unwrap();
    let parent = manager.create_session("Parent chat").unwrap();
    let boundary = manager
        .append_message(parent, MessageRole::User, "first request")
        .unwrap();
    let (child, provenance) = manager
        .fork_from_message(parent, boundary, "Branch: Parent chat")
        .unwrap();

    assert_eq!(provenance.parent_session_id, parent);
    assert_eq!(provenance.fork_message_seq, 1);
    assert_eq!(provenance.boundary_message_id, Some(boundary));
    drop(manager);

    let reopened = SessionManager::open_branch_workspace(&path).unwrap();
    let provenance = reopened.fork_provenance(child.id).unwrap().unwrap();
    assert_eq!(provenance.parent_session_id, parent);
    assert_eq!(provenance.fork_message_seq, 1);
    assert_eq!(provenance.boundary_message_id, Some(boundary));
}
