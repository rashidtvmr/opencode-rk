//! Instruction selection survives reopening a disposable database and stays session-local.
use opencode_rk_contracts::{SessionId, SessionState, SessionSummary, Timestamp};
use opencode_rk_storage::{Storage, StorageError, StoragePaths};

fn session(storage: &Storage) -> SessionId {
    let id = SessionId::new();
    let now = Timestamp::now();
    storage
        .create_session(&SessionSummary {
            id,
            title: "Memory selection".into(),
            state: SessionState::Active,
            created_at: now,
            updated_at: now,
            archived_at: None,
        })
        .unwrap();
    id
}

#[test]
fn exclusions_survive_reopen_stay_isolated_and_reinclude_is_durable() {
    let directory = tempfile::tempdir().unwrap();
    let paths = || StoragePaths::under(directory.path().join("data"));
    let source = directory.path().join("AGENTS.md");
    std::fs::write(&source, "durable instruction source").unwrap();
    let path = source.to_str().unwrap();
    let storage = Storage::open(paths()).unwrap();
    let first = session(&storage);
    let second = session(&storage);
    storage.set_memory_included(first, path, false).unwrap();
    storage.set_memory_included(first, path, false).unwrap();
    drop(storage);

    let storage = Storage::open(paths()).unwrap();
    assert_eq!(
        storage.memory_exclusions(first).unwrap(),
        vec![path.to_owned()]
    );
    assert!(storage.memory_exclusions(second).unwrap().is_empty());
    storage.set_memory_included(first, path, true).unwrap();
    drop(storage);

    let storage = Storage::open(paths()).unwrap();
    assert!(storage.memory_exclusions(first).unwrap().is_empty());
    assert_eq!(
        std::fs::read_to_string(source).unwrap(),
        "durable instruction source"
    );
}

#[test]
fn overflow_and_unknown_session_leave_existing_selection_intact() {
    let directory = tempfile::tempdir().unwrap();
    let storage = Storage::open_in_memory(directory.path().join("blobs")).unwrap();
    let id = session(&storage);
    for index in 0..64 {
        storage
            .set_memory_included(id, &format!("/fixture/source-{index}"), false)
            .unwrap();
    }
    let before = storage.memory_exclusions(id).unwrap();
    assert!(storage
        .set_memory_included(id, "/fixture/overflow", false)
        .is_err());
    assert_eq!(storage.memory_exclusions(id).unwrap(), before);
    // An idempotent exclusion at capacity is still valid.
    storage.set_memory_included(id, &before[0], false).unwrap();
    assert!(matches!(
        storage.set_memory_included(SessionId::new(), "/fixture/source", false),
        Err(StorageError::SessionNotFound(_))
    ));
    assert_eq!(storage.memory_exclusions(id).unwrap(), before);
}
