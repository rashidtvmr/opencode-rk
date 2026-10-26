//! Native global search returns bounded session/title and message matches.
use opencode_rk_contracts::{
    MessageId, MessageRecord, MessageRole, PayloadRef, SessionId, SessionState, SessionSummary,
    Timestamp,
};
use opencode_rk_storage::{Storage, StoragePaths};
use tempfile::tempdir;

#[test]
fn searches_local_titles_and_recent_inline_messages_without_tool_output() {
    let tmp = tempdir().unwrap();
    let storage = Storage::open(StoragePaths::under(tmp.path().join("db"))).unwrap();
    let id = SessionId::new();
    let now = Timestamp::now();
    storage
        .create_session(&SessionSummary {
            id,
            title: "Feature % literal search".into(),
            state: SessionState::Active,
            created_at: now,
            updated_at: now,
            archived_at: None,
        })
        .unwrap();
    let user = MessageRecord {
        id: MessageId::new(),
        session_id: id,
        role: MessageRole::User,
        body: PayloadRef::inline("Inspect time-series dashboard").unwrap(),
        created_at: Timestamp::now(),
    };
    storage.append_message(&user).unwrap();
    let tool = MessageRecord {
        id: MessageId::new(),
        session_id: id,
        role: MessageRole::Tool,
        body: PayloadRef::inline("PRIVATE_TOOL_ONLY").unwrap(),
        created_at: Timestamp::now(),
    };
    storage.append_message(&tool).unwrap();
    let found = storage.search_content("time-series").unwrap();
    assert_eq!(found.messages.len(), 1);
    assert_eq!(found.messages[0].session_id, id.to_string());
    assert_eq!(found.messages[0].role, "user");
    assert!(found.messages[0].snippet.contains("time-series"));
    assert!(storage
        .search_content("PRIVATE_TOOL_ONLY")
        .unwrap()
        .messages
        .is_empty());
    assert!(storage
        .search_content("% literal")
        .unwrap()
        .sessions
        .iter()
        .any(|hit| hit.id == id.to_string()));
    assert!(storage.search_content("\nnewlines").is_err());
    assert!(storage.search_content("x").is_err());
}
