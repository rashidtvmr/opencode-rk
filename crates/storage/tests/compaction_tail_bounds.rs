//! Checkpoint tails are bounded independently of their preserved prefix.
use opencode_rk_contracts::{MessageId, MessageRecord, MessageRole, PayloadRef, SessionId, SessionState, SessionSummary, Timestamp};
use opencode_rk_storage::{HistoryItem, Storage, StorageError, ToolPair};

fn durable_now() -> Timestamp {
    let serialized = Timestamp::now().to_string();
    let parsed = chrono::DateTime::parse_from_rfc3339(&serialized).unwrap();
    Timestamp::from_datetime(parsed.with_timezone(&chrono::Utc))
}

fn session(storage: &Storage) -> SessionId {
    let id = SessionId::new();
    let now = Timestamp::now();
    storage.create_session(&SessionSummary { id, title:"Compaction tail".into(), state:SessionState::Active, created_at:now, updated_at:now, archived_at:None }).unwrap();
    id
}

fn message(storage: &Storage, session_id: SessionId, role: MessageRole, text: &str) -> MessageRecord {
    let message = MessageRecord { id:MessageId::new(), session_id, role, body:PayloadRef::inline(text).unwrap(), created_at:durable_now() };
    storage.append_message(&message).unwrap();
    message
}

fn pair(storage: &Storage, session_id: SessionId, turn: MessageId, round: &str, call: &str) {
    let round = storage.begin_tool_round(session_id, turn, round, 0, 1).unwrap();
    storage.append_tool_pair(&round, 0, &ToolPair {
        call_id:call.into(), name:"fixture.tool".into(), input:PayloadRef::inline("{}").unwrap(),
        output:PayloadRef::inline("tail result").unwrap(),
        message:MessageRecord { id:MessageId::new(), session_id, role:MessageRole::Tool, body:PayloadRef::inline("tool display").unwrap(), created_at:Timestamp::now() },
    }).unwrap();
}

#[test]
fn large_preserved_prefix_does_not_consume_tail_count_or_byte_budget() {
    for (count, text) in [(150, "old".to_owned()), (2, "old".repeat(3000))] {
        let directory = tempfile::tempdir().unwrap();
        let storage = Storage::open_in_memory(directory.path().join("blobs")).unwrap();
        let id = session(&storage);
        let mut boundary = None;
        for _ in 0..count { boundary = Some(message(&storage, id, MessageRole::Assistant, &text).id); }
        let first = message(&storage, id, MessageRole::User, "new request");
        let second = message(&storage, id, MessageRole::Assistant, "new answer");
        let tail = storage.bounded_history_after(id, boundary.unwrap(), 2, 1024).expect("only the post-checkpoint tail consumes its budget");
        assert_eq!(tail, vec![HistoryItem::Message(first), HistoryItem::Message(second)]);
    }
}

#[test]
fn completed_prefix_rounds_do_not_fill_tail_headers_and_tail_pair_order_is_preserved() {
    let directory = tempfile::tempdir().unwrap();
    let storage = Storage::open_in_memory(directory.path().join("blobs")).unwrap();
    let id = session(&storage);
    for index in 0..10 {
        let prompt = message(&storage, id, MessageRole::User, "old request");
        pair(&storage, id, prompt.id, &format!("prefix-round-{index}"), &format!("prefix-call-{index}"));
    }
    let boundary = message(&storage, id, MessageRole::Assistant, "checkpoint boundary");
    let prompt = message(&storage, id, MessageRole::User, "new request");
    pair(&storage, id, prompt.id, "tail-round", "tail-call");
    let answer = message(&storage, id, MessageRole::Assistant, "new answer");
    let tail = storage.bounded_history_after(id, boundary.id, 4, 4096).expect("prefix round headers must be filtered before admission");
    assert_eq!(tail.len(), 4);
    assert_eq!(tail[0], HistoryItem::Message(prompt));
    assert!(matches!(&tail[1], HistoryItem::Tool(item) if item.kind == "call" && item.call_id == "tail-call" && item.payload == "{}"));
    assert!(matches!(&tail[2], HistoryItem::Tool(item) if item.kind == "output" && item.call_id == "tail-call" && item.payload == "tail result"));
    assert_eq!(tail[3], HistoryItem::Message(answer));
    let foreign = session(&storage);
    assert!(storage.bounded_history_after(foreign, boundary.id, 4, 4096).is_err());
    let pending = message(&storage, id, MessageRole::User, "pending tail");
    storage.begin_tool_round(id, pending.id, "pending-tail", 0, 1).unwrap();
    assert!(matches!(storage.bounded_history_after(id, boundary.id, 8, 4096), Err(StorageError::TypedHistoryIncomplete)));
}
