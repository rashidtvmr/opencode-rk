//! The typed-history ownership index name is part of the schema contract.
//! Reusing that name for a semantically different index must not be accepted.
use std::path::Path;

use opencode_rk_contracts::{MessageId, MessageRecord, MessageRole, PayloadRef, SessionId, SessionState, SessionSummary, Timestamp};
use opencode_rk_storage::{HistoryItem, Storage, StorageError, StoragePaths};
use rusqlite::Connection;
use tempfile::tempdir;

fn paths(dir: &Path) -> StoragePaths { StoragePaths::under(dir.join("db")) }

fn session(storage: &Storage) -> SessionId {
    let id = SessionId::new();
    let now = Timestamp::now();
    storage.create_session(&SessionSummary {
        id, title: "index-fixture".into(), state: SessionState::Active,
        created_at: now, updated_at: now, archived_at: None,
    }).unwrap();
    id
}

fn user_message(session_id: SessionId) -> MessageRecord {
    MessageRecord {
        id: MessageId::new(), session_id, role: MessageRole::User,
        body: PayloadRef::inline("index fixture message").unwrap(),
        created_at: Timestamp::now(),
    }
}

#[test]
fn same_name_wrong_typed_history_index_is_rejected_on_reopen() {
    let dir = tempdir().unwrap();
    let database = paths(dir.path()).database;
    {
        let storage = Storage::open(paths(dir.path())).unwrap();
        let id = session(&storage);
        storage.append_message(&user_message(id)).unwrap();
    }
    {
        let db = Connection::open(&database).unwrap();
        db.execute_batch(
            "DROP INDEX typed_tool_message_idx;
             CREATE INDEX typed_tool_message_idx ON typed_tool_records(kind);",
        ).unwrap();
    }
    let result = Storage::open(paths(dir.path()));
    assert!(matches!(&result, Err(StorageError::TypedHistorySchema)),
        "same-name semantically wrong index must fail schema validation: {:?}",
        result.as_ref().err());
}

#[test]
fn valid_typed_history_index_reopens_without_changing_history() {
    let dir = tempdir().unwrap();
    let p = paths(dir.path());
    let id;
    let message_id;
    {
        let storage = Storage::open(p.clone()).unwrap();
        id = session(&storage);
        let message = user_message(id);
        message_id = message.id;
        storage.append_message(&message).unwrap();
    }
    let storage = Storage::open(p).unwrap();
    let history = storage.bounded_history(id, 16, 64 * 1024).unwrap();
    assert!(matches!(history.as_slice(),
        [HistoryItem::Message(message)]
        if message.id == message_id
            && message.session_id == id
            && message.role == MessageRole::User
            && message.body == PayloadRef::inline("index fixture message").unwrap()));
}
