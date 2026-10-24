//! Corrupt persisted checkpoint fields fail closed before being returned.
use opencode_rk_contracts::{MessageId, MessageRecord, MessageRole, PayloadRef, SessionId, SessionState, SessionSummary, Timestamp};
use opencode_rk_storage::{session_compaction::{CompactionCheckpoint, MAX_WINDOW_BYTES}, Storage, StoragePaths};

#[test]
fn oversized_or_misaccounted_persisted_checkpoint_is_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let paths = StoragePaths::under(directory.path().join("storage"));
    let storage = Storage::open(paths.clone()).unwrap();
    let session = SessionId::new();
    let now = Timestamp::now();
    storage.create_session(&SessionSummary { id:session, title:"Checkpoint bounds".into(), state:SessionState::Active, created_at:now, updated_at:now, archived_at:None }).unwrap();
    let boundary = MessageId::new();
    storage.append_message(&MessageRecord { id:boundary, session_id:session, role:MessageRole::Assistant, body:PayloadRef::inline("boundary").unwrap(), created_at:now }).unwrap();
    let checkpoint = CompactionCheckpoint { boundary_message_id:boundary, provider_id:"openai".into(), model_id:"fixture".into(), window_json:r#"{"id":"window"}"#.into() };
    storage.save_compaction_checkpoint(session, &checkpoint).unwrap();
    assert_eq!(storage.compaction_checkpoint(session).unwrap(), Some(checkpoint.clone()));
    // Corruption injection is restricted to this disposable fixture database.
    let raw = rusqlite::Connection::open(&paths.database).unwrap();
    raw.execute_batch("PRAGMA ignore_check_constraints=ON;").unwrap();
    raw.execute("UPDATE session_compaction_checkpoints SET window_json=?1,window_bytes=?2 WHERE session_id=?3", rusqlite::params!["x".repeat(MAX_WINDOW_BYTES + 1), (MAX_WINDOW_BYTES + 1) as i64, session.to_string()]).unwrap();
    assert!(storage.compaction_checkpoint(session).is_err(), "oversized persisted JSON must not be returned");
    raw.execute("UPDATE session_compaction_checkpoints SET window_json=?1,window_bytes=1 WHERE session_id=?2", rusqlite::params![checkpoint.window_json, session.to_string()]).unwrap();
    assert!(storage.compaction_checkpoint(session).is_err(), "declared bytes must match actual UTF-8 bytes");
}
