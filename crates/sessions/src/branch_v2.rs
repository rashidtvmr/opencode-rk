//! Compatibility bridge from the legacy HTTP session boundary to format-2 forks.
//!
//! Legacy sessions remain authoritative for ordinary chats until the storage-v2
//! activation gate is complete. Fork children, however, are created by the real
//! [`ForkV2`] implementation in a sidecar format-2 workspace. A bounded shadow of
//! legacy history is synchronized only as input to that fork operation; shadow
//! sessions are never listed to clients. Fork children are then read/written through
//! [`SessionManager`] so their durable ancestry and shared-payload semantics stay v2.

use crate::{SessionError, SessionManager};
use chrono::Utc;
use opencode_rk_contracts::{
    AssistantActivity, MessageId, MessageRecord, MessageRole, PayloadRef, SessionId, SessionSummary,
};
use opencode_rk_storage::{
    fork_v2::{ForkV2, MAX_FORK_COPY_MESSAGES, MAX_FORK_DEPTH},
    writer_v2::MESSAGE_PART_REASONING_SUMMARY,
    NewMessage, NewSession, SchemaV2, StorageError, V2Writer,
};
use rusqlite::{params, OptionalExtension};
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ForkProvenance {
    pub parent_session_id: SessionId,
    pub fork_message_seq: u64,
    pub boundary_message_id: Option<MessageId>,
}

