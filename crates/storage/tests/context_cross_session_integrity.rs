//! Cross-session typed-history corruption must fail closed at every read seam.
//!
//! The fixture is deliberately installed in a disposable database: SQLite's
//! foreign key on `typed_tool_records.message_id` still permits a record in
//! round A to point at a tool display belonging to session B.
use std::path::{Path, PathBuf};

use opencode_rk_contracts::{MessageId, MessageRecord, MessageRole, PayloadRef, SessionId, SessionState, SessionSummary, Timestamp};
use opencode_rk_storage::{Storage, StorageError, StoragePaths, ToolPair};
use rusqlite::{params, Connection};
use tempfile::tempdir;

fn paths(dir: &Path) -> StoragePaths { StoragePaths::under(dir.join("db")) }

fn session(storage: &Storage, title: &str) -> SessionId {
    let id = SessionId::new();
    let now = Timestamp::now();
    storage.create_session(&SessionSummary {
        id, title: title.into(), state: SessionState::Active,
        created_at: now, updated_at: now, archived_at: None,
    }).unwrap();
    id
}

fn message(session_id: SessionId, role: MessageRole, text: &str) -> MessageRecord {
    MessageRecord { id: MessageId::new(), session_id, role,
        body: PayloadRef::inline(text).unwrap(), created_at: Timestamp::now() }
}

fn fixture() -> (tempfile::TempDir, Storage, PathBuf, SessionId, SessionId, MessageId, MessageId) {
    let dir = tempdir().unwrap();
    let storage = Storage::open(paths(dir.path())).unwrap();
    let a = session(&storage, "owner");
    let b = session(&storage, "victim");
    let turn = message(a, MessageRole::User, "prompt");
    storage.append_message(&turn).unwrap();
    let round = storage.begin_tool_round(a, turn.id, "round-a", 0, 1).unwrap();
    let owner_display = message(a, MessageRole::Tool, "tool-a");
    storage.append_tool_pair(&round, 0, &ToolPair {
        call_id: "call-a".into(), name: "fixture.tool".into(),
        input: PayloadRef::inline("{}").unwrap(), output: PayloadRef::inline("ok").unwrap(),
        message: owner_display.clone(),
    }).unwrap();

    // B's display is intentionally a normal, unlinked tool message. Updating
    // both A records below preserves the message FK while crossing ownership.
    let victim = message(b, MessageRole::Tool, "victim-display");
    storage.append_message(&victim).unwrap();
    let database = paths(dir.path()).database;
    (dir, storage, database, a, b, owner_display.id, victim.id)
}

fn corrupt(database: &Path, victim: MessageId) {
    let db = Connection::open(database).unwrap();
    db.execute("UPDATE typed_tool_records SET message_id=?1 WHERE round_id=?2",
        params![victim.to_string(), "round-a"]).unwrap();
}

fn assert_integrity<T: std::fmt::Debug>(result: Result<T, StorageError>) {
    assert!(matches!(result,
        Err(StorageError::TypedHistoryIdentity)
        | Err(StorageError::TypedHistoryIncomplete)
        | Err(StorageError::UnlinkedToolHistory)),
        "cross-session corruption must be rejected as integrity failure, got {result:?}");
}

#[test]
fn cross_session_round_links_are_rejected_by_raw_and_context_history() {
    let (dir, storage, database, owner, victim, owner_message, victim_message) = fixture();
    assert!(storage.bounded_history(owner, 32, 64 * 1024).is_ok());
    assert!(storage.bounded_context_history(owner, None, 32, 64 * 1024).is_ok());
    assert!(storage.context_tool_preview(owner, owner_message).is_ok());
    corrupt(&database, victim_message);
    let _ = dir;
    assert_integrity(storage.bounded_history(owner, 32, 64 * 1024));
    assert_integrity(storage.bounded_history(victim, 32, 64 * 1024));
    assert_integrity(storage.bounded_context_history(victim, None, 32, 64 * 1024));
}

#[test]
fn cross_session_round_links_are_rejected_by_tool_preview() {
    let (dir, storage, database, _, victim, _, victim_message) = fixture();
    corrupt(&database, victim_message);
    let _ = dir;
    assert_integrity(storage.context_tool_preview(victim, victim_message));
}
