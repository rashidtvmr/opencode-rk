//! DB-023 component tests for the existing typed tool-history Storage seam.
//! These use only disposable, file-backed databases and the public API; raw
//! SQLite is used solely to install corruption/fault fixtures.
use std::path::Path;

use opencode_rk_contracts::{MessageId, MessageRecord, MessageRole, PayloadRef, SessionId, SessionState, SessionSummary, Timestamp};
use opencode_rk_storage::{HistoryItem, Storage, StorageError, StoragePaths, ToolPair};
use rusqlite::{params, Connection};
use tempfile::tempdir;

fn paths(dir: &Path) -> StoragePaths { StoragePaths::under(dir.join("db")) }

fn session(storage: &Storage, title: &str) -> SessionId {
    let id = SessionId::new();
    let now = Timestamp::now();
    storage.create_session(&SessionSummary { id, title: title.into(), state: SessionState::Active, created_at: now, updated_at: now, archived_at: None }).unwrap();
    id
}

fn message(session_id: SessionId, role: MessageRole, text: &str) -> MessageRecord {
    MessageRecord { id: MessageId::new(), session_id, role, body: PayloadRef::inline(text).unwrap(), created_at: Timestamp::now() }
}

fn pair(session_id: SessionId, call_id: &str, name: &str, input: &str, output: &str) -> ToolPair {
    ToolPair { call_id: call_id.into(), name: name.into(), input: PayloadRef::inline(input).unwrap(), output: PayloadRef::inline(output).unwrap(), message: message(session_id, MessageRole::Tool, "tool display") }
}

fn typed(items: &[HistoryItem]) -> Vec<(String, u64, String, String, String)> {
    items.iter().filter_map(|item| match item {
        HistoryItem::Tool(t) => Some((t.call_id.clone(), t.pair_index, t.kind.clone(), t.name.clone(), t.payload.clone())),
        HistoryItem::Message(_) => None,
    }).collect()
}

#[test]
fn round_trip_reopen_orders_calls_before_outputs_and_isolates_sessions() {
    let dir = tempdir().unwrap();
    let p = paths(dir.path());
    let sid;
    {
        let storage = Storage::open(p.clone()).unwrap();
        sid = session(&storage, "primary");
        let other = session(&storage, "other");
        let turn = message(sid, MessageRole::User, "prompt");
        storage.append_message(&turn).unwrap();
        let round = storage.begin_tool_round(sid, turn.id, "round-α", 0, 2).unwrap();
        storage.append_tool_pair(&round, 0, &pair(sid, "call-1\0", "réad", "arg\0一", "out\0✓")).unwrap();
        storage.append_tool_pair(&round, 1, &pair(sid, "call-2", "write", "second", "done")).unwrap();
        assert!(storage.bounded_history(other, 100, 512 * 1024).unwrap().is_empty());
    }
    let storage = Storage::open(p).unwrap();
    let rows = typed(&storage.bounded_history(sid, 100, 512 * 1024).unwrap());
    assert_eq!(rows.iter().map(|r| r.2.as_str()).collect::<Vec<_>>(), ["call", "call", "output", "output"]);
    assert_eq!(rows[0], ("call-1\0".into(), 0, "call".into(), "réad".into(), "arg\0一".into()));
    assert_eq!(rows[3].4, "done");
}

#[test]
fn incomplete_noncontiguous_and_unlinked_legacy_tool_fail_closed() {
    let dir = tempdir().unwrap();
    let p = paths(dir.path());
    let storage = Storage::open(p.clone()).unwrap();
    let sid = session(&storage, "invalid");
    let turn = message(sid, MessageRole::User, "prompt");
    storage.append_message(&turn).unwrap();
    let round = storage.begin_tool_round(sid, turn.id, "broken", 0, 2).unwrap();
    storage.append_tool_pair(&round, 0, &pair(sid, "only", "tool", "in", "out")).unwrap();
    assert!(matches!(storage.bounded_history(sid, 100, 512 * 1024), Err(StorageError::TypedHistoryIncomplete)));
    let legacy = message(sid, MessageRole::Tool, "unlinked");
    storage.append_message(&legacy).unwrap();
    let raw = Connection::open(&p.database).unwrap();
    raw.execute("DELETE FROM typed_tool_records WHERE round_id=?1", params![round.round_id]).unwrap();
    raw.execute("DELETE FROM tool_rounds WHERE round_id=?1", params![round.round_id]).unwrap();
    drop(raw);
    assert!(matches!(storage.bounded_history(sid, 100, 512 * 1024), Err(StorageError::UnlinkedToolHistory)));
}

