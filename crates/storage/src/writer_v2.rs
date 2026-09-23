//! Bounded writers for the format-2 workspace schema.

use opencode_rk_contracts::{
    AssistantReference, AssistantToolCall, MAX_REASONING_SUMMARY_BYTES, MessageId, MessageRole,
    PayloadRef, SessionId,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use crate::StorageError;
use crate::quota_v2::{QuotaSnapshot, QuotaV2, QuotaV2Error};
use crate::validate_assistant_activity;

const MAX_TITLE_BYTES: usize = 1024;
const MAX_INLINE_PAYLOAD_BYTES: usize = 8192;
const MAX_EVENT_KIND_BYTES: usize = 128;
const MAX_EVENT_PAYLOAD_BYTES: usize = 4096;
const MAX_SESSION_PAGE_SIZE: usize = 500;
pub const MESSAGE_PART_TEXT: i64 = 0;
pub const MESSAGE_PART_REASONING_SUMMARY: i64 = 1;
pub const MESSAGE_PART_TOOL_CALLS: i64 = 2;
pub const MESSAGE_PART_REFERENCES: i64 = 3;

pub struct NewSession {
    pub id: SessionId,
    pub title: String,
    pub created_at_us: i64,
    pub updated_at_us: i64,
}

pub struct NewMessage {
    pub id: MessageId,
    pub session_id: SessionId,
    pub role: MessageRole,
    pub body: PayloadRef,
    pub created_at_us: i64,
}

pub struct V2Writer;

/// Admission budget for the quota-gated append paths. Zero means "admit
/// nothing" for that resource (any measured usage exceeds a zero budget).
pub struct QuotaBudget {
    pub max_db_bytes: i64,
    pub max_wal_bytes: i64,
}

impl V2Writer {
    pub fn create_session(
        connection: &mut Connection,
        session: &NewSession,
    ) -> Result<(), StorageError> {
        if session.title.len() > MAX_TITLE_BYTES {
            return Err(invalid_input());
        }

        connection.execute(
            "INSERT INTO sessions (id, title, created_at_us, updated_at_us) VALUES (?1, ?2, ?3, ?4)",
            params![
                session.id.as_uuid().as_bytes().as_slice(),
                &session.title,
                session.created_at_us,
                session.updated_at_us,
            ],
        )?;
        Ok(())
    }

    pub fn append_message(
        connection: &mut Connection,
        message: &NewMessage,
    ) -> Result<(), StorageError> {
        insert_message(connection, message, None)
    }

    pub fn append_message_with_reasoning(
        connection: &mut Connection,
        message: &NewMessage,
        reasoning_summary: Option<&str>,
    ) -> Result<(), StorageError> {
        insert_message(connection, message, reasoning_summary)
    }

    pub fn append_message_with_activity(
        connection: &mut Connection,
        message: &NewMessage,
        reasoning_summary: Option<&str>,
        tool_calls: &[AssistantToolCall],
        references: &[AssistantReference],
    ) -> Result<(), StorageError> {
        insert_message_with_activity(
            connection,
            message,
            reasoning_summary,
            tool_calls,
            references,
        )
    }

    pub fn append_outbox_event(
        connection: &mut Connection,
        session_id: SessionId,
        kind: &str,
        payload_json: &str,
    ) -> Result<(), StorageError> {
        insert_outbox_event(connection, session_id, kind, payload_json)
    }

    pub fn list_recent_sessions(
        connection: &Connection,
        state: u8,
        before: Option<(i64, i64)>,
        limit: usize,
    ) -> Result<Vec<(i64, i64)>, StorageError> {
        let limit = limit.clamp(1, MAX_SESSION_PAGE_SIZE) as i64;
        let mut rows = Vec::with_capacity(limit as usize);
        match before {
            Some((updated_at_us, pk)) => {
                let mut statement = connection.prepare(
                    "SELECT updated_at_us, pk FROM sessions
                     WHERE state=?1 AND (updated_at_us, pk) < (?2, ?3)
                     ORDER BY updated_at_us DESC, pk DESC LIMIT ?4",
                )?;
                let result = statement
                    .query_map(params![i64::from(state), updated_at_us, pk, limit], |row| {
                        Ok((row.get(0)?, row.get(1)?))
                    })?;
                for row in result {
                    rows.push(row?);
                }
            }
            None => {
                let mut statement = connection.prepare(
                    "SELECT updated_at_us, pk FROM sessions
                     WHERE state=?1
                     ORDER BY updated_at_us DESC, pk DESC LIMIT ?2",
                )?;
                let result = statement.query_map(params![i64::from(state), limit], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })?;
                for row in result {
                    rows.push(row?);
                }
            }
        }
        Ok(rows)
    }
}