impl SessionManager {
    pub fn list_message_history_page(
        &self,
        session_id: SessionId,
        before: Option<MessageId>,
        limit: usize,
    ) -> Result<(Vec<MessageRecord>, Option<MessageId>), SessionError> {
        let conn = self.conn.lock().map_err(|_| SessionError::Poisoned)?;
        let limit = limit.clamp(1, 100);
        let before_seq = match before {
            Some(message_id) => conn
                .query_row(
                    "SELECT m.seq FROM messages m JOIN sessions s ON s.pk=m.session_pk
                     WHERE s.id=?1 AND m.id=?2",
                    params![
                        session_id.as_uuid().as_bytes().as_slice(),
                        message_id.as_uuid().as_bytes().as_slice(),
                    ],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?
                .ok_or(SessionError::HistoryCursorNotFound(message_id))?,
            None => i64::MAX,
        };
        let mut statement = conn.prepare(
            "SELECT m.id, m.seq, m.role, m.created_at_us, p.inline_data, p.blob_pk, p.raw_bytes, b.hash
             FROM messages m
             JOIN sessions s ON s.pk=m.session_pk
             JOIN message_parts mp ON mp.message_pk=m.pk AND mp.ordinal=0
             JOIN payloads p ON p.pk=mp.payload_pk
             LEFT JOIN blobs b ON b.pk=p.blob_pk
             WHERE s.id=?1 AND m.seq<?2
             ORDER BY m.seq DESC LIMIT ?3",
        )?;
        let rows = statement.query_map(
            params![
                session_id.as_uuid().as_bytes().as_slice(),
                before_seq,
                (limit + 1) as i64,
            ],
            |row| crate::decode_message_row(session_id, row),
        )?;
        let mut messages = rows.collect::<Result<Vec<_>, _>>()?;
        let has_more = messages.len() > limit;
        messages.truncate(limit);
        messages.reverse();
        let next_before = if has_more {
            messages.first().map(|message| message.id)
        } else {
            None
        };
        Ok((messages, next_before))
    }

    pub fn open_branch_workspace(path: &Path) -> Result<Self, SessionError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(StorageError::from)?;
        }
        let connection =
            if path.exists() && std::fs::metadata(path).map_err(StorageError::from)?.len() > 0 {
                SchemaV2::open_existing(path)?
            } else {
                let workspace_id = *SessionId::new().as_uuid().as_bytes();
                let cursor_epoch = *SessionId::new().as_uuid().as_bytes();
                SchemaV2::initialize_workspace(
                    path,
                    workspace_id,
                    cursor_epoch,
                    Utc::now().timestamp_micros(),
                )?
            };
        Ok(Self::new(connection))
    }

    pub fn is_fork_session(&self, id: SessionId) -> Result<bool, SessionError> {
        let conn = self.conn.lock().map_err(|_| SessionError::Poisoned)?;
        let forked: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM sessions WHERE id=?1 AND fork_parent_id IS NOT NULL",
                params![id.as_uuid().as_bytes().as_slice()],
                |row| row.get(0),
            )
            .optional()?;
        Ok(forked.is_some())
    }

    pub fn list_fork_sessions(
        &self,
        include_archived: bool,
        limit: usize,
    ) -> Result<Vec<SessionSummary>, SessionError> {
        let ids = {
            let conn = self.conn.lock().map_err(|_| SessionError::Poisoned)?;
            let sql = if include_archived {
                "SELECT id FROM sessions WHERE fork_parent_id IS NOT NULL ORDER BY updated_at_us DESC, pk DESC LIMIT ?1"
            } else {
                "SELECT id FROM sessions WHERE fork_parent_id IS NOT NULL AND state=0 ORDER BY updated_at_us DESC, pk DESC LIMIT ?1"
            };
            let mut statement = conn.prepare(sql)?;
            let rows = statement.query_map(params![limit.clamp(1, 500) as i64], |row| {
                row.get::<_, Vec<u8>>(0)
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        let mut sessions = Vec::with_capacity(ids.len());
        for bytes in ids {
            let uuid = uuid::Uuid::from_slice(&bytes).map_err(|_| rusqlite::Error::InvalidQuery)?;
            let id = SessionId::from_uuid(uuid);
            if let Some(session) = self.get_session(id)? {
                sessions.push(session);
            }
        }
        Ok(sessions)
    }

    pub fn synchronize_legacy_shadow(
        &self,
        parent: &SessionSummary,
        source: &[MessageRecord],
        activity: &[AssistantActivity],
    ) -> Result<(), SessionError> {
        if source.len() > MAX_FORK_COPY_MESSAGES {
            return Err(SessionError::ForkHistoryTooLarge);
        }
        let mut conn = self.conn.lock().map_err(|_| SessionError::Poisoned)?;
        let existing_parent: Option<(i64,)> = conn
            .query_row(
                "SELECT pk FROM sessions WHERE id=?1",
                params![parent.id.as_uuid().as_bytes().as_slice()],
                |row| Ok((row.get(0)?,)),
            )
            .optional()?;
        if existing_parent.is_none() {
            V2Writer::create_session(
                &mut conn,
                &NewSession {
                    id: parent.id,
                    title: parent.title.clone(),
                    created_at_us: parent.created_at.as_datetime().timestamp_micros(),
                    updated_at_us: parent.updated_at.as_datetime().timestamp_micros(),
                },
            )?;
        } else {
            let is_fork: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sessions WHERE id=?1 AND fork_parent_id IS NOT NULL",
                params![parent.id.as_uuid().as_bytes().as_slice()],
                |row| row.get(0),
            )?;
            if is_fork != 0 {
                return Err(SessionError::Contract(
                    "legacy shadow id collided with a format-2 fork session".to_owned(),
                ));
            }
            conn.execute(
                "UPDATE sessions SET title=?1, updated_at_us=?2 WHERE id=?3",
                params![
                    parent.title,
                    parent.updated_at.as_datetime().timestamp_micros(),
                    parent.id.as_uuid().as_bytes().as_slice(),
                ],
            )?;
        }

        let existing: Vec<(Vec<u8>, i64)> = {
            let mut statement = conn.prepare(
                "SELECT m.id,m.role FROM messages m JOIN sessions s ON s.pk=m.session_pk
                 WHERE s.id=?1 ORDER BY m.seq ASC",
            )?;
            let rows = statement
                .query_map(params![parent.id.as_uuid().as_bytes().as_slice()], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        for (index, (id_bytes, role)) in existing.iter().take(source.len()).enumerate() {
            let expected = &source[index];
            if id_bytes.as_slice() != expected.id.as_uuid().as_bytes()
                || *role != role_code(expected.role)
            {
                return Err(SessionError::Contract(
                    "format-2 legacy shadow diverged from its source session".to_owned(),
                ));
            }
        }
        if existing.len() >= source.len() {
            return Ok(());
        }
        for message in &source[existing.len()..] {
            if !matches!(message.body, PayloadRef::Inline { .. }) {
                return Err(SessionError::BranchPayloadUnsupported);
            }
            let candidate = NewMessage {
                id: message.id,
                session_id: parent.id,
                role: message.role,
                body: message.body.clone(),
                created_at_us: message.created_at.as_datetime().timestamp_micros(),
            };
            let summary = activity
                .iter()
                .find(|entry| entry.message_id == message.id)
                .map(|entry| entry.reasoning_summary.as_str());
            V2Writer::append_message_with_reasoning(&mut conn, &candidate, summary)?;
        }
        Ok(())
    }

    pub fn append_fork_text(
        &self,
        session_id: SessionId,
        role: MessageRole,
        text: String,
    ) -> Result<MessageRecord, SessionError> {
        let body =
            PayloadRef::inline(text).map_err(|error| SessionError::Contract(error.to_string()))?;
        let message = MessageRecord {
            id: MessageId::new(),
            session_id,
            role,
            body,
            created_at: opencode_rk_contracts::Timestamp::now(),
        };
        let mut conn = self.conn.lock().map_err(|_| SessionError::Poisoned)?;
        V2Writer::append_message(
            &mut conn,
            &NewMessage {
                id: message.id,
                session_id,
                role,
                body: message.body.clone(),
                created_at_us: message.created_at.as_datetime().timestamp_micros(),
            },
        )?;
        Ok(message)
    }

    pub fn append_fork_assistant_with_reasoning(
        &self,
        session_id: SessionId,
        text: String,
        reasoning_summary: Option<String>,
    ) -> Result<MessageRecord, SessionError> {
        let body =
            PayloadRef::inline(text).map_err(|error| SessionError::Contract(error.to_string()))?;
        let message = MessageRecord {
            id: MessageId::new(),
            session_id,
            role: MessageRole::Assistant,
            body,
            created_at: opencode_rk_contracts::Timestamp::now(),
        };
        let mut conn = self.conn.lock().map_err(|_| SessionError::Poisoned)?;
        V2Writer::append_message_with_reasoning(
            &mut conn,
            &NewMessage {
                id: message.id,
                session_id,
                role: MessageRole::Assistant,
                body: message.body.clone(),
                created_at_us: message.created_at.as_datetime().timestamp_micros(),
            },
            reasoning_summary.as_deref(),
        )?;
        Ok(message)
    }

    pub fn list_assistant_activity(
        &self,
        session_id: SessionId,
        limit: usize,
    ) -> Result<Vec<AssistantActivity>, SessionError> {
        let conn = self.conn.lock().map_err(|_| SessionError::Poisoned)?;
        let mut statement = conn.prepare(
            "SELECT m.id,p.inline_data,p.blob_pk FROM messages m
             JOIN sessions s ON s.pk=m.session_pk
             JOIN message_parts mp ON mp.message_pk=m.pk AND mp.kind=?2
             JOIN payloads p ON p.pk=mp.payload_pk
             WHERE s.id=?1 AND m.role=2
             ORDER BY m.seq ASC LIMIT ?3",
        )?;
        let rows = statement.query_map(
            params![
                session_id.as_uuid().as_bytes().as_slice(),
                MESSAGE_PART_REASONING_SUMMARY,
                limit.clamp(1, 500) as i64,
            ],
            |row| {
                let id: Vec<u8> = row.get(0)?;
                let inline: Option<Vec<u8>> = row.get(1)?;
                let blob_pk: Option<i64> = row.get(2)?;
                if blob_pk.is_some() {
                    return Err(rusqlite::Error::InvalidQuery);
                }
                let reasoning_summary = String::from_utf8(inline.unwrap_or_default())
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                let message_id = MessageId::from_uuid(
                    uuid::Uuid::from_slice(&id).map_err(|_| rusqlite::Error::InvalidQuery)?,
                );
                Ok(AssistantActivity {
                    message_id,
                    reasoning_summary,
                    tool_calls: Vec::new(),
                    references: Vec::new(),
                })
            },
        )?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(SessionError::from)
    }

    pub fn retry_request(
        &self,
        session_id: SessionId,
        target_message_id: MessageId,
    ) -> Result<(MessageId, String), SessionError> {
        let conn = self.conn.lock().map_err(|_| SessionError::Poisoned)?;
        let target: Option<(i64, i64)> = conn
            .query_row(
                "SELECT m.seq,m.role FROM messages m JOIN sessions s ON s.pk=m.session_pk
                 WHERE s.id=?1 AND m.id=?2",
                params![
                    session_id.as_uuid().as_bytes().as_slice(),
                    target_message_id.as_uuid().as_bytes().as_slice(),
                ],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((target_seq, target_role)) = target else {
            return Err(SessionError::BranchMessageNotFound(target_message_id));
        };
        let request_seq = match target_role {
            1 => target_seq,
            2 => conn
                .query_row(
                    "SELECT MAX(m.seq) FROM messages m JOIN sessions s ON s.pk=m.session_pk
                     WHERE s.id=?1 AND m.role=1 AND m.seq<?2",
                    params![session_id.as_uuid().as_bytes().as_slice(), target_seq],
                    |row| row.get::<_, Option<i64>>(0),
                )?
                .ok_or(SessionError::InvalidBranchBoundary)?,
            _ => return Err(SessionError::InvalidBranchBoundary),
        };
        let request: Option<(Vec<u8>, Option<Vec<u8>>, Option<i64>)> = conn
            .query_row(
                "SELECT m.id,p.inline_data,p.blob_pk FROM messages m
                 JOIN sessions s ON s.pk=m.session_pk
                 JOIN message_parts mp ON mp.message_pk=m.pk AND mp.ordinal=0
                 JOIN payloads p ON p.pk=mp.payload_pk
                 WHERE s.id=?1 AND m.seq=?2 AND m.role=1",
                params![session_id.as_uuid().as_bytes().as_slice(), request_seq],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((request_id, inline_data, blob_pk)) = request else {
            return Err(SessionError::InvalidBranchBoundary);
        };
        if blob_pk.is_some() {
            return Err(SessionError::BranchPayloadUnsupported);
        }
        let text = String::from_utf8(inline_data.unwrap_or_default())
            .map_err(|_| SessionError::Contract("retry request is not valid UTF-8".to_owned()))?;
        let request_id = MessageId::from_uuid(
            uuid::Uuid::from_slice(&request_id).map_err(|_| rusqlite::Error::InvalidQuery)?,
        );
        Ok((request_id, text))
    }

    pub fn fork_before_user_message(
        &self,
        parent_session_id: SessionId,
        request_message_id: MessageId,
        title: &str,
    ) -> Result<(SessionSummary, ForkProvenance), SessionError> {
        let mut conn = self.conn.lock().map_err(|_| SessionError::Poisoned)?;
        let request_seq: Option<i64> = conn
            .query_row(
                "SELECT m.seq FROM messages m JOIN sessions s ON s.pk=m.session_pk
                 WHERE s.id=?1 AND m.id=?2 AND m.role=1",
                params![
                    parent_session_id.as_uuid().as_bytes().as_slice(),
                    request_message_id.as_uuid().as_bytes().as_slice(),
                ],
                |row| row.get(0),
            )
            .optional()?;
        let request_seq = request_seq.ok_or(SessionError::InvalidBranchBoundary)?;
        let through_seq = request_seq - 1;
        let in_range: i64 = conn.query_row(
            "SELECT COUNT(*) FROM messages m JOIN sessions s ON s.pk=m.session_pk
             WHERE s.id=?1 AND m.seq<=?2",
            params![
                parent_session_id.as_uuid().as_bytes().as_slice(),
                through_seq
            ],
            |row| row.get(0),
        )?;
        if in_range as usize > MAX_FORK_COPY_MESSAGES {
            return Err(SessionError::ForkHistoryTooLarge);
        }
        if fork_depth(&conn, parent_session_id)? >= MAX_FORK_DEPTH {
            return Err(SessionError::ForkDepthExceeded);
        }
        let child_id = SessionId::new();
        let now = Utc::now().timestamp_micros();
        ForkV2::fork_session(
            &mut conn,
            parent_session_id.as_uuid().as_bytes().as_slice(),
            through_seq,
            child_id,
            title,
            now,
        )?;
        drop(conn);
        let child = self
            .get_session(child_id)?
            .ok_or(SessionError::NotFound(child_id))?;
        Ok((
            child,
            ForkProvenance {
                parent_session_id,
                fork_message_seq: u64::try_from(through_seq)
                    .map_err(|_| SessionError::InvalidBranchBoundary)?,
                boundary_message_id: None,
            },
        ))
    }

    pub fn fork_from_message(
        &self,
        parent_session_id: SessionId,
        boundary_message_id: MessageId,
        title: &str,
    ) -> Result<(SessionSummary, ForkProvenance), SessionError> {
        let mut conn = self.conn.lock().map_err(|_| SessionError::Poisoned)?;
        let boundary: Option<(i64, i64)> = conn
            .query_row(
                "SELECT m.seq,m.role FROM messages m JOIN sessions s ON s.pk=m.session_pk
                 WHERE s.id=?1 AND m.id=?2",
                params![
                    parent_session_id.as_uuid().as_bytes().as_slice(),
                    boundary_message_id.as_uuid().as_bytes().as_slice(),
                ],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((through_seq, role)) = boundary else {
            return Err(SessionError::BranchMessageNotFound(boundary_message_id));
        };
        if role != role_code(MessageRole::User) && role != role_code(MessageRole::Assistant) {
            return Err(SessionError::InvalidBranchBoundary);
        }
        let in_range: i64 = conn.query_row(
            "SELECT COUNT(*) FROM messages m JOIN sessions s ON s.pk=m.session_pk
             WHERE s.id=?1 AND m.seq<=?2",
            params![
                parent_session_id.as_uuid().as_bytes().as_slice(),
                through_seq
            ],
            |row| row.get(0),
        )?;
        if in_range as usize > MAX_FORK_COPY_MESSAGES {
            return Err(SessionError::ForkHistoryTooLarge);
        }
        if fork_depth(&conn, parent_session_id)? >= MAX_FORK_DEPTH {
            return Err(SessionError::ForkDepthExceeded);
        }

        let child_id = SessionId::new();
        let now = Utc::now().timestamp_micros();
        ForkV2::fork_session(
            &mut conn,
            parent_session_id.as_uuid().as_bytes().as_slice(),
            through_seq,
            child_id,
            title,
            now,
        )?;
        drop(conn);
        let child = self
            .get_session(child_id)?
            .ok_or(SessionError::NotFound(child_id))?;
        Ok((
            child,
            ForkProvenance {
                parent_session_id,
                fork_message_seq: u64::try_from(through_seq)
                    .map_err(|_| SessionError::InvalidBranchBoundary)?,
                boundary_message_id: Some(boundary_message_id),
            },
        ))
    }

    pub fn fork_provenance(
        &self,
        child_session_id: SessionId,
    ) -> Result<Option<ForkProvenance>, SessionError> {
        let conn = self.conn.lock().map_err(|_| SessionError::Poisoned)?;
        let raw: Option<(Vec<u8>, i64)> = conn
            .query_row(
                "SELECT fork_parent_id,fork_message_seq FROM sessions
                 WHERE id=?1 AND fork_parent_id IS NOT NULL",
                params![child_session_id.as_uuid().as_bytes().as_slice()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((parent_bytes, sequence)) = raw else {
            return Ok(None);
        };
        let parent_uuid =
            uuid::Uuid::from_slice(&parent_bytes).map_err(|_| rusqlite::Error::InvalidQuery)?;
        let parent_session_id = SessionId::from_uuid(parent_uuid);
        let boundary: Option<Vec<u8>> = conn
            .query_row(
                "SELECT m.id FROM messages m JOIN sessions s ON s.pk=m.session_pk
                 WHERE s.id=?1 AND m.seq=?2",
                params![parent_session_id.as_uuid().as_bytes().as_slice(), sequence],
                |row| row.get(0),
            )
            .optional()?;
        let boundary_message_id = boundary
            .map(|bytes| {
                uuid::Uuid::from_slice(&bytes)
                    .map(MessageId::from_uuid)
                    .map_err(|_| rusqlite::Error::InvalidQuery)
            })
            .transpose()?;
        Ok(Some(ForkProvenance {
            parent_session_id,
            fork_message_seq: u64::try_from(sequence)
                .map_err(|_| SessionError::InvalidBranchBoundary)?,
            boundary_message_id,
        }))
    }
}

fn fork_depth(conn: &rusqlite::Connection, session_id: SessionId) -> Result<usize, SessionError> {
    let mut current = session_id.as_uuid().as_bytes().to_vec();
    let mut depth = 0usize;
    loop {
        let parent: Option<Vec<u8>> = conn
            .query_row(
                "SELECT fork_parent_id FROM sessions WHERE id=?1",
                params![current],
                |row| row.get::<_, Option<Vec<u8>>>(0),
            )
            .optional()?
            .flatten();
        let Some(parent) = parent else { break };
        depth += 1;
        current = parent;
        if depth >= MAX_FORK_DEPTH {
            break;
        }
    }
    Ok(depth)
}

fn role_code(role: MessageRole) -> i64 {
    match role {
        MessageRole::System => 0,
        MessageRole::User => 1,
        MessageRole::Assistant => 2,
        MessageRole::Tool => 3,
    }
}
