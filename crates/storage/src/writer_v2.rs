//! Bounded writers for the format-2 workspace schema.

use opencode_rk_contracts::{MessageId, MessageRole, PayloadRef, SessionId};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::StorageError;

const MAX_TITLE_BYTES: usize = 1024;
const MAX_INLINE_PAYLOAD_BYTES: usize = 8192;
const MAX_EVENT_KIND_BYTES: usize = 128;
const MAX_EVENT_PAYLOAD_BYTES: usize = 4096;
const MAX_SESSION_PAGE_SIZE: usize = 500;

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
        let inline_data = match &message.body {
            PayloadRef::Inline { text } if text.len() <= MAX_INLINE_PAYLOAD_BYTES => {
                text.as_bytes()
            }
            PayloadRef::Inline { .. } | PayloadRef::Blob { .. } => {
                return Err(StorageError::InlinePayloadTooLarge);
            }
        };
        let role = encode_role(message.role);
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
        transaction.execute(
            "INSERT INTO payloads (inline_data, raw_bytes, created_at_us) VALUES (?1, ?2, ?3)",
            params![inline_data, inline_data.len() as i64, message.created_at_us],
        )?;
        let payload_pk = transaction.last_insert_rowid();
        transaction.execute(
            "INSERT INTO message_parts (message_pk, ordinal, kind, payload_pk)
             VALUES (?1, 0, 0, ?2)",
            params![message_pk, payload_pk],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn append_outbox_event(
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
