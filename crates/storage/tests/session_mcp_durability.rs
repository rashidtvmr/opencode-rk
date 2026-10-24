//! Session MCP selection persists without disabling another session's tools.
use opencode_rk_contracts::{SessionId, SessionState, SessionSummary, Timestamp};
use opencode_rk_storage::{Storage, StorageError, StoragePaths};

fn session(storage: &Storage) -> SessionId {
    let id = SessionId::new();
    let now = Timestamp::now();
    storage.create_session(&SessionSummary {
        id, title: "MCP context selection".into(), state: SessionState::Active,
        created_at: now, updated_at: now, archived_at: None,
    }).unwrap();
    id
}

#[test]
fn selection_survives_reopen_is_session_local_and_can_be_restored() {
    let root = tempfile::tempdir().unwrap();
    let paths = || StoragePaths::under(root.path().join("data"));
    let storage = Storage::open(paths()).unwrap();
    let first = session(&storage);
    let second = session(&storage);
    storage.set_mcp_included(first, "alpha", false).unwrap();
    drop(storage);

    let storage = Storage::open(paths()).unwrap();
    assert_eq!(storage.mcp_exclusions(first).unwrap(), vec!["alpha"]);
    assert!(storage.mcp_exclusions(second).unwrap().is_empty());
    storage.set_mcp_included(first, "alpha", true).unwrap();
    drop(storage);

    let storage = Storage::open(paths()).unwrap();
    assert!(storage.mcp_exclusions(first).unwrap().is_empty());
    assert!(storage.mcp_exclusions(second).unwrap().is_empty());
}

#[test]
fn rejected_writes_preserve_previous_selections_and_reinclusion_frees_capacity() {
    let root = tempfile::tempdir().unwrap();
    let storage = Storage::open_in_memory(root.path().join("blobs")).unwrap();
    let id = session(&storage);
    for index in 0..128 {
        storage.set_mcp_included(id, &format!("server-{index:03}"), false).unwrap();
    }
    let before = storage.mcp_exclusions(id).unwrap();
    assert!(storage.set_mcp_included(id, "overflow", false).is_err());
    assert!(storage.set_mcp_included(id, "invalid\nname", false).is_err());
    assert!(matches!(storage.set_mcp_included(SessionId::new(), "alpha", false), Err(StorageError::SessionNotFound(_))));
    storage.set_mcp_included(id, &before[0], false).unwrap();
    assert_eq!(storage.mcp_exclusions(id).unwrap(), before);
    storage.set_mcp_included(id, &before[0], true).unwrap();
    storage.set_mcp_included(id, "replacement", false).unwrap();
    let after = storage.mcp_exclusions(id).unwrap();
    assert_eq!(after.len(), 128);
    assert!(after.iter().any(|name| name == "replacement"));
    assert!(!after.contains(&before[0]));
}
