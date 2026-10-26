//! Web history pages hydrate the typed tool rows belonging to that page,
//! while the legacy flat-message inventory keeps its first-N semantics.
use opencode_rk_contracts::{
    MessageId, MessageRecord, MessageRole, PayloadRef, SessionId, SessionState, SessionSummary,
    Timestamp,
};
use opencode_rk_storage::{Storage, StoragePaths, ToolCallAdmission, ToolRound};
use tempfile::tempdir;

fn message(session: SessionId, role: MessageRole, body: &str) -> MessageRecord {
    MessageRecord {
        id: MessageId::new(),
        session_id: session,
        role,
        body: PayloadRef::inline(body).unwrap(),
        created_at: Timestamp::now(),
    }
}

#[test]
fn hydrates_each_page_including_pages_beyond_initial_limit() {
    let dir = tempdir().unwrap();
    let storage = Storage::open(StoragePaths::under(dir.path().join("db"))).unwrap();
    let session = SessionId::new();
    let now = Timestamp::now();
    storage
        .create_session(&SessionSummary {
            id: session,
            title: "typed tools".into(),
            state: SessionState::Active,
            created_at: now,
            updated_at: now,
            archived_at: None,
        })
        .unwrap();

    for index in 0..4u32 {
        let prompt = message(session, MessageRole::User, &format!("turn {index}"));
        storage.append_message(&prompt).unwrap();
        let display = message(session, MessageRole::Tool, "[bash] pending");
        let round = ToolRound {
            round_id: format!("round-{index}"),
            session_id: session,
            turn_message_id: prompt.id,
            round_ordinal: 0,
            expected_pairs: 1,
        };
        storage
            .admit_tool_round(
                &round,
                &[ToolCallAdmission {
                    call_id: format!("call-{index}"),
                    name: "bash".into(),
                    arguments: format!("{{\"command\":\"echo {index}\"}}"),
                    message: display.clone(),
                }],
            )
            .unwrap();
        storage
            .append_tool_output(&round, 0, &format!("output {index}"), display.id)
            .unwrap();
    }

    // Oldest-first is intentional for the legacy /messages endpoint.
    let original = storage.display_tool_history(session, 2).unwrap();
    assert_eq!(original.len(), 2);
    assert!(original.iter().all(|row| row.call_id == "call-0"));

    let (recent, before) = storage.list_message_history_page(session, None, 2).unwrap();
    assert_eq!(recent.len(), 2);
    let first_page = storage.display_tool_history_page(session, None, 2).unwrap();
    assert_eq!(first_page.len(), 2);
    assert!(first_page.iter().all(|row| row.call_id == "call-3"));

    let cursor = before.expect("older page exists");
    let second_page = storage
        .display_tool_history_page(session, Some(cursor), 2)
        .unwrap();
    assert_eq!(second_page.len(), 2);
    assert!(second_page.iter().all(|row| row.call_id == "call-2"));
    assert_ne!(first_page[0].message_id, second_page[0].message_id);
}
