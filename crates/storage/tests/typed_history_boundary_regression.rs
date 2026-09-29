//! External disposable reproduction for STORAGE51; proposed as an additive
//! regression test only after task registration/controller freeze.
use std::path::Path;

use opencode_rk_contracts::{MessageId, MessageRecord, MessageRole, PayloadRef, SessionId, SessionState, SessionSummary, Timestamp};
use opencode_rk_storage::{Storage, StorageError, StoragePaths, ToolPair};
use rusqlite::{params, Connection};
use tempfile::tempdir;

fn paths(dir: &Path) -> StoragePaths { StoragePaths::under(dir.join("db")) }

fn session(storage: &Storage, title: &str) -> SessionId {
    let id = SessionId::new();
    let now = Timestamp::now();
    storage.create_session(&SessionSummary { id, title: title.into(), state: SessionState::Active, created_at: now, updated_at: now, archived_at: None }).unwrap();
    id
}

fn message(session_id: SessionId, role: MessageRole, body: PayloadRef) -> MessageRecord {
    MessageRecord { id: MessageId::new(), session_id, role, body, created_at: Timestamp::now() }
}

fn inline_pair(session_id: SessionId, call_id: &str) -> ToolPair {
    ToolPair {
        call_id: call_id.into(), name: "fixture.tool".into(),
        input: PayloadRef::inline("{}").unwrap(), output: PayloadRef::inline("ok").unwrap(),
        message: message(session_id, MessageRole::Tool, PayloadRef::inline("tool display").unwrap()),
    }
}

#[test]
fn forged_round_expected_pairs_cannot_admit_out_of_header_pair_and_leak_rows() {
    let dir = tempdir().unwrap();
    let p = paths(dir.path());
    let storage = Storage::open(p.clone()).unwrap();
    let sid = session(&storage, "forged-round-bound");
    let turn = message(sid, MessageRole::User, PayloadRef::inline("prompt").unwrap());
    storage.append_message(&turn).unwrap();
    let persisted = storage.begin_tool_round(sid, turn.id, "persisted-one-pair", 0, 1).unwrap();
    // Public fields make this a caller-forgeable handle; keep the actual
    // persisted round identity/session/turn/ordinal but inflate only its count.
    let forged = opencode_rk_storage::ToolRound { expected_pairs: 2, ..persisted.clone() };
    let result = storage.append_tool_pair(&forged, 1, &inline_pair(sid, "forged-call"));
    let check = Connection::open(&p.database).unwrap();
    let messages: i64 = check.query_row("SELECT count(*) FROM messages WHERE role='tool'", [], |r| r.get(0)).unwrap();
    let records: i64 = check.query_row("SELECT count(*) FROM typed_tool_records WHERE round_id=?1", params![persisted.round_id], |r| r.get(0)).unwrap();
    assert!(
        matches!(result, Err(StorageError::TypedHistoryIncomplete)) && messages == 0 && records == 0,
        "out-of-header append must reject atomically; result={result:?}, tool_messages={messages}, typed_records={records}"
    );
}

#[test]
fn legacy_blob_fitting_provider_input_byte_budget_is_not_charged_twice() {
    let dir = tempdir().unwrap();
    let storage = Storage::open(paths(dir.path())).unwrap();
    let sid = session(&storage, "legacy-blob-budget");
    // 300 KiB is below the accepted 512 KiB pre-JSON-framing Responses input
    // bound. Check actual serializer framing (not a guessed overhead) too.
    let payload = "b".repeat(300 * 1024);
    #[derive(serde::Serialize)]
    struct ResponsesText<'a> { role: &'static str, content: &'a str }
    // Same serialized shape/order as ResponsesItem::Text's custom serializer.
    let wire_bytes = serde_json::to_vec(&ResponsesText { role: "user", content: &payload }).unwrap().len();
    assert!(payload.len() < 512 * 1024);
    assert!(wire_bytes < 512 * 1024, "actual serialized fixture bytes={wire_bytes}");
    let blob = storage.blob_store().put(payload.as_bytes()).unwrap();
    let msg = message(sid, MessageRole::User, PayloadRef::Blob { hash: blob.hash, bytes: blob.raw_bytes });
    storage.append_message(&msg).unwrap();

    // One ordinary provider item with a payload comfortably within the actual
    // contract limit must be readable without an internal second payload charge.
    // 512 KiB is MAX_RESPONSES_INPUT_BYTES in providers/src/responses.rs:14-18.
    let history = storage.bounded_history(sid, 100, 512 * 1024);
    assert!(history.is_ok(), "in-budget legacy blob history rejected: {history:?}; payload={} actual-json={wire_bytes} budget={}", payload.len(), 512 * 1024);
    let history = history.unwrap();
    assert_eq!(history.len(), 1);
    assert!(matches!(&history[0], opencode_rk_storage::HistoryItem::Message(m)
        if m.id == msg.id && m.role == MessageRole::User && m.body == msg.body));
}
