//! Display-history reads must validate typed ownership globally, rather than
//! hiding malformed rows through a session-anchored JOIN/filter.
use std::path::{Path, PathBuf};

use opencode_rk_contracts::{MessageId, MessageRecord, MessageRole, PayloadRef, SessionId, SessionState, SessionSummary, Timestamp};
use opencode_rk_storage::{Storage, StorageError, StoragePaths, ToolPair};
use rusqlite::{params, Connection};
use tempfile::tempdir;

fn paths(dir: &Path) -> StoragePaths { StoragePaths::under(dir.join("db")) }
fn session(storage: &Storage, title: &str) -> SessionId {
    let id = SessionId::new(); let now = Timestamp::now();
    storage.create_session(&SessionSummary { id, title: title.into(), state: SessionState::Active,
        created_at: now, updated_at: now, archived_at: None }).unwrap(); id
}
fn message(session_id: SessionId, role: MessageRole, text: &str) -> MessageRecord {
    MessageRecord { id: MessageId::new(), session_id, role,
        body: PayloadRef::inline(text).unwrap(), created_at: Timestamp::now() }
}

struct Fixture {
    _dir: tempfile::TempDir,
    storage: Storage,
    database: PathBuf,
    owner: SessionId,
    victim: SessionId,
    unrelated: SessionId,
    owner_turn: MessageId,
    victim_user: MessageId,
    victim_display: MessageId,
    owner_display: MessageId,
}

fn fixture() -> Fixture {
    let dir = tempdir().unwrap(); let storage = Storage::open(paths(dir.path())).unwrap();
    let owner = session(&storage, "owner"); let victim = session(&storage, "victim");
    let unrelated = session(&storage, "unrelated");
    let owner_turn = message(owner, MessageRole::User, "owner prompt");
    let victim_user = message(victim, MessageRole::User, "victim prompt");
    let unrelated_user = message(unrelated, MessageRole::User, "unrelated prompt");
    storage.append_message(&owner_turn).unwrap();
    storage.append_message(&victim_user).unwrap();
    let victim_display = message(victim, MessageRole::Tool, "victim display");
    storage.append_message(&victim_display).unwrap();
    storage.append_message(&unrelated_user).unwrap();
    let round = storage.begin_tool_round(owner, owner_turn.id, "round-owner", 0, 1).unwrap();
    let owner_display = message(owner, MessageRole::Tool, "owner display");
    storage.append_tool_pair(&round, 0, &ToolPair { call_id: "call".into(), name: "fixture".into(),
        input: PayloadRef::inline("{}").unwrap(), output: PayloadRef::inline("ok").unwrap(), message: owner_display.clone() }).unwrap();
    Fixture { database: paths(dir.path()).database, _dir: dir, storage, owner, victim, unrelated,
        owner_turn: owner_turn.id, victim_user: victim_user.id, victim_display: victim_display.id,
        owner_display: owner_display.id }
}

fn expect_identity(result: Result<impl std::fmt::Debug, StorageError>) {
    assert!(matches!(result, Err(StorageError::TypedHistoryIdentity)),
        "malformed typed history must be rejected, got {result:?}");
}
fn valid_display(f: &Fixture) {
    assert_eq!(f.storage.display_tool_history(f.owner, 32).unwrap().len(), 2);
}

#[test]
fn display_history_rejects_foreign_display_link_for_owner_and_victim() {
    let f = fixture(); valid_display(&f);
    let db = Connection::open(&f.database).unwrap();
    db.execute("UPDATE typed_tool_records SET message_id=?1 WHERE round_id=?2",
        params![f.victim_display.to_string(), "round-owner"]).unwrap();
    expect_identity(f.storage.display_tool_history(f.owner, 32));
    expect_identity(f.storage.display_tool_history(f.victim, 32));
    // A corrupted pair in other sessions must not make ordinary history fail.
    assert!(f.storage.bounded_history(f.unrelated, 32, 64 * 1024).is_ok());
}

#[test]
fn typed_readers_reject_foreign_round_turn_link_before_payload() {
    let f = fixture(); valid_display(&f);
    let db = Connection::open(&f.database).unwrap();
    db.execute("UPDATE tool_rounds SET turn_message_id=?1 WHERE round_id=?2",
        params![f.victim_user.to_string(), "round-owner"]).unwrap();
    expect_identity(f.storage.bounded_history(f.owner, 32, 64 * 1024));
    expect_identity(f.storage.display_tool_history(f.owner, 32));
    let _ = (f.owner_turn, f.owner_display);
}