#[test]
fn caller_item_and_byte_limits_reject_without_materializing_history() {
    let dir = tempdir().unwrap();
    let storage = Storage::open(paths(dir.path())).unwrap();
    let sid = session(&storage, "limits");
    let turn = message(sid, MessageRole::User, "prompt");
    storage.append_message(&turn).unwrap();
    let huge_id = "x".repeat(2048);
    let round = storage.begin_tool_round(sid, turn.id, "large", 0, 1).unwrap();
    storage.append_tool_pair(&round, 0, &pair(sid, &huge_id, "name", "input", "output")).unwrap();
    let ok = storage.bounded_history(sid, 100, 512 * 1024).unwrap();
    assert!(typed(&ok).iter().any(|row| row.0 == huge_id));
    assert!(matches!(storage.bounded_history(sid, 2, 512 * 1024), Err(StorageError::TypedHistoryLimit)));
    assert!(matches!(storage.bounded_history(sid, 100, 16), Err(StorageError::TypedHistoryLimit)));
}

#[test]
fn pair_append_trigger_rolls_back_message_call_and_output() {
    let dir = tempdir().unwrap();
    let p = paths(dir.path());
    let storage = Storage::open(p.clone()).unwrap();
    let sid = session(&storage, "rollback");
    let turn = message(sid, MessageRole::User, "prompt");
    storage.append_message(&turn).unwrap();
    let round = storage.begin_tool_round(sid, turn.id, "faulty", 0, 1).unwrap();
    let raw = Connection::open(&p.database).unwrap();
    raw.execute_batch("CREATE TRIGGER typed_pair_fault BEFORE INSERT ON typed_tool_records WHEN NEW.kind='output' AND NEW.call_id='fault-call' BEGIN SELECT RAISE(ABORT, 'typed pair fault'); END;").unwrap();
    drop(raw);
    let fault = pair(sid, "fault-call", "tool", "input", "output");
    assert!(storage.append_tool_pair(&round, 0, &fault).is_err());
    let check = Connection::open(&p.database).unwrap();
    assert_eq!(check.query_row("SELECT count(*) FROM messages WHERE role='tool'", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
    assert_eq!(check.query_row("SELECT count(*) FROM typed_tool_records WHERE round_id=?1", params![round.round_id], |r| r.get::<_, i64>(0)).unwrap(), 0);
    assert_eq!(check.query_row("SELECT count(*) FROM tool_rounds", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
}

#[test]
fn malformed_raw_identity_is_a_typed_bounded_error() {
    let dir = tempdir().unwrap();
    let p = paths(dir.path());
    let storage = Storage::open(p.clone()).unwrap();
    let sid = session(&storage, "malformed");
    let turn = message(sid, MessageRole::User, "prompt");
    storage.append_message(&turn).unwrap();
    let round = storage.begin_tool_round(sid, turn.id, "raw", 0, 1).unwrap();
    storage.append_tool_pair(&round, 0, &pair(sid, "valid", "tool", "in", "out")).unwrap();
    let raw = Connection::open(&p.database).unwrap();
    // The typed table's real CHECK constraint correctly rejects an empty ID;
    // install a malformed legacy message identity instead, which is the raw
    // compatibility path bounded_history must reject without allocation.
    raw.execute("INSERT INTO messages(id,session_id,role,inline_text,blob_hash,byte_len,created_at) VALUES('not-a-message-id',?1,'assistant','legacy',NULL,6,?2)", params![sid.to_string(), Timestamp::now().to_string()]).unwrap();
    drop(raw);
    assert!(matches!(storage.bounded_history(sid, 100, 512 * 1024), Err(StorageError::TypedHistoryIncomplete | StorageError::TypedHistoryIdentity | StorageError::Sqlite(_))));
}

#[test]
fn zero_expected_pairs_with_no_records_fails_closed() {
    let dir = tempdir().unwrap();
    let storage = Storage::open(paths(dir.path())).unwrap();
    let sid = session(&storage, "zero");
    let turn = message(sid, MessageRole::User, "prompt");
    storage.append_message(&turn).unwrap();
    storage.begin_tool_round(sid, turn.id, "zero-round", 0, 0).unwrap();
    assert!(matches!(storage.bounded_history(sid, 100, 512 * 1024), Err(StorageError::TypedHistoryIncomplete)));
}

#[test]
fn valid_noncontiguous_pair_indexes_fail_closed() {
    let dir = tempdir().unwrap();
    let p = paths(dir.path());
    let storage = Storage::open(p.clone()).unwrap();
    let sid = session(&storage, "noncontiguous");
    let turn = message(sid, MessageRole::User, "prompt");
    storage.append_message(&turn).unwrap();
    let round = storage.begin_tool_round(sid, turn.id, "gap", 0, 2).unwrap();
    storage.append_tool_pair(&round, 0, &pair(sid, "first", "tool", "in", "out")).unwrap();
    storage.append_tool_pair(&round, 1, &pair(sid, "second", "tool", "in2", "out2")).unwrap();
    let raw = Connection::open(&p.database).unwrap();
    raw.execute("UPDATE typed_tool_records SET pair_index=2 WHERE round_id=?1 AND pair_index=1", params![round.round_id]).unwrap();
    drop(raw);
    assert!(matches!(storage.bounded_history(sid, 100, 512 * 1024), Err(StorageError::TypedHistoryIncomplete)));
}

#[test]
fn cross_row_name_and_message_identity_mismatch_fails_closed() {
    for mismatch in ["name", "message_id"] {
        let dir = tempdir().unwrap();
        let p = paths(dir.path());
        let storage = Storage::open(p.clone()).unwrap();
        let sid = session(&storage, mismatch);
        let turn = message(sid, MessageRole::User, "prompt");
        storage.append_message(&turn).unwrap();
        let round = storage.begin_tool_round(sid, turn.id, mismatch, 0, 1).unwrap();
        storage.append_tool_pair(&round, 0, &pair(sid, "identity", "tool", "in", "out")).unwrap();
        let alternate = message(sid, MessageRole::Tool, "alternate");
        storage.append_message(&alternate).unwrap();
        let raw = Connection::open(&p.database).unwrap();
        if mismatch == "name" {
            raw.execute("UPDATE typed_tool_records SET name='different' WHERE round_id=?1 AND kind='output'", params![round.round_id]).unwrap();
        } else {
            raw.execute("UPDATE typed_tool_records SET message_id=?1 WHERE round_id=?2 AND kind='output'", params![alternate.id.to_string(), round.round_id]).unwrap();
        }
        drop(raw);
        assert!(matches!(storage.bounded_history(sid, 100, 512 * 1024), Err(StorageError::TypedHistoryIncomplete)));
    }
}

#[test]
fn successful_pair_remains_durable_when_later_pair_aborts() {
    let dir = tempdir().unwrap();
    let p = paths(dir.path());
    let storage = Storage::open(p.clone()).unwrap();
    let sid = session(&storage, "neighbor");
    let turn = message(sid, MessageRole::User, "prompt");
    storage.append_message(&turn).unwrap();
    let round = storage.begin_tool_round(sid, turn.id, "neighbor-round", 0, 2).unwrap();
    storage.append_tool_pair(&round, 0, &pair(sid, "good-call", "tool", "good-input", "good-output")).unwrap();
    let raw = Connection::open(&p.database).unwrap();
    raw.execute_batch("CREATE TRIGGER typed_pair_fault_later BEFORE INSERT ON typed_tool_records WHEN NEW.kind='output' AND NEW.call_id='bad-call' BEGIN SELECT RAISE(ABORT, 'later pair fault'); END;").unwrap();
    drop(raw);
    assert!(storage.append_tool_pair(&round, 1, &pair(sid, "bad-call", "tool", "bad-input", "bad-output")).is_err());
    let check = Connection::open(&p.database).unwrap();
    assert_eq!(check.query_row("SELECT count(*) FROM messages WHERE role='tool'", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
    assert_eq!(check.query_row("SELECT count(*) FROM typed_tool_records WHERE round_id=?1 AND pair_index=0", params![round.round_id], |r| r.get::<_, i64>(0)).unwrap(), 2);
    assert_eq!(check.query_row("SELECT count(*) FROM typed_tool_records WHERE round_id=?1 AND pair_index=1", params![round.round_id], |r| r.get::<_, i64>(0)).unwrap(), 0);
    assert_eq!(check.query_row("SELECT payload FROM typed_tool_records WHERE round_id=?1 AND pair_index=0 AND kind='output'", params![round.round_id], |r| r.get::<_, String>(0)).unwrap(), "good-output");
}

#[test]
fn positive_expected_pairs_with_zero_records_fails_closed() {
    let dir = tempdir().unwrap();
    let storage = Storage::open(paths(dir.path())).unwrap();
    let sid = session(&storage, "positive-zero-records");
    let turn = message(sid, MessageRole::User, "prompt");
    storage.append_message(&turn).unwrap();
    storage.begin_tool_round(sid, turn.id, "missing-records", 0, 1).unwrap();
    assert!(matches!(storage.bounded_history(sid, 100, 512 * 1024), Err(StorageError::TypedHistoryIncomplete)));
}