fn insert_message(
    connection: &mut Connection,
    message: &NewMessage,
    reasoning_summary: Option<&str>,
) -> Result<(), StorageError> {
    insert_message_with_activity(connection, message, reasoning_summary, &[], &[])
}

fn insert_message_with_activity(
    connection: &mut Connection,
    message: &NewMessage,
    reasoning_summary: Option<&str>,
    tool_calls: &[AssistantToolCall],
    references: &[AssistantReference],
) -> Result<(), StorageError> {
    // Validate all inputs before any side effect.
    let inline_data = match &message.body {
        PayloadRef::Inline { text } if text.len() <= MAX_INLINE_PAYLOAD_BYTES => text.as_bytes(),
        PayloadRef::Inline { .. } | PayloadRef::Blob { .. } => {
            return Err(StorageError::InlinePayloadTooLarge);
        }
    };
    let role = encode_role(message.role);
    if let Some(summary) = reasoning_summary {
        if message.role != MessageRole::Assistant
            || summary.as_bytes().len() > MAX_REASONING_SUMMARY_BYTES
        {
            return Err(StorageError::InlinePayloadTooLarge);
        }
    }
    validate_assistant_activity(tool_calls, references)?;
    let tool_calls_json =
        serde_json::to_string(tool_calls).map_err(|_| StorageError::InvalidAssistantActivity)?;
    let references_json =
        serde_json::to_string(references).map_err(|_| StorageError::InvalidAssistantActivity)?;

    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let session = transaction
        .query_row(
            "SELECT pk, next_message_seq FROM sessions WHERE id=?1",
            params![message.session_id.as_uuid().as_bytes().as_slice()],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?;
    let Some((session_pk, sequence)) = session else {
        return Err(StorageError::SessionNotFound(message.session_id));
    };
    let next_sequence = sequence.checked_add(1).ok_or_else(invalid_input)?;

    transaction.execute(
        "UPDATE sessions SET next_message_seq=?1, updated_at_us=?2 WHERE pk=?3",
        params![next_sequence, message.created_at_us, session_pk],
    )?;
    transaction.execute(
        "INSERT INTO messages
         (id, session_pk, seq, role, status, created_at_us, completed_at_us)
         VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)",
        params![
            message.id.as_uuid().as_bytes().as_slice(),
            session_pk,
            sequence,
            role,
            message.created_at_us,
        ],
    )?;
    let message_pk = transaction.last_insert_rowid();

    // Text part (ordinal 0): the message body.
    transaction.execute(
        "INSERT INTO payloads (inline_data, raw_bytes, created_at_us) VALUES (?1, ?2, ?3)",
        params![inline_data, inline_data.len() as i64, message.created_at_us],
    )?;
    let text_payload_pk = transaction.last_insert_rowid();
    transaction.execute(
        "INSERT INTO message_parts (message_pk, ordinal, kind, payload_pk)
         VALUES (?1, 0, ?2, ?3)",
        params![message_pk, MESSAGE_PART_TEXT, text_payload_pk],
    )?;

    // Reasoning summary part (ordinal 1, kind 1).
    let mut ordinal: i64 = 1;
    if let Some(summary) = reasoning_summary.filter(|summary| !summary.is_empty()) {
        transaction.execute(
            "INSERT INTO payloads (inline_data, raw_bytes, created_at_us) VALUES (?1, ?2, ?3)",
            params![
                summary.as_bytes(),
                summary.len() as i64,
                message.created_at_us
            ],
        )?;
        let summary_payload_pk = transaction.last_insert_rowid();
        transaction.execute(
            "INSERT INTO message_parts (message_pk, ordinal, kind, payload_pk, mime, name)
             VALUES (?1, ?2, ?3, ?4, 'text/plain; charset=utf-8', 'reasoning_summary')",
            params![
                message_pk,
                ordinal,
                MESSAGE_PART_REASONING_SUMMARY,
                summary_payload_pk
            ],
        )?;
        ordinal += 1;
    }

    // Tool calls part (ordinal N, kind 2).
    if !tool_calls.is_empty() {
        transaction.execute(
            "INSERT INTO payloads (inline_data, raw_bytes, created_at_us) VALUES (?1, ?2, ?3)",
            params![
                &tool_calls_json,
                tool_calls_json.len() as i64,
                message.created_at_us
            ],
        )?;
        let tool_calls_payload_pk = transaction.last_insert_rowid();
        transaction.execute(
            "INSERT INTO message_parts (message_pk, ordinal, kind, payload_pk, mime, name)
             VALUES (?1, ?2, ?3, ?4, 'application/json', 'tool_calls')",
            params![
                message_pk,
                ordinal,
                MESSAGE_PART_TOOL_CALLS,
                tool_calls_payload_pk
            ],
        )?;
        ordinal += 1;
    }

    // References part (ordinal N, kind 3).
    if !references.is_empty() {
        transaction.execute(
            "INSERT INTO payloads (inline_data, raw_bytes, created_at_us) VALUES (?1, ?2, ?3)",
            params![
                &references_json,
                references_json.len() as i64,
                message.created_at_us
            ],
        )?;
        let references_payload_pk = transaction.last_insert_rowid();
        transaction.execute(
            "INSERT INTO message_parts (message_pk, ordinal, kind, payload_pk, mime, name)
             VALUES (?1, ?2, ?3, ?4, 'application/json', 'references')",
            params![
                message_pk,
                ordinal,
                MESSAGE_PART_REFERENCES,
                references_payload_pk
            ],
        )?;
    }

    transaction.commit()?;
    Ok(())
}

