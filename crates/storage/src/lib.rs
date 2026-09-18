//! Embedded persistence with bounded relational state and content-addressed blobs.
#![forbid(unsafe_code)]
pub mod admission_v2;
pub mod approvals_v2;
pub mod catalog_v2;
pub mod execution_v2;
pub mod facade;
pub mod fork_v2;
pub mod gc_v2;
pub mod import_v2;
pub mod migrations;
pub mod quota_v2;
pub mod retention_v2;
pub mod backup_v2;
pub mod rollout_v2;
pub mod schema_v2;
pub mod snapshot_v2;
pub mod writer_v2;
pub use admission_v2::AdmissionV2;
pub use approvals_v2::ApprovalsV2;
pub use catalog_v2::CatalogV2;
use chrono::{DateTime, Utc};
pub use execution_v2::ExecV2;
pub use gc_v2::GcV2;
pub use import_v2::ImportV2;
use opencode_rk_contracts::{
    ArtifactDocument, ArtifactId, ArtifactKind, ArtifactSummary, ArtifactVersion,
    AssistantActivity, AttachmentId, DraftAttachment, MessageId, MessageRecord, MessageRole,
    PayloadRef, SessionId, SessionState, SessionSummary, Timestamp, MAX_ARTIFACTS_PER_SESSION,
    MAX_ARTIFACT_CONTENT_BYTES, MAX_ARTIFACT_LANGUAGE_BYTES, MAX_ARTIFACT_TITLE_BYTES,
    MAX_ARTIFACT_TOTAL_BYTES, MAX_ARTIFACT_VERSIONS, MAX_ATTACHMENT_MIME_BYTES,
    MAX_ATTACHMENT_NAME_BYTES, MAX_DRAFT_ATTACHMENTS, MAX_DRAFT_ATTACHMENT_BYTES,
    MAX_INLINE_PAYLOAD_BYTES, MAX_REASONING_SUMMARY_BYTES,
};
pub use quota_v2::QuotaV2;
pub use retention_v2::RetentionV2;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
pub use schema_v2::SchemaV2;
pub use snapshot_v2::SnapshotV2;
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    str::FromStr,
    sync::Mutex,
};
use thiserror::Error;
pub use writer_v2::{NewMessage, NewSession, V2Writer};
pub const DEFAULT_MAX_EVENTS_PER_SESSION: usize = 10_000;
pub const MAX_EVENT_PAYLOAD_BYTES: usize = 64 * 1024;
pub const BLOB_COMPRESSION_LEVEL: i32 = 3;
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("session not found: {0}")]
    SessionNotFound(SessionId),
    #[error("message not found: {0}")]
    MessageNotFound(MessageId),
    #[error("artifact not found: {0}")]
    ArtifactNotFound(ArtifactId),
    #[error("artifact content exceeds the supported size bound")]
    ArtifactContentTooLarge,
    #[error("artifact count, version history, or retained bytes exceeded the supported bound")]
    ArtifactLimitExceeded,
    #[error("artifact metadata is invalid")]
    InvalidArtifact,
    #[error("payload exceeds inline limit and must be stored as a blob")]
    InlinePayloadTooLarge,
    #[error("event payload exceeds {MAX_EVENT_PAYLOAD_BYTES} bytes")]
    EventPayloadTooLarge,
    #[error("blob hash is malformed")]
    InvalidBlobHash,
    #[error("blob content hash did not match requested hash")]
    BlobHashMismatch,
    #[error("storage mutex poisoned")]
    Poisoned,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoragePaths {
    pub root: PathBuf,
    pub database: PathBuf,
    pub blobs: PathBuf,
}
impl StoragePaths {
    #[must_use]
    pub fn under(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            database: root.join("state.db"),
            blobs: root.join("blobs"),
            root,
        }
    }
}
pub struct Storage {
    connection: Mutex<Connection>,
    blobs: BlobStore,
    max_events_per_session: usize,
}
impl Storage {
    pub fn open(paths: StoragePaths) -> Result<Self, StorageError> {
        fs::create_dir_all(&paths.root)?;
        fs::create_dir_all(&paths.blobs)?;
        let connection = Connection::open(paths.database)?;
        Self::configure(&connection)?;
        Self::migrate(&connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
            blobs: BlobStore::new(paths.blobs),
            max_events_per_session: DEFAULT_MAX_EVENTS_PER_SESSION,
        })
    }
    pub fn open_in_memory(blob_root: impl Into<PathBuf>) -> Result<Self, StorageError> {
        let connection = Connection::open_in_memory()?;
        Self::configure(&connection)?;
        Self::migrate(&connection)?;
        let blob_root = blob_root.into();
        fs::create_dir_all(&blob_root)?;
        Ok(Self {
            connection: Mutex::new(connection),
            blobs: BlobStore::new(blob_root),
            max_events_per_session: DEFAULT_MAX_EVENTS_PER_SESSION,
        })
    }
    #[must_use]
    pub fn blob_store(&self) -> &BlobStore {
        &self.blobs
    }
    pub fn create_session(&self, session: &SessionSummary) -> Result<(), StorageError> {
        session
            .validate()
            .map_err(|_| StorageError::Sqlite(rusqlite::Error::InvalidQuery))?;
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        connection.execute("INSERT INTO sessions (id,title,state,created_at,updated_at,archived_at) VALUES (?1,?2,?3,?4,?5,?6)",params![session.id.to_string(),session.title,encode_state(session.state),session.created_at.to_string(),session.updated_at.to_string(),session.archived_at.map(Timestamp::to_rfc3339)])?;
        Ok(())
    }
    pub fn get_session(&self, id: SessionId) -> Result<SessionSummary, StorageError> {
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        connection
            .query_row(
                "SELECT id,title,state,created_at,updated_at,archived_at FROM sessions WHERE id=?1",
                params![id.to_string()],
                decode_session,
            )
            .optional()?
            .ok_or(StorageError::SessionNotFound(id))
    }
    pub fn list_sessions(
        &self,
        include_archived: bool,
    ) -> Result<Vec<SessionSummary>, StorageError> {
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let sql = if include_archived {
            "SELECT id,title,state,created_at,updated_at,archived_at FROM sessions ORDER BY updated_at DESC,id DESC"
        } else {
            "SELECT id,title,state,created_at,updated_at,archived_at FROM sessions WHERE state='active' ORDER BY updated_at DESC,id DESC"
        };
        let mut statement = connection.prepare(sql)?;
        let rows = statement.query_map([], decode_session)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }
    pub fn rename_session(
        &self,
        id: SessionId,
        title: &str,
        at: Timestamp,
    ) -> Result<(), StorageError> {
        if title.len() > opencode_rk_contracts::MAX_TITLE_BYTES {
            return Err(StorageError::Sqlite(rusqlite::Error::InvalidQuery));
        }
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let changed = connection.execute(
            "UPDATE sessions SET title=?1,updated_at=?2 WHERE id=?3",
            params![title, at.to_string(), id.to_string()],
        )?;
        if changed == 0 {
            return Err(StorageError::SessionNotFound(id));
        }
        Ok(())
    }
    pub fn archive_session(&self, id: SessionId, at: Timestamp) -> Result<(), StorageError> {
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let changed = connection.execute(
            "UPDATE sessions SET state='archived',archived_at=?1,updated_at=?1 WHERE id=?2",
            params![at.to_string(), id.to_string()],
        )?;
        if changed == 0 {
            return Err(StorageError::SessionNotFound(id));
        }
        Ok(())
    }
    pub fn append_message(&self, message: &MessageRecord) -> Result<(), StorageError> {
        self.append_message_with_reasoning(message, None)
    }
    pub fn append_message_with_reasoning(
        &self,
        message: &MessageRecord,
        reasoning_summary: Option<&str>,
    ) -> Result<(), StorageError> {
        let (inline_text, blob_hash, byte_len) = match &message.body {
            PayloadRef::Inline { text } => {
                if text.len() > MAX_INLINE_PAYLOAD_BYTES {
                    return Err(StorageError::InlinePayloadTooLarge);
                }
                (Some(text.as_str()), None, text.len() as u64)
            }
            PayloadRef::Blob { hash, bytes } => (None, Some(hash.as_str()), *bytes),
        };
        if let Some(summary) = reasoning_summary {
            if message.role != MessageRole::Assistant
                || summary.as_bytes().len() > MAX_REASONING_SUMMARY_BYTES
            {
                return Err(StorageError::InlinePayloadTooLarge);
            }
        }
        let mut connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("INSERT INTO messages (id,session_id,role,inline_text,blob_hash,byte_len,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",params![message.id.to_string(),message.session_id.to_string(),encode_role(message.role),inline_text,blob_hash,byte_len as i64,message.created_at.to_string()])?;
        if let Some(summary) = reasoning_summary.filter(|summary| !summary.is_empty()) {
            tx.execute(
                "INSERT INTO message_activity (message_id,reasoning_summary) VALUES (?1,?2)",
                params![message.id.to_string(), summary],
            )?;
        }
        tx.execute(
            "UPDATE sessions SET updated_at=?1 WHERE id=?2",
            params![
                message.created_at.to_string(),
                message.session_id.to_string()
            ],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn create_draft_attachment(
        &self,
        session_id: SessionId,
        name: &str,
        mime: &str,
        bytes: &[u8],
    ) -> Result<DraftAttachment, StorageError> {
        if bytes.is_empty() || bytes.len() > MAX_DRAFT_ATTACHMENT_BYTES {
            return Err(StorageError::InlinePayloadTooLarge);
        }
        if name.is_empty()
            || name.as_bytes().len() > MAX_ATTACHMENT_NAME_BYTES
            || mime.is_empty()
            || mime.as_bytes().len() > MAX_ATTACHMENT_MIME_BYTES
        {
            return Err(StorageError::Sqlite(rusqlite::Error::InvalidQuery));
        }
        self.get_session(session_id)?;
        let blob = self.blobs.put(bytes)?;
        let created_at = parse_timestamp(Timestamp::now().to_string())?;
        let attachment = DraftAttachment {
            id: AttachmentId::new(),
            session_id,
            name: name.to_owned(),
            mime: mime.to_owned(),
            hash: blob.hash,
            bytes: blob.raw_bytes,
            created_at,
        };
        let mut connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM draft_attachments WHERE session_id=?1",
            params![session_id.to_string()],
            |row| row.get(0),
        )?;
        if count as usize >= MAX_DRAFT_ATTACHMENTS {
            return Err(StorageError::Sqlite(rusqlite::Error::InvalidQuery));
        }
        tx.execute(
            "INSERT INTO draft_attachments (id,session_id,name,mime,blob_hash,byte_len,created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                attachment.id.to_string(),
                attachment.session_id.to_string(),
                attachment.name,
                attachment.mime,
                attachment.hash,
                attachment.bytes as i64,
                attachment.created_at.to_string(),
            ],
        )?;
        tx.commit()?;
        Ok(attachment)
    }
    pub fn list_draft_attachments(
        &self,
        session_id: SessionId,
    ) -> Result<Vec<DraftAttachment>, StorageError> {
        self.get_session(session_id)?;
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let mut statement = connection.prepare(
            "SELECT id,name,mime,blob_hash,byte_len,created_at FROM draft_attachments
             WHERE session_id=?1 ORDER BY rowid ASC LIMIT ?2",
        )?;
        let rows = statement.query_map(
            params![session_id.to_string(), MAX_DRAFT_ATTACHMENTS as i64],
            |row| {
                let id: String = row.get(0)?;
                let created_at: String = row.get(5)?;
                Ok(DraftAttachment {
                    id: AttachmentId::from_str(&id).map_err(|_| rusqlite::Error::InvalidQuery)?,
                    session_id,
                    name: row.get(1)?,
                    mime: row.get(2)?,
                    hash: row.get(3)?,
                    bytes: row.get::<_, i64>(4)?.max(0) as u64,
                    created_at: parse_timestamp(created_at)?,
                })
            },
        )?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }
    pub fn delete_draft_attachment(
        &self,
        session_id: SessionId,
        attachment_id: AttachmentId,
    ) -> Result<(), StorageError> {
        self.get_session(session_id)?;
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let changed = connection.execute(
            "DELETE FROM draft_attachments WHERE session_id=?1 AND id=?2",
            params![session_id.to_string(), attachment_id.to_string()],
        )?;
        if changed == 0 {
            return Err(StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows));
        }
        Ok(())
    }
    pub fn list_assistant_activity(
        &self,
        session_id: SessionId,
        limit: usize,
    ) -> Result<Vec<AssistantActivity>, StorageError> {
        let limit = limit.clamp(1, 500) as i64;
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let mut statement = connection.prepare(
            "SELECT m.id,a.reasoning_summary FROM messages m
             JOIN message_activity a ON a.message_id=m.id
             WHERE m.session_id=?1 AND m.role='assistant'
             ORDER BY m.rowid ASC LIMIT ?2",
        )?;
        let rows = statement.query_map(params![session_id.to_string(), limit], |row| {
            let id: String = row.get(0)?;
            let message_id = MessageId::from_str(&id).map_err(|_| rusqlite::Error::InvalidQuery)?;
            Ok(AssistantActivity {
                message_id,
                reasoning_summary: row.get(1)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }
    pub fn get_message(
        &self,
        session_id: SessionId,
        message_id: MessageId,
    ) -> Result<MessageRecord, StorageError> {
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        connection
            .query_row(
                "SELECT id,session_id,role,inline_text,blob_hash,byte_len,created_at
                 FROM messages WHERE session_id=?1 AND id=?2",
                params![session_id.to_string(), message_id.to_string()],
                decode_message,
            )
            .optional()?
            .ok_or(StorageError::MessageNotFound(message_id))
    }
    pub fn create_artifact(
        &self,
        session_id: SessionId,
        source_message_id: MessageId,
        kind: ArtifactKind,
        title: &str,
        language: Option<&str>,
        content: &str,
        at: Timestamp,
    ) -> Result<ArtifactDocument, StorageError> {
        validate_artifact_metadata(title, language)?;
        validate_artifact_content(content)?;
        let artifact_id = ArtifactId::new();
        let mut connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM artifacts WHERE session_id=?1",
            params![session_id.to_string()],
            |row| row.get(0),
        )?;
        if count as usize >= MAX_ARTIFACTS_PER_SESSION {
            return Err(StorageError::ArtifactLimitExceeded);
        }
        let at = at.to_string();
        tx.execute(
            "INSERT INTO artifacts
             (id,session_id,source_message_id,kind,title,language,current_version,created_at,updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,1,?7,?7)",
            params![
                artifact_id.to_string(),
                session_id.to_string(),
                source_message_id.to_string(),
                encode_artifact_kind(kind),
                title,
                language,
                at,
            ],
        )?;
        tx.execute(
            "INSERT INTO artifact_versions (artifact_id,version,content,byte_len,created_at)
             VALUES (?1,1,?2,?3,?4)",
            params![artifact_id.to_string(), content, content.len() as i64, at],
        )?;
        tx.commit()?;
        drop(connection);
        self.get_artifact(session_id, artifact_id)
    }
    pub fn list_artifacts(
        &self,
        session_id: SessionId,
        limit: usize,
    ) -> Result<Vec<ArtifactSummary>, StorageError> {
        let limit = limit.clamp(1, MAX_ARTIFACTS_PER_SESSION) as i64;
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let mut statement = connection.prepare(
            "SELECT id,session_id,source_message_id,kind,title,language,current_version,created_at,updated_at
             FROM artifacts WHERE session_id=?1 ORDER BY updated_at DESC,id DESC LIMIT ?2",
        )?;
        let rows = statement.query_map(
            params![session_id.to_string(), limit],
            decode_artifact_summary,
        )?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }
    pub fn get_artifact(
        &self,
        session_id: SessionId,
        artifact_id: ArtifactId,
    ) -> Result<ArtifactDocument, StorageError> {
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let summary = connection
            .query_row(
                "SELECT id,session_id,source_message_id,kind,title,language,current_version,created_at,updated_at
                 FROM artifacts WHERE session_id=?1 AND id=?2",
                params![session_id.to_string(), artifact_id.to_string()],
                decode_artifact_summary,
            )
            .optional()?
            .ok_or(StorageError::ArtifactNotFound(artifact_id))?;
        let content: String = connection.query_row(
            "SELECT content FROM artifact_versions WHERE artifact_id=?1 AND version=?2",
            params![artifact_id.to_string(), i64::from(summary.current_version)],
            |row| row.get(0),
        )?;
        let mut statement = connection.prepare(
            "SELECT version,byte_len,created_at FROM artifact_versions
             WHERE artifact_id=?1 ORDER BY version ASC LIMIT ?2",
        )?;
        let rows = statement.query_map(
            params![artifact_id.to_string(), MAX_ARTIFACT_VERSIONS as i64],
            |row| {
                let version = row.get::<_, i64>(0)?;
                let bytes = row.get::<_, i64>(1)?;
                Ok(ArtifactVersion {
                    version: u16::try_from(version).map_err(|_| rusqlite::Error::InvalidQuery)?,
                    bytes: u64::try_from(bytes).map_err(|_| rusqlite::Error::InvalidQuery)?,
                    created_at: parse_timestamp(row.get(2)?)?,
                })
            },
        )?;
        let versions = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(ArtifactDocument {
            summary,
            content,
            versions,
        })
    }
    pub fn append_artifact_version(
        &self,
        session_id: SessionId,
        artifact_id: ArtifactId,
        content: &str,
        at: Timestamp,
    ) -> Result<ArtifactDocument, StorageError> {
        validate_artifact_content(content)?;
        let mut connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let state: Option<(i64, i64)> = tx
            .query_row(
                "SELECT a.current_version,COALESCE(SUM(v.byte_len),0)
                 FROM artifacts a LEFT JOIN artifact_versions v ON v.artifact_id=a.id
                 WHERE a.session_id=?1 AND a.id=?2 GROUP BY a.id,a.current_version",
                params![session_id.to_string(), artifact_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((current_version, retained_bytes)) = state else {
            return Err(StorageError::ArtifactNotFound(artifact_id));
        };
        if current_version as usize >= MAX_ARTIFACT_VERSIONS
            || retained_bytes.saturating_add(content.len() as i64) > MAX_ARTIFACT_TOTAL_BYTES as i64
        {
            return Err(StorageError::ArtifactLimitExceeded);
        }
        let next_version = current_version
            .checked_add(1)
            .ok_or(StorageError::ArtifactLimitExceeded)?;
        let at = at.to_string();
        tx.execute(
            "INSERT INTO artifact_versions (artifact_id,version,content,byte_len,created_at)
             VALUES (?1,?2,?3,?4,?5)",
            params![
                artifact_id.to_string(),
                next_version,
                content,
                content.len() as i64,
                at,
            ],
        )?;
        tx.execute(
            "UPDATE artifacts SET current_version=?1,updated_at=?2 WHERE id=?3",
            params![next_version, at, artifact_id.to_string()],
        )?;
        tx.commit()?;
        drop(connection);
        self.get_artifact(session_id, artifact_id)
    }
    pub fn list_messages(
        &self,
        session_id: SessionId,
        limit: usize,
    ) -> Result<Vec<MessageRecord>, StorageError> {
        let limit = limit.clamp(1, 1_000) as i64;
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let mut statement=connection.prepare("SELECT id,session_id,role,inline_text,blob_hash,byte_len,created_at FROM messages WHERE session_id=?1 ORDER BY rowid ASC LIMIT ?2")?;
        let rows = statement.query_map(params![session_id.to_string(), limit], decode_message)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }
    pub fn list_message_history_page(
        &self,
        session_id: SessionId,
        before: Option<MessageId>,
        limit: usize,
    ) -> Result<(Vec<MessageRecord>, Option<MessageId>), StorageError> {
        self.get_session(session_id)?;
        let limit = limit.clamp(1, 100);
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let before_rowid = match before {
            Some(message_id) => connection
                .query_row(
                    "SELECT rowid FROM messages WHERE session_id=?1 AND id=?2",
                    params![session_id.to_string(), message_id.to_string()],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?
                .ok_or(StorageError::MessageNotFound(message_id))?,
            None => i64::MAX,
        };
        let mut statement = connection.prepare(
            "SELECT id,session_id,role,inline_text,blob_hash,byte_len,created_at
             FROM messages WHERE session_id=?1 AND rowid<?2
             ORDER BY rowid DESC LIMIT ?3",
        )?;
        let rows = statement.query_map(
            params![session_id.to_string(), before_rowid, (limit + 1) as i64],
            decode_message,
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
    pub fn message_ordinal(
        &self,
        session_id: SessionId,
        message_id: MessageId,
    ) -> Result<Option<u64>, StorageError> {
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let rowid: Option<i64> = connection
            .query_row(
                "SELECT rowid FROM messages WHERE session_id=?1 AND id=?2",
                params![session_id.to_string(), message_id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        let Some(rowid) = rowid else {
            return Ok(None);
        };
        let ordinal: i64 = connection.query_row(
            "SELECT COUNT(*) FROM messages WHERE session_id=?1 AND rowid<=?2",
            params![session_id.to_string(), rowid],
            |row| row.get(0),
        )?;
        u64::try_from(ordinal)
            .map(Some)
            .map_err(|_| StorageError::Sqlite(rusqlite::Error::InvalidQuery))
    }
    pub fn append_event(
        &self,
        session_id: SessionId,
        kind: &str,
        payload_json: &str,
        at: Timestamp,
    ) -> Result<u64, StorageError> {
        if payload_json.len() > MAX_EVENT_PAYLOAD_BYTES {
            return Err(StorageError::EventPayloadTooLarge);
        }
        let mut connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("INSERT INTO recent_events (session_id,kind,payload_json,created_at) VALUES (?1,?2,?3,?4)",params![session_id.to_string(),kind,payload_json,at.to_string()])?;
        let sequence = tx.last_insert_rowid() as u64;
        tx.execute("DELETE FROM recent_events WHERE session_id=?1 AND seq NOT IN (SELECT seq FROM recent_events WHERE session_id=?1 ORDER BY seq DESC LIMIT ?2)",params![session_id.to_string(),self.max_events_per_session as i64])?;
        tx.commit()?;
        Ok(sequence)
    }
    pub fn incremental_vacuum(&self, pages: u32) -> Result<(), StorageError> {
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        connection.execute_batch(&format!("PRAGMA incremental_vacuum({pages});"))?;
        Ok(())
    }
    fn configure(connection: &Connection) -> Result<(), StorageError> {
        connection.execute_batch("PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000; PRAGMA synchronous=NORMAL; PRAGMA auto_vacuum=INCREMENTAL;")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        Ok(())
    }
    fn migrate(connection: &Connection) -> Result<(), StorageError> {
        connection.execute_batch("CREATE TABLE IF NOT EXISTS schema_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY,title TEXT NOT NULL,state TEXT NOT NULL CHECK(state IN ('active','archived')),created_at TEXT NOT NULL,updated_at TEXT NOT NULL,archived_at TEXT); CREATE INDEX IF NOT EXISTS sessions_updated_idx ON sessions(updated_at DESC); CREATE TABLE IF NOT EXISTS messages(id TEXT PRIMARY KEY,session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,role TEXT NOT NULL,inline_text TEXT,blob_hash TEXT,byte_len INTEGER NOT NULL CHECK(byte_len>=0),created_at TEXT NOT NULL,CHECK((inline_text IS NULL)!=(blob_hash IS NULL))); CREATE INDEX IF NOT EXISTS messages_session_idx ON messages(session_id,created_at,id); CREATE TABLE IF NOT EXISTS message_activity(message_id TEXT PRIMARY KEY REFERENCES messages(id) ON DELETE CASCADE,reasoning_summary TEXT NOT NULL CHECK(length(CAST(reasoning_summary AS BLOB))<=8192)); CREATE TABLE IF NOT EXISTS recent_events(seq INTEGER PRIMARY KEY AUTOINCREMENT,session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,kind TEXT NOT NULL,payload_json TEXT NOT NULL,created_at TEXT NOT NULL); CREATE INDEX IF NOT EXISTS recent_events_session_idx ON recent_events(session_id,seq); INSERT INTO schema_meta(key,value) VALUES('schema_version','1') ON CONFLICT(key) DO UPDATE SET value=excluded.value;")?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS draft_attachments(
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                name TEXT NOT NULL CHECK(length(CAST(name AS BLOB)) BETWEEN 1 AND 255),
                mime TEXT NOT NULL CHECK(length(CAST(mime AS BLOB)) BETWEEN 1 AND 255),
                blob_hash TEXT NOT NULL CHECK(length(blob_hash)=64),
                byte_len INTEGER NOT NULL CHECK(byte_len BETWEEN 1 AND 8388608),
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS draft_attachments_session_idx
                ON draft_attachments(session_id,created_at,id);",
        )?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS artifacts(
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                source_message_id TEXT NOT NULL,
                kind TEXT NOT NULL CHECK(kind IN ('writing','code')),
                title TEXT NOT NULL CHECK(length(CAST(title AS BLOB)) BETWEEN 1 AND 256),
                language TEXT CHECK(language IS NULL OR length(CAST(language AS BLOB)) BETWEEN 1 AND 64),
                current_version INTEGER NOT NULL CHECK(current_version BETWEEN 1 AND 16),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS artifacts_session_idx
                ON artifacts(session_id,updated_at DESC,id DESC);
            CREATE TABLE IF NOT EXISTS artifact_versions(
                artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE CASCADE,
                version INTEGER NOT NULL CHECK(version BETWEEN 1 AND 16),
                content TEXT NOT NULL CHECK(length(CAST(content AS BLOB))<=65536),
                byte_len INTEGER NOT NULL CHECK(byte_len>=0 AND byte_len=length(CAST(content AS BLOB))),
                created_at TEXT NOT NULL,
                PRIMARY KEY(artifact_id,version)
            );",
        )?;
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct BlobStore {
    root: PathBuf,
}
impl BlobStore {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    pub fn put(&self, bytes: &[u8]) -> Result<BlobInfo, StorageError> {
        fs::create_dir_all(&self.root)?;
        let hash = blake3::hash(bytes).to_hex().to_string();
        let path = self.path_for(&hash)?;
        if !path.exists() {
            let parent = path.parent().ok_or(StorageError::InvalidBlobHash)?;
            fs::create_dir_all(parent)?;
            let temp = parent.join(format!(".{hash}.tmp-{}", std::process::id()));
            {
                let file = fs::File::create(&temp)?;
                let mut encoder = zstd::Encoder::new(file, BLOB_COMPRESSION_LEVEL)?;
                encoder.write_all(bytes)?;
                let mut file = encoder.finish()?;
                file.flush()?;
                file.sync_all()?;
            }
            match fs::rename(&temp, &path) {
                Ok(()) => {}
                Err(error) if path.exists() => {
                    let _ = fs::remove_file(&temp);
                    let _ = error;
                }
                Err(error) => return Err(StorageError::Io(error)),
            }
        }
        let stored_bytes = fs::metadata(&path)?.len();
        Ok(BlobInfo {
            hash,
            raw_bytes: bytes.len() as u64,
            stored_bytes,
        })
    }
    pub fn get(&self, hash: &str) -> Result<Vec<u8>, StorageError> {
        let path = self.path_for(hash)?;
        let file = fs::File::open(path)?;
        let mut decoder = zstd::Decoder::new(file)?;
        let mut bytes = Vec::new();
        decoder.read_to_end(&mut bytes)?;
        if blake3::hash(&bytes).to_hex().as_str() != hash {
            return Err(StorageError::BlobHashMismatch);
        }
        Ok(bytes)
    }
    pub fn path_for(&self, hash: &str) -> Result<PathBuf, StorageError> {
        if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(StorageError::InvalidBlobHash);
        }
        Ok(self.root.join(&hash[..2]).join(format!("{hash}.zst")))
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlobInfo {
    pub hash: String,
    pub raw_bytes: u64,
    pub stored_bytes: u64,
}
fn encode_state(state: SessionState) -> &'static str {
    match state {
        SessionState::Active => "active",
        SessionState::Archived => "archived",
    }
}
fn decode_state(value: &str) -> Result<SessionState, rusqlite::Error> {
    match value {
        "active" => Ok(SessionState::Active),
        "archived" => Ok(SessionState::Archived),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}
fn encode_role(role: MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    }
}
fn decode_role(value: &str) -> Result<MessageRole, rusqlite::Error> {
    match value {
        "system" => Ok(MessageRole::System),
        "user" => Ok(MessageRole::User),
        "assistant" => Ok(MessageRole::Assistant),
        "tool" => Ok(MessageRole::Tool),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}
fn encode_artifact_kind(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Writing => "writing",
        ArtifactKind::Code => "code",
    }
}
fn decode_artifact_kind(value: &str) -> Result<ArtifactKind, rusqlite::Error> {
    match value {
        "writing" => Ok(ArtifactKind::Writing),
        "code" => Ok(ArtifactKind::Code),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}
fn validate_artifact_metadata(title: &str, language: Option<&str>) -> Result<(), StorageError> {
    if title.is_empty()
        || title.as_bytes().len() > MAX_ARTIFACT_TITLE_BYTES
        || language.is_some_and(|value| {
            value.is_empty() || value.as_bytes().len() > MAX_ARTIFACT_LANGUAGE_BYTES
        })
    {
        return Err(StorageError::InvalidArtifact);
    }
    Ok(())
}
fn validate_artifact_content(content: &str) -> Result<(), StorageError> {
    if content.as_bytes().len() > MAX_ARTIFACT_CONTENT_BYTES {
        return Err(StorageError::ArtifactContentTooLarge);
    }
    Ok(())
}
fn decode_artifact_summary(row: &rusqlite::Row<'_>) -> Result<ArtifactSummary, rusqlite::Error> {
    let current_version = row.get::<_, i64>(6)?;
    Ok(ArtifactSummary {
        id: parse_id(row.get(0)?)?,
        session_id: parse_id(row.get(1)?)?,
        source_message_id: parse_id(row.get(2)?)?,
        kind: decode_artifact_kind(&row.get::<_, String>(3)?)?,
        title: row.get(4)?,
        language: row.get(5)?,
        current_version: u16::try_from(current_version)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        created_at: parse_timestamp(row.get(7)?)?,
        updated_at: parse_timestamp(row.get(8)?)?,
    })
}
fn parse_id<T>(value: String) -> Result<T, rusqlite::Error>
where
    T: FromStr,
{
    value.parse().map_err(|_| rusqlite::Error::InvalidQuery)
}
fn parse_timestamp(value: String) -> Result<Timestamp, rusqlite::Error> {
    DateTime::parse_from_rfc3339(&value)
        .map(|value| Timestamp::from_datetime(value.with_timezone(&Utc)))
        .map_err(|_| rusqlite::Error::InvalidQuery)
}
fn decode_session(row: &rusqlite::Row<'_>) -> Result<SessionSummary, rusqlite::Error> {
    let archived: Option<String> = row.get(5)?;
    Ok(SessionSummary {
        id: parse_id(row.get(0)?)?,
        title: row.get(1)?,
        state: decode_state(&row.get::<_, String>(2)?)?,
        created_at: parse_timestamp(row.get(3)?)?,
        updated_at: parse_timestamp(row.get(4)?)?,
        archived_at: archived.map(parse_timestamp).transpose()?,
    })
}
fn decode_message(row: &rusqlite::Row<'_>) -> Result<MessageRecord, rusqlite::Error> {
    let inline: Option<String> = row.get(3)?;
    let blob: Option<String> = row.get(4)?;
    let byte_len = row
        .get::<_, i64>(5)
        .and_then(|value| u64::try_from(value).map_err(|_| rusqlite::Error::InvalidQuery))?;
    let body = match (inline, blob) {
        (Some(text), None) => PayloadRef::Inline { text },
        (None, Some(hash)) => PayloadRef::Blob {
            hash,
            bytes: byte_len,
        },
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(MessageRecord {
        id: parse_id(row.get(0)?)?,
        session_id: parse_id(row.get(1)?)?,
        role: decode_role(&row.get::<_, String>(2)?)?,
        body,
        created_at: parse_timestamp(row.get(6)?)?,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    fn now() -> Timestamp {
        let datetime = Timestamp::now().as_datetime();
        Timestamp::from_datetime(
            chrono::DateTime::from_timestamp_millis(datetime.timestamp_millis()).unwrap(),
        )
    }
    fn session(title: &str) -> SessionSummary {
        let now = now();
        SessionSummary {
            id: SessionId::new(),
            title: title.to_owned(),
            state: SessionState::Active,
            created_at: now,
            updated_at: now,
            archived_at: None,
        }
    }
    #[test]
    fn session_round_trip_and_archive() {
        let temp = tempdir().unwrap();
        let storage = Storage::open_in_memory(temp.path().join("blobs")).unwrap();
        let mut expected = session("first");
        storage.create_session(&expected).unwrap();
        assert_eq!(
            storage.list_sessions(false).unwrap(),
            vec![expected.clone()]
        );
        let archived_at = now();
        storage.archive_session(expected.id, archived_at).unwrap();
        expected.state = SessionState::Archived;
        expected.archived_at = Some(archived_at);
        expected.updated_at = archived_at;
        assert!(storage.list_sessions(false).unwrap().is_empty());
        assert_eq!(storage.get_session(expected.id).unwrap(), expected);
    }
    #[test]
    fn large_message_requires_blob() {
        let temp = tempdir().unwrap();
        let storage = Storage::open_in_memory(temp.path().join("blobs")).unwrap();
        let session = session("large");
        storage.create_session(&session).unwrap();
        let message = MessageRecord {
            id: MessageId::new(),
            session_id: session.id,
            role: MessageRole::User,
            body: PayloadRef::Inline {
                text: "x".repeat(MAX_INLINE_PAYLOAD_BYTES + 1),
            },
            created_at: Timestamp::now(),
        };
        assert!(matches!(
            storage.append_message(&message),
            Err(StorageError::InlinePayloadTooLarge)
        ));
    }
    #[test]
    fn blob_store_deduplicates() {
        let temp = tempdir().unwrap();
        let store = BlobStore::new(temp.path());
        let content = b"same payload same hash".repeat(4096);
        let first = store.put(&content).unwrap();
        let second = store.put(&content).unwrap();
        assert_eq!(first.hash, second.hash);
        assert_eq!(store.get(&first.hash).unwrap(), content);
    }
    #[test]
    fn event_payloads_bounded() {
        let temp = tempdir().unwrap();
        let storage = Storage::open_in_memory(temp.path().join("blobs")).unwrap();
        let session = session("events");
        storage.create_session(&session).unwrap();
        assert!(matches!(
            storage.append_event(
                session.id,
                "delta",
                &"x".repeat(MAX_EVENT_PAYLOAD_BYTES + 1),
                Timestamp::now()
            ),
            Err(StorageError::EventPayloadTooLarge)
        ));
    }
}