/// Quota-gated append. Measures the live DB/WAL snapshot, admits under
/// the budget, and only on admit proceeds with the same insert as
/// `append_message`. On a rejected limit the error names which resource hit it
/// and nothing is written.
pub fn append_message_checked(
    connection: &mut Connection,
    message: &NewMessage,
    budget: &QuotaBudget,
) -> Result<(), StorageError> {
    let snapshot = QuotaV2::measure(connection)?;
    admit_checked(connection, &snapshot, budget)?;
    insert_message(connection, message, None)
}

/// Quota-gated outbox append, mirroring `append_outbox_event` after admission.
pub fn append_outbox_event_checked(
    connection: &mut Connection,
    session_id: SessionId,
    kind: &str,
    payload_json: &str,
    budget: &QuotaBudget,
) -> Result<(), StorageError> {
    let snapshot = QuotaV2::measure(connection)?;
    admit_checked(connection, &snapshot, budget)?;
    insert_outbox_event(connection, session_id, kind, payload_json)
}

fn admit_checked(
    connection: &Connection,
    snapshot: &QuotaSnapshot,
    budget: &QuotaBudget,
) -> Result<(), StorageError> {
    match QuotaV2::admit(
        connection,
        snapshot,
        budget.max_db_bytes,
        budget.max_wal_bytes,
    ) {
        Ok(()) => Ok(()),
        Err(QuotaV2Error::DbBytes(n)) => Err(StorageError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("db budget exceeded: {n} bytes"),
        ))),
        Err(QuotaV2Error::WalBytes(n)) => Err(StorageError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("wal budget exceeded: {n} bytes"),
        ))),
    }
}

fn insert_outbox_event(
    connection: &mut Connection,
    session_id: SessionId,
    kind: &str,
    payload_json: &str,
) -> Result<(), StorageError> {
    if !(1..=MAX_EVENT_KIND_BYTES).contains(&kind.len()) {
        return Err(invalid_input());
    }
    if payload_json.len() > MAX_EVENT_PAYLOAD_BYTES {
        return Err(StorageError::EventPayloadTooLarge);
    }
    serde_json::from_str::<serde_json::Value>(payload_json).map_err(|_| invalid_input())?;

    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let sequence = allocate_event_sequence(&transaction)?;
    transaction.execute(
        "INSERT INTO event_outbox (seq, session_id, kind, payload_json, created_at_us)
         VALUES (?1, ?2, ?3, ?4, 0)",
        params![
            sequence,
            session_id.as_uuid().as_bytes().as_slice(),
            kind,
            payload_json,
        ],
    )?;
    transaction.commit()?;
    Ok(())
}

fn allocate_event_sequence(transaction: &Transaction<'_>) -> Result<i64, StorageError> {
    match transaction.query_row(
        "UPDATE workspace_state
         SET event_head_seq=event_head_seq+1
         WHERE id=1
         RETURNING event_head_seq",
        [],
        |row| row.get(0),
    ) {
        Ok(sequence) => Ok(sequence),
        Err(_) => {
            let current: i64 = transaction.query_row(
                "SELECT event_head_seq FROM workspace_state WHERE id=1",
                [],
                |row| row.get(0),
            )?;
            let sequence = current.checked_add(1).ok_or_else(invalid_input)?;
            let changed = transaction.execute(
                "UPDATE workspace_state SET event_head_seq=?1 WHERE id=1",
                [sequence],
            )?;
            if changed != 1 {
                return Err(invalid_input());
            }
            Ok(sequence)
        }
    }
}

fn encode_role(role: MessageRole) -> i64 {
    match role {
        MessageRole::System => 0,
        MessageRole::User => 1,
        MessageRole::Assistant => 2,
        MessageRole::Tool => 3,
    }
}

fn invalid_input() -> StorageError {
    StorageError::Sqlite(rusqlite::Error::InvalidQuery)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencode_rk_contracts::{MessageId, MessageRole, PayloadRef, SessionId};
    use std::path::Path;
    use tempfile::tempdir;

    fn workspace(path: &Path) -> Connection {
        crate::schema_v2::SchemaV2::initialize_workspace(path, [1_u8; 16], [2_u8; 16], 10).unwrap()
    }

    fn msg(session: SessionId, body: &str, ts: i64) -> NewMessage {
        NewMessage {
            id: MessageId::new(),
            session_id: session,
            role: MessageRole::User,
            body: PayloadRef::Inline {
                text: body.to_owned(),
            },
            created_at_us: ts,
        }
    }

    fn create_session(conn: &mut Connection, id: SessionId) {
        V2Writer::create_session(
            conn,
            &NewSession {
                id,
                title: "t".to_owned(),
                created_at_us: 100,
                updated_at_us: 100,
            },
        )
        .unwrap();
    }

    fn message_rows(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
            .unwrap()
    }

    fn payload_rows(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM payloads", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn checked_append_generous_budget_inserts_same_rows_as_plain() {
        let (dir_a, dir_b) = (tempdir().unwrap(), tempdir().unwrap());
        let mut plain = workspace(&dir_a.path().join("w.db"));
        let mut checked = workspace(&dir_b.path().join("w.db"));
        let (sid_a, sid_b) = (SessionId::new(), SessionId::new());
        create_session(&mut plain, sid_a);
        create_session(&mut checked, sid_b);

        V2Writer::append_message(&mut plain, &msg(sid_a, "hello", 1000)).unwrap();
        let budget = QuotaBudget {
            max_db_bytes: i64::MAX,
            max_wal_bytes: i64::MAX,
        };
        append_message_checked(&mut checked, &msg(sid_b, "hello", 1000), &budget).unwrap();

        assert_eq!(message_rows(&plain), message_rows(&checked));
        assert_eq!(payload_rows(&plain), payload_rows(&checked));
    }

    #[test]
    fn checked_append_zero_db_budget_rejects_and_writes_nothing() {
        let dir = tempdir().unwrap();
        let mut conn = workspace(&dir.path().join("w.db"));
        let sid = SessionId::new();
        create_session(&mut conn, sid);

        let before = message_rows(&conn);
        let budget = QuotaBudget {
            max_db_bytes: 0,
            max_wal_bytes: i64::MAX,
        };
        let err = append_message_checked(&mut conn, &msg(sid, "nope", 1000), &budget).unwrap_err();
        assert!(matches!(err, StorageError::Io(_)));
        assert_eq!(message_rows(&conn), before);
    }
}
