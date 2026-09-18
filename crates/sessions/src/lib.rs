//! Session lifecycle over the format-2 workspace schema (primary) plus the
//! legacy format-1 [`SessionService`] kept for the server/CLI boundary.
#![forbid(unsafe_code)]
pub mod auto_compact;
pub mod auto_lease;
pub mod auto_sched;
pub mod branch_v2;
pub mod import;
pub mod index;
pub mod legacy_view;
pub mod mcp_status_panel;
pub mod migration;
pub mod ops_limits;
pub mod query;
pub mod reference;
pub mod remote_share;
pub mod share;
pub mod share_audit;
pub mod share_count;
pub mod share_expiry;
pub mod share_invite;
pub mod share_list;
pub mod share_merge;
pub mod share_policy;
pub mod share_queue;
pub mod share_revoke;
pub mod share_scope;
pub mod share_token;
pub mod state;
pub mod store;
pub mod task_quota;
pub mod tui_state;
mod types;
pub mod ui_001;
pub mod ui_002;
pub mod ui_003;
pub mod ui_004;
pub mod ui_005;
pub mod ui_006;
pub mod ui_007;
pub mod ui_008;
pub mod ui_009;
pub mod ui_010;
pub mod ui_011;
pub mod ui_012;
pub mod ui_013;
pub mod part_events;
pub mod runner;
pub mod tui_info_panel;
pub mod app_history;
pub mod session_membership;

use chrono::{DateTime, Utc};
use opencode_rk_contracts::{
    ArtifactDocument, ArtifactId, ArtifactKind, ArtifactSummary, AssistantActivity, AttachmentId,
    DraftAttachment, MessageId, MessageRecord, MessageRole, PayloadRef, SessionId, SessionState,
    SessionSummary, Timestamp, MAX_REASONING_SUMMARY_BYTES, MAX_TITLE_BYTES,
};
use opencode_rk_storage::{CatalogV2, NewMessage, NewSession, Storage, StorageError, V2Writer};
use rusqlite::{params, Connection, OptionalExtension};
use std::{path::{Path, PathBuf}, sync::{Arc, Mutex}};
use thiserror::Error;
pub use branch_v2::ForkProvenance;
pub type SessionRecord = SessionSummary;
pub use opencode_rk_contracts::MessageRole as Role;
const ACTIVE: i64 = 0;
const ARCHIVED: i64 = 1;
const PAGE_MAX: usize = 500;
/// Sync CRUD over a format-2 workspace [`Connection`]. Wraps writers in
/// [`V2Writer`] and reads with direct SQL. Not `Clone`; share via `Arc`.
pub struct SessionManager {
    conn: Mutex<Connection>,
}
impl SessionManager {
    #[must_use]
    pub fn new(conn: Connection) -> Self {
        Self {
            conn: Mutex::new(conn),
        }
    }
    pub fn create_session(&self, title: &str) -> Result<SessionId, SessionError> {
        let id = SessionId::new();
        let now = now_us();
        let mut conn = self.conn.lock().map_err(|_| SessionError::Poisoned)?;
        V2Writer::create_session(
            &mut conn,
            &NewSession {
                id,
                title: title.to_owned(),
                created_at_us: now,
                updated_at_us: now,
            },
        )?;
        Ok(id)
    }
    pub fn get_session(&self, id: SessionId) -> Result<Option<SessionRecord>, SessionError> {
        let conn = self.conn.lock().map_err(|_| SessionError::Poisoned)?;
        Ok(conn.query_row("SELECT title, state, created_at_us, updated_at_us, archived_at_us FROM sessions WHERE id=?1", params![id.as_uuid().as_bytes().as_slice()], |row| decode_session(id, row)).optional()?)
    }
    pub fn list_sessions(&self, limit: usize) -> Result<Vec<SessionSummary>, SessionError> {
        self.list_sessions_paged(limit, 0)
    }
    pub fn list_sessions_paged(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<SessionSummary>, SessionError> {
        let conn = self.conn.lock().map_err(|_| SessionError::Poisoned)?;
        let mut stmt = conn.prepare("SELECT id, title, state, created_at_us, updated_at_us, archived_at_us FROM sessions WHERE state=?1 ORDER BY updated_at_us DESC, pk DESC LIMIT ?2 OFFSET ?3")?;
        let rows = stmt.query_map(
            params![ACTIVE, clamp_page(limit) as i64, offset.max(0) as i64],
            decode_summary_row,
        )?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(SessionError::from)
    }

    /// List all sessions including archived ones (no state filter).
    pub fn list_all_sessions(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<SessionSummary>, SessionError> {
        let conn = self.conn.lock().map_err(|_| SessionError::Poisoned)?;
        let mut stmt = conn.prepare("SELECT id, title, state, created_at_us, updated_at_us, archived_at_us FROM sessions ORDER BY updated_at_us DESC, pk DESC LIMIT ?1 OFFSET ?2")?;
        let rows = stmt.query_map(
            params![clamp_page(limit) as i64, offset.max(0) as i64],
            decode_summary_row,
        )?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(SessionError::from)
    }
    pub fn rename_session(&self, id: SessionId, title: &str) -> Result<(), SessionError> {
        let conn = self.conn.lock().map_err(|_| SessionError::Poisoned)?;
        let changed = conn.execute(
            "UPDATE sessions SET title=?1, updated_at_us=?2 WHERE id=?3",
            params![title, now_us(), id.as_uuid().as_bytes().as_slice()],
        )?;
        if changed == 0 {
            return Err(SessionError::NotFound(id));
        }
        Ok(())
    }
    pub fn archive_session(&self, id: SessionId) -> Result<(), SessionError> {
        let conn = self.conn.lock().map_err(|_| SessionError::Poisoned)?;
        let now = now_us();
        let changed = conn.execute(
            "UPDATE sessions SET state=?1, archived_at_us=?2, updated_at_us=?2 WHERE id=?3",
            params![ARCHIVED, now, id.as_uuid().as_bytes().as_slice()],
        )?;
        if changed == 0 {
            return Err(SessionError::NotFound(id));
        }
        Ok(())
    }
    pub fn append_message(
        &self,
        session: SessionId,
        role: Role,
        content: &str,
    ) -> Result<MessageId, SessionError> {
        let message = NewMessage {
            id: MessageId::new(),
            session_id: session,
            role,
            body: PayloadRef::Inline {
                text: content.to_owned(),
            },
            created_at_us: now_us(),
        };
        let mut conn = self.conn.lock().map_err(|_| SessionError::Poisoned)?;
        match V2Writer::append_message(&mut conn, &message) {
            Ok(()) => Ok(message.id),
            Err(StorageError::SessionNotFound(missing)) => Err(SessionError::NotFound(missing)),
            Err(other) => Err(SessionError::Storage(other)),
        }
    }
    pub fn list_messages(&self, session: SessionId) -> Result<Vec<MessageRecord>, SessionError> {
        self.list_messages_paged(session, PAGE_MAX)
    }
    pub fn list_messages_paged(
        &self,
        session: SessionId,
        limit: usize,
    ) -> Result<Vec<MessageRecord>, SessionError> {
        let conn = self.conn.lock().map_err(|_| SessionError::Poisoned)?;
        let mut stmt = conn.prepare("SELECT m.id, m.seq, m.role, m.created_at_us, p.inline_data, p.blob_pk, p.raw_bytes, b.hash FROM messages m JOIN sessions s ON s.pk=m.session_pk JOIN message_parts mp ON mp.message_pk=m.pk AND mp.ordinal=0 JOIN payloads p ON p.pk=mp.payload_pk LEFT JOIN blobs b ON b.pk=p.blob_pk WHERE s.id=?1 ORDER BY m.seq ASC LIMIT ?2")?;
        let rows = stmt.query_map(
            params![
                session.as_uuid().as_bytes().as_slice(),
                clamp_page(limit) as i64
            ],
            |row| decode_message_row(session, row),
        )?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(SessionError::from)
    }
    pub fn get_message_record(
        &self,
        session: SessionId,
        message_id: MessageId,
    ) -> Result<MessageRecord, SessionError> {
        let conn = self.conn.lock().map_err(|_| SessionError::Poisoned)?;
        conn.query_row(
            "SELECT m.id, m.seq, m.role, m.created_at_us, p.inline_data, p.blob_pk, p.raw_bytes, b.hash
             FROM messages m
             JOIN sessions s ON s.pk=m.session_pk
             JOIN message_parts mp ON mp.message_pk=m.pk AND mp.ordinal=0
             JOIN payloads p ON p.pk=mp.payload_pk
             LEFT JOIN blobs b ON b.pk=p.blob_pk
             WHERE s.id=?1 AND m.id=?2",
            params![
                session.as_uuid().as_bytes().as_slice(),
                message_id.as_uuid().as_bytes().as_slice(),
            ],
            |row| decode_message_row(session, row),
        )
        .optional()?
        .ok_or(SessionError::ArtifactSourceInvalid(message_id))
    }
    pub fn session_message_count(&self, session: SessionId) -> Result<u64, SessionError> {
        let conn = self.conn.lock().map_err(|_| SessionError::Poisoned)?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM messages m JOIN sessions s ON s.pk=m.session_pk WHERE s.id=?1",
            params![session.as_uuid().as_bytes().as_slice()],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as u64)
    }
}
fn decode_session(
    id: SessionId,
    row: &rusqlite::Row<'_>,
) -> Result<SessionRecord, rusqlite::Error> {
    Ok(SessionRecord {
        id,
        title: row.get(0)?,
        state: decode_state(row.get(1)?)?,
        created_at: decode_ts(row.get(2)?)?,
        updated_at: decode_ts(row.get(3)?)?,
        archived_at: row.get::<_, Option<i64>>(4)?.map(decode_ts).transpose()?,
    })
}
fn decode_summary_row(row: &rusqlite::Row<'_>) -> Result<SessionSummary, rusqlite::Error> {
    let idb: Vec<u8> = row.get(0)?;
    let id = decode_sid(&idb)?;
    Ok(SessionSummary {
        id,
        title: row.get(1)?,
        state: decode_state(row.get(2)?)?,
        created_at: decode_ts(row.get(3)?)?,
        updated_at: decode_ts(row.get(4)?)?,
        archived_at: row.get::<_, Option<i64>>(5)?.map(decode_ts).transpose()?,
    })
}
fn decode_message_row(
    session: SessionId,
    row: &rusqlite::Row<'_>,
) -> Result<MessageRecord, rusqlite::Error> {
    let idb: Vec<u8> = row.get(0)?;
    let _seq: i64 = row.get(1)?;
    let role: i64 = row.get(2)?;
    let created: i64 = row.get(3)?;
    let inline: Option<Vec<u8>> = row.get(4)?;
    let _blob_pk: Option<i64> = row.get(5)?;
    let raw: i64 = row.get(6)?;
    let hash: Option<Vec<u8>> = row.get(7)?;
    let body = match (inline, hash) {
        (Some(bytes), _) => decode_inline(&bytes)?,
        (None, Some(digest)) => PayloadRef::blob(hex(&digest), raw.max(0) as u64),
        (None, None) => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(MessageRecord {
        id: decode_mid(&idb)?,
        session_id: session,
        role: decode_role(role)?,
        body,
        created_at: decode_ts(created)?,
    })
}
fn decode_inline(bytes: &[u8]) -> Result<PayloadRef, rusqlite::Error> {
    String::from_utf8(bytes.to_vec())
        .map(|text| PayloadRef::Inline { text })
        .map_err(|_| rusqlite::Error::InvalidQuery)
}
fn decode_state(value: i64) -> Result<SessionState, rusqlite::Error> {
    match value {
        ACTIVE => Ok(SessionState::Active),
        ARCHIVED => Ok(SessionState::Archived),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}
fn decode_role(value: i64) -> Result<MessageRole, rusqlite::Error> {
    match value {
        0 => Ok(MessageRole::System),
        1 => Ok(MessageRole::User),
        2 => Ok(MessageRole::Assistant),
        3 => Ok(MessageRole::Tool),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}
fn decode_ts(micros: i64) -> Result<Timestamp, rusqlite::Error> {
    DateTime::from_timestamp_micros(micros)
        .map(Timestamp::from_datetime)
        .ok_or(rusqlite::Error::InvalidQuery)
}
fn decode_sid(bytes: &[u8]) -> Result<SessionId, rusqlite::Error> {
    uuid_from(bytes).map(SessionId::from_uuid)
}
fn decode_mid(bytes: &[u8]) -> Result<MessageId, rusqlite::Error> {
    uuid_from(bytes).map(MessageId::from_uuid)
}
fn uuid_from(bytes: &[u8]) -> Result<uuid::Uuid, rusqlite::Error> {
    uuid::Uuid::from_slice(bytes).map_err(|_| rusqlite::Error::InvalidQuery)
}
fn now_us() -> i64 {
    Utc::now().timestamp_micros()
}
fn clamp_page(limit: usize) -> usize {
    limit.clamp(1, PAGE_MAX)
}
fn hex(bytes: &[u8]) -> String {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(H[(b >> 4) as usize] as char);
        out.push(H[(b & 15) as usize] as char);
    }
    out
}
#[derive(Clone)]
pub struct SessionService {
    storage: Arc<Storage>,
    branch_manager: Option<Arc<SessionManager>>,
    workspace_catalog_path: Option<Arc<PathBuf>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSummary {
    pub id: [u8; 16],
    pub label: String,
    pub project_root: Option<String>,
    pub status: u8,
    pub created_at_us: i64,
}

impl SessionService {
    #[must_use]
    pub fn new(storage: Arc<Storage>) -> Self {
        Self {
            storage,
            branch_manager: None,
            workspace_catalog_path: None,
        }
    }
    #[must_use]
    pub fn with_branch_manager(storage: Arc<Storage>, branch_manager: Arc<SessionManager>) -> Self {
        Self {
            storage,
            branch_manager: Some(branch_manager),
            workspace_catalog_path: None,
        }
    }
    pub fn with_workspace_catalog_path(mut self, path: &Path) -> Result<Self, SessionError> {
        CatalogV2::open_existing(path)?;
        self.workspace_catalog_path = Some(Arc::new(path.to_path_buf()));
        Ok(self)
    }
    pub async fn list_workspaces(
        &self,
        limit: usize,
    ) -> Result<Option<Vec<WorkspaceSummary>>, SessionError> {
        let Some(path) = self.workspace_catalog_path.as_ref() else {
            return Ok(None);
        };
        let path = Arc::clone(path);
        let workspaces = run_blocking(move || {
            let connection = CatalogV2::open_existing(path.as_ref())?;
            let rows = CatalogV2::list_workspaces(&connection, limit)?;
            Ok(rows
                .into_iter()
                .map(|row| WorkspaceSummary {
                    id: row.id,
                    label: row.label,
                    project_root: row.project_root,
                    status: row.status,
                    created_at_us: row.created_at_us,
                })
                .collect())
        })
        .await?;
        Ok(Some(workspaces))
    }
    pub async fn create(&self, title: impl Into<String>) -> Result<SessionSummary, SessionError> {
        let now = Timestamp::now();
        let session = SessionSummary {
            id: SessionId::new(),
            title: title.into(),
            state: SessionState::Active,
            created_at: now,
            updated_at: now,
            archived_at: None,
        };
        let storage = Arc::clone(&self.storage);
        let candidate = session.clone();
        run_blocking(move || storage.create_session(&candidate)).await?;
        Ok(session)
    }
    pub async fn get(&self, id: SessionId) -> Result<SessionSummary, SessionError> {
        if let Some(manager) = self.fork_manager_for(id).await? {
            let session = run_session_blocking(move || manager.get_session(id)).await?;
            return session.ok_or(SessionError::NotFound(id));
        }
        let storage = Arc::clone(&self.storage);
        Ok(run_blocking(move || storage.get_session(id)).await?)
    }
    pub async fn list(&self, all: bool) -> Result<Vec<SessionSummary>, SessionError> {
        let storage = Arc::clone(&self.storage);
        let mut sessions = run_blocking(move || storage.list_sessions(all)).await?;
        if let Some(manager) = &self.branch_manager {
            let manager = Arc::clone(manager);
            let mut forks =
                run_session_blocking(move || manager.list_fork_sessions(all, 500)).await?;
            sessions.append(&mut forks);
            sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        }
        Ok(sessions)
    }
    pub async fn rename(
        &self,
        id: SessionId,
        title: impl Into<String>,
    ) -> Result<(), SessionError> {
        if let Some(manager) = self.fork_manager_for(id).await? {
            let title = title.into();
            return run_session_blocking(move || manager.rename_session(id, &title)).await;
        }
        let storage = Arc::clone(&self.storage);
        let title = title.into();
        run_blocking(move || storage.rename_session(id, &title, Timestamp::now())).await?;
        Ok(())
    }
    pub async fn archive(&self, id: SessionId) -> Result<(), SessionError> {
        if let Some(manager) = self.fork_manager_for(id).await? {
            return run_session_blocking(move || manager.archive_session(id)).await;
        }
        let storage = Arc::clone(&self.storage);
        run_blocking(move || storage.archive_session(id, Timestamp::now())).await?;
        Ok(())
    }
    pub async fn append_text(
        &self,
        session_id: SessionId,
        role: MessageRole,
        text: impl Into<String>,
    ) -> Result<MessageRecord, SessionError> {
        let text = text.into();
        if let Some(manager) = self.fork_manager_for(session_id).await? {
            return run_session_blocking(move || manager.append_fork_text(session_id, role, text))
                .await;
        }
        let body =
            PayloadRef::inline(text).map_err(|e| SessionError::Contract(e.to_string()))?;
        let message = MessageRecord {
            id: MessageId::new(),
            session_id,
            role,
            body,
            created_at: Timestamp::now(),
        };
        let storage = Arc::clone(&self.storage);
        let candidate = message.clone();
        run_blocking(move || storage.append_message(&candidate)).await?;
        Ok(message)
    }
    pub async fn append_assistant_with_reasoning(
        &self,
        session_id: SessionId,
        text: impl Into<String>,
        reasoning_summary: Option<String>,
    ) -> Result<MessageRecord, SessionError> {
        let text = text.into();
        if reasoning_summary
            .as_ref()
            .is_some_and(|summary| summary.as_bytes().len() > MAX_REASONING_SUMMARY_BYTES)
        {
            return Err(SessionError::Contract(
                "reasoning summary exceeds the transcript activity bound".to_owned(),
            ));
        }
        if let Some(manager) = self.fork_manager_for(session_id).await? {
            return run_session_blocking(move || {
                manager.append_fork_assistant_with_reasoning(session_id, text, reasoning_summary)
            })
            .await;
        }
        let body =
            PayloadRef::inline(text).map_err(|e| SessionError::Contract(e.to_string()))?;
        let message = MessageRecord {
            id: MessageId::new(),
            session_id,
            role: MessageRole::Assistant,
            body,
            created_at: Timestamp::now(),
        };
        let storage = Arc::clone(&self.storage);
        let candidate = message.clone();
        run_blocking(move || {
            storage.append_message_with_reasoning(&candidate, reasoning_summary.as_deref())
        })
        .await?;
        Ok(message)
    }
    pub async fn append_blob(
        &self,
        session_id: SessionId,
        role: MessageRole,
        bytes: Vec<u8>,
    ) -> Result<MessageRecord, SessionError> {
        if self.fork_manager_for(session_id).await?.is_some() {
            return Err(SessionError::BranchPayloadUnsupported);
        }
        let storage = Arc::clone(&self.storage);
        let (hash, byte_len) = run_blocking(move || {
            storage
                .blob_store()
                .put(&bytes)
                .map(|i| (i.hash, i.raw_bytes))
        })
        .await?;
        let message = MessageRecord {
            id: MessageId::new(),
            session_id,
            role,
            body: PayloadRef::blob(hash, byte_len),
            created_at: Timestamp::now(),
        };
        let storage = Arc::clone(&self.storage);
        let candidate = message.clone();
        run_blocking(move || storage.append_message(&candidate)).await?;
        Ok(message)
    }
    pub async fn create_draft_attachment(
        &self,
        session_id: SessionId,
        name: String,
        mime: String,
        bytes: Vec<u8>,
    ) -> Result<DraftAttachment, SessionError> {
        if self.fork_manager_for(session_id).await?.is_some() {
            return Err(SessionError::DraftAttachmentUnavailable);
        }
        let storage = Arc::clone(&self.storage);
        match run_blocking(move || {
            storage.create_draft_attachment(session_id, &name, &mime, &bytes)
        })
        .await
        {
            Err(SessionError::Storage(StorageError::InlinePayloadTooLarge)) => {
                Err(SessionError::DraftAttachmentTooLarge)
            }
            Err(SessionError::Storage(StorageError::Sqlite(rusqlite::Error::InvalidQuery))) => {
                Err(SessionError::InvalidDraftAttachment)
            }
            other => other,
        }
    }
    pub async fn draft_attachments(
        &self,
        session_id: SessionId,
    ) -> Result<Vec<DraftAttachment>, SessionError> {
        if self.fork_manager_for(session_id).await?.is_some() {
            return Err(SessionError::DraftAttachmentUnavailable);
        }
        let storage = Arc::clone(&self.storage);
        Ok(run_blocking(move || storage.list_draft_attachments(session_id)).await?)
    }
    pub async fn delete_draft_attachment(
        &self,
        session_id: SessionId,
        attachment_id: AttachmentId,
    ) -> Result<(), SessionError> {
        if self.fork_manager_for(session_id).await?.is_some() {
            return Err(SessionError::DraftAttachmentUnavailable);
        }
        let storage = Arc::clone(&self.storage);
        match run_blocking(move || storage.delete_draft_attachment(session_id, attachment_id)).await {
            Ok(()) => Ok(()),
            Err(SessionError::Storage(StorageError::Sqlite(
                rusqlite::Error::QueryReturnedNoRows,
            ))) => Err(SessionError::DraftAttachmentNotFound(attachment_id)),
            Err(other) => Err(other),
        }
    }
    pub async fn messages(
        &self,
        session_id: SessionId,
        limit: usize,
    ) -> Result<Vec<MessageRecord>, SessionError> {
        if let Some(manager) = self.fork_manager_for(session_id).await? {
            return run_session_blocking(move || manager.list_messages_paged(session_id, limit))
                .await;
        }
        let storage = Arc::clone(&self.storage);
        Ok(run_blocking(move || storage.list_messages(session_id, limit)).await?)
    }
    pub async fn history_page(
        &self,
        session_id: SessionId,
        before: Option<MessageId>,
        limit: usize,
    ) -> Result<(Vec<MessageRecord>, Option<MessageId>), SessionError> {
        if let Some(manager) = self.fork_manager_for(session_id).await? {
            return run_session_blocking(move || {
                manager.list_message_history_page(session_id, before, limit)
            })
            .await;
        }
        let storage = Arc::clone(&self.storage);
        match run_blocking(move || storage.list_message_history_page(session_id, before, limit)).await {
            Err(SessionError::Storage(StorageError::MessageNotFound(message_id))) => {
                Err(SessionError::HistoryCursorNotFound(message_id))
            }
            other => other,
        }
    }
    pub async fn assistant_activity(
        &self,
        session_id: SessionId,
        limit: usize,
    ) -> Result<Vec<AssistantActivity>, SessionError> {
        if let Some(manager) = self.fork_manager_for(session_id).await? {
            return run_session_blocking(move || manager.list_assistant_activity(session_id, limit))
                .await;
        }
        let storage = Arc::clone(&self.storage);
        Ok(run_blocking(move || storage.list_assistant_activity(session_id, limit)).await?)
    }
    pub async fn create_artifact(
        &self,
        session_id: SessionId,
        source_message_id: MessageId,
        kind: ArtifactKind,
        title: String,
        language: Option<String>,
        content: String,
    ) -> Result<ArtifactDocument, SessionError> {
        let source = if let Some(manager) = self.fork_manager_for(session_id).await? {
            run_session_blocking(move || manager.get_message_record(session_id, source_message_id))
                .await?
        } else {
            let storage = Arc::clone(&self.storage);
            match run_blocking(move || storage.get_message(session_id, source_message_id)).await {
                Ok(message) => message,
                Err(SessionError::Storage(StorageError::MessageNotFound(_))) => {
                    return Err(SessionError::ArtifactSourceInvalid(source_message_id));
                }
                Err(error) => return Err(error),
            }
        };
        if source.role != MessageRole::Assistant {
            return Err(SessionError::ArtifactSourceInvalid(source_message_id));
        }
        let storage = Arc::clone(&self.storage);
        map_artifact_storage_result(
            run_blocking(move || {
                storage.create_artifact(
                    session_id,
                    source_message_id,
                    kind,
                    &title,
                    language.as_deref(),
                    &content,
                    Timestamp::now(),
                )
            })
            .await,
        )
    }
    pub async fn artifacts(
        &self,
        session_id: SessionId,
        limit: usize,
    ) -> Result<Vec<ArtifactSummary>, SessionError> {
        self.get(session_id).await?;
        let storage = Arc::clone(&self.storage);
        map_artifact_storage_result(
            run_blocking(move || storage.list_artifacts(session_id, limit)).await,
        )
    }
    pub async fn artifact(
        &self,
        session_id: SessionId,
        artifact_id: ArtifactId,
    ) -> Result<ArtifactDocument, SessionError> {
        self.get(session_id).await?;
        let storage = Arc::clone(&self.storage);
        map_artifact_storage_result(
            run_blocking(move || storage.get_artifact(session_id, artifact_id)).await,
        )
    }
    pub async fn append_artifact_version(
        &self,
        session_id: SessionId,
        artifact_id: ArtifactId,
        content: String,
    ) -> Result<ArtifactDocument, SessionError> {
        self.get(session_id).await?;
        let storage = Arc::clone(&self.storage);
        map_artifact_storage_result(
            run_blocking(move || {
                storage.append_artifact_version(
                    session_id,
                    artifact_id,
                    &content,
                    Timestamp::now(),
                )
            })
            .await,
        )
    }
    pub async fn branch_from_message(
        &self,
        parent_session_id: SessionId,
        through_message_id: MessageId,
    ) -> Result<(SessionSummary, ForkProvenance), SessionError> {
        let manager = self
            .branch_manager
            .as_ref()
            .cloned()
            .ok_or(SessionError::BranchingUnavailable)?;
        let parent = self.get(parent_session_id).await?;
        let is_v2_parent = {
            let manager = Arc::clone(&manager);
            run_session_blocking(move || manager.is_fork_session(parent_session_id)).await?
        };
        if !is_v2_parent {
            let storage = Arc::clone(&self.storage);
            let ordinal = run_blocking(move || {
                storage.message_ordinal(parent_session_id, through_message_id)
            })
            .await?
            .ok_or(SessionError::BranchMessageNotFound(through_message_id))?;
            if ordinal > opencode_rk_storage::fork_v2::MAX_FORK_COPY_MESSAGES as u64 {
                return Err(SessionError::ForkHistoryTooLarge);
            }
            let storage = Arc::clone(&self.storage);
            let prefix = run_blocking(move || {
                storage.list_messages(parent_session_id, ordinal as usize)
            })
            .await?;
            let manager_for_sync = Arc::clone(&manager);
            let parent_for_sync = parent.clone();
            let storage = Arc::clone(&self.storage);
            let activity = run_blocking(move || storage.list_assistant_activity(parent_session_id, 500)).await?;
            run_session_blocking(move || {
                manager_for_sync.synchronize_legacy_shadow(&parent_for_sync, &prefix, &activity)
            })
            .await?;
        }
        let title = branch_title(&parent.title);
        run_session_blocking(move || {
            manager.fork_from_message(parent_session_id, through_message_id, &title)
        })
        .await
    }
    pub async fn prepare_retry_branch(
        &self,
        parent_session_id: SessionId,
        target_message_id: MessageId,
    ) -> Result<(SessionSummary, ForkProvenance, String), SessionError> {
        let manager = self
            .branch_manager
            .as_ref()
            .cloned()
            .ok_or(SessionError::BranchingUnavailable)?;
        let parent = self.get(parent_session_id).await?;
        let is_v2_parent = {
            let manager = Arc::clone(&manager);
            run_session_blocking(move || manager.is_fork_session(parent_session_id)).await?
        };
        let (request_message_id, request_text) = if is_v2_parent {
            let manager_for_request = Arc::clone(&manager);
            run_session_blocking(move || {
                manager_for_request.retry_request(parent_session_id, target_message_id)
            })
            .await?
        } else {
            let storage = Arc::clone(&self.storage);
            let ordinal = run_blocking(move || storage.message_ordinal(parent_session_id, target_message_id))
                .await?
                .ok_or(SessionError::BranchMessageNotFound(target_message_id))?;
            if ordinal > opencode_rk_storage::fork_v2::MAX_FORK_COPY_MESSAGES as u64 {
                return Err(SessionError::ForkHistoryTooLarge);
            }
            let storage = Arc::clone(&self.storage);
            let prefix = run_blocking(move || storage.list_messages(parent_session_id, ordinal as usize)).await?;
            let target = prefix.last().ok_or(SessionError::InvalidBranchBoundary)?;
            if target.id != target_message_id {
                return Err(SessionError::BranchMessageNotFound(target_message_id));
            }
            let request = if target.role == MessageRole::User {
                target
            } else if target.role == MessageRole::Assistant {
                prefix
                    .iter()
                    .rev()
                    .skip(1)
                    .find(|message| message.role == MessageRole::User)
                    .ok_or(SessionError::InvalidBranchBoundary)?
            } else {
                return Err(SessionError::InvalidBranchBoundary);
            };
            let PayloadRef::Inline { text } = &request.body else {
                return Err(SessionError::BranchPayloadUnsupported);
            };
            let request_message_id = request.id;
            let request_text = text.clone();
            let request_ordinal = prefix
                .iter()
                .position(|message| message.id == request_message_id)
                .ok_or(SessionError::InvalidBranchBoundary)?
                + 1;
            let shadow_prefix = prefix[..request_ordinal].to_vec();
            let manager_for_sync = Arc::clone(&manager);
            let parent_for_sync = parent.clone();
            let storage = Arc::clone(&self.storage);
            let activity = run_blocking(move || storage.list_assistant_activity(parent_session_id, 500)).await?;
            run_session_blocking(move || {
                manager_for_sync.synchronize_legacy_shadow(&parent_for_sync, &shadow_prefix, &activity)
            })
            .await?;
            (request_message_id, request_text)
        };
        let title = branch_title(&parent.title);
        let manager_for_fork = Arc::clone(&manager);
        let (child, provenance) = run_session_blocking(move || {
            manager_for_fork.fork_before_user_message(parent_session_id, request_message_id, &title)
        })
        .await?;
        Ok((child, provenance, request_text))
    }

    pub async fn fork_provenance(
        &self,
        child_session_id: SessionId,
    ) -> Result<Option<ForkProvenance>, SessionError> {
        let Some(manager) = &self.branch_manager else {
            return Ok(None);
        };
        let manager = Arc::clone(manager);
        run_session_blocking(move || manager.fork_provenance(child_session_id)).await
    }
    async fn fork_manager_for(
        &self,
        session_id: SessionId,
    ) -> Result<Option<Arc<SessionManager>>, SessionError> {
        let Some(manager) = &self.branch_manager else {
            return Ok(None);
        };
        let manager = Arc::clone(manager);
        let check = Arc::clone(&manager);
        if run_session_blocking(move || check.is_fork_session(session_id)).await? {
            Ok(Some(manager))
        } else {
            Ok(None)
        }
    }
}
fn branch_title(source: &str) -> String {
    const PREFIX: &str = "Branch: ";
    let mut title = String::with_capacity((PREFIX.len() + source.len()).min(MAX_TITLE_BYTES));
    title.push_str(PREFIX);
    for ch in source.chars() {
        if title.len() + ch.len_utf8() > MAX_TITLE_BYTES {
            break;
        }
        title.push(ch);
    }
    title
}
async fn run_blocking<T, F>(operation: F) -> Result<T, SessionError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, StorageError> + Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|e| SessionError::BlockingTask(e.to_string()))?
        .map_err(SessionError::Storage)
}
async fn run_session_blocking<T, F>(operation: F) -> Result<T, SessionError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, SessionError> + Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|e| SessionError::BlockingTask(e.to_string()))?
}
fn map_artifact_storage_result<T>(result: Result<T, SessionError>) -> Result<T, SessionError> {
    match result {
        Err(SessionError::Storage(StorageError::ArtifactNotFound(id))) => {
            Err(SessionError::ArtifactNotFound(id))
        }
        Err(SessionError::Storage(StorageError::ArtifactContentTooLarge)) => {
            Err(SessionError::ArtifactContentTooLarge)
        }
        Err(SessionError::Storage(StorageError::ArtifactLimitExceeded)) => {
            Err(SessionError::ArtifactLimitExceeded)
        }
        Err(SessionError::Storage(StorageError::InvalidArtifact)) => {
            Err(SessionError::InvalidArtifact)
        }
        other => other,
    }
}
#[derive(Debug, Error)]
pub enum SessionError {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("session not found: {0}")]
    NotFound(SessionId),
    #[error("blocking storage task failed: {0}")]
    BlockingTask(String),
    #[error("contract violation: {0}")]
    Contract(String),
    #[error("branching is unavailable because the format-2 branch workspace is not configured")]
    BranchingUnavailable,
    #[error("branch boundary message not found: {0}")]
    BranchMessageNotFound(MessageId),
    #[error("message role cannot be used as a visible branch boundary")]
    InvalidBranchBoundary,
    #[error("fork history exceeds the format-2 copy bound")]
    ForkHistoryTooLarge,
    #[error("fork depth exceeds the format-2 branch bound")]
    ForkDepthExceeded,
    #[error("branching blob-backed history is unavailable until the format-2 blob adapter is active")]
    BranchPayloadUnsupported,
    #[error("draft attachments are unavailable for format-2 branch sessions until blob stores are unified")]
    DraftAttachmentUnavailable,
    #[error("draft attachment exceeds the supported size bound")]
    DraftAttachmentTooLarge,
    #[error("draft attachment metadata is invalid or the per-chat attachment limit was reached")]
    InvalidDraftAttachment,
    #[error("draft attachment not found: {0}")]
    DraftAttachmentNotFound(AttachmentId),
    #[error("history cursor message not found: {0}")]
    HistoryCursorNotFound(MessageId),
    #[error("artifact source must be an assistant message in the selected session: {0}")]
    ArtifactSourceInvalid(MessageId),
    #[error("artifact not found: {0}")]
    ArtifactNotFound(ArtifactId),
    #[error("artifact content exceeds the supported size bound")]
    ArtifactContentTooLarge,
    #[error("artifact count, version history, or retained bytes exceeded the supported bound")]
    ArtifactLimitExceeded,
    #[error("artifact metadata is invalid")]
    InvalidArtifact,
    #[error("storage mutex poisoned")]
    Poisoned,
}
impl From<rusqlite::Error> for SessionError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Storage(StorageError::Sqlite(error))
    }
}
#[cfg(test)]
mod manager_tests {
    use super::*;
    use opencode_rk_storage::SchemaV2;
    fn manager() -> (tempfile::TempDir, SessionManager) {
        let dir = tempfile::tempdir().unwrap();
        let conn =
            SchemaV2::initialize_workspace(&dir.path().join("w.db"), [1_u8; 16], [2_u8; 16], 10)
                .unwrap();
        (dir, SessionManager::new(conn))
    }
    #[test]
    fn create_and_get() {
        let (_dir, m) = manager();
        let id = m.create_session("hello").unwrap();
        let got = m.get_session(id).unwrap().unwrap();
        assert_eq!(got.id, id);
        assert_eq!(got.title, "hello");
        assert_eq!(got.state, SessionState::Active);
        assert!(m.get_session(SessionId::new()).unwrap().is_none());
    }
    #[test]
    fn list_sessions_ordered() {
        let (_dir, m) = manager();
        let a = m.create_session("a").unwrap();
        let b = m.create_session("b").unwrap();
        let c = m.create_session("c").unwrap();
        let list = m.list_sessions(10).unwrap();
        assert_eq!(list.iter().map(|s| s.id).collect::<Vec<_>>(), vec![c, b, a]);
        assert_eq!(m.list_sessions(2).unwrap().len(), 2);
    }
    #[test]
    fn rename_works() {
        let (_dir, m) = manager();
        let id = m.create_session("old").unwrap();
        m.rename_session(id, "new").unwrap();
        assert_eq!(m.get_session(id).unwrap().unwrap().title, "new");
        assert!(matches!(
            m.rename_session(SessionId::new(), "x"),
            Err(SessionError::NotFound(_))
        ));
    }
    #[test]
    fn archive_hides() {
        let (_dir, m) = manager();
        let id = m.create_session("gone").unwrap();
        m.archive_session(id).unwrap();
        assert!(m.list_sessions(10).unwrap().is_empty());
        let got = m.get_session(id).unwrap().unwrap();
        assert_eq!(got.state, SessionState::Archived);
        assert!(got.archived_at.is_some());
        assert!(matches!(
            m.archive_session(SessionId::new()),
            Err(SessionError::NotFound(_))
        ));
    }
    #[test]
    fn append_and_list_messages() {
        let (_dir, m) = manager();
        let id = m.create_session("chat").unwrap();
        let roles = [
            MessageRole::User,
            MessageRole::Assistant,
            MessageRole::User,
            MessageRole::Assistant,
            MessageRole::Tool,
        ];
        for (i, role) in roles.iter().enumerate() {
            m.append_message(id, *role, &format!("msg-{i}")).unwrap();
        }
        let messages = m.list_messages(id).unwrap();
        assert_eq!(messages.len(), 5);
        for (i, message) in messages.iter().enumerate() {
            assert_eq!(message.session_id, id);
            assert_eq!(message.role, roles[i]);
            assert!(matches!(&message.body,PayloadRef::Inline{text}if text==&format!("msg-{i}")));
        }
        assert_eq!(m.session_message_count(id).unwrap(), 5);
        assert!(matches!(
            m.append_message(SessionId::new(), MessageRole::User, "orphan"),
            Err(SessionError::NotFound(_))
        ));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    #[tokio::test]
    async fn lifecycle() {
        let temp = tempdir().unwrap();
        let storage = Arc::new(Storage::open_in_memory(temp.path().join("blobs")).unwrap());
        let first = SessionService::new(Arc::clone(&storage));
        let session = first.create("hello").await.unwrap();
        first
            .append_text(session.id, MessageRole::User, "message")
            .await
            .unwrap();
        let recreated = SessionService::new(storage);
        assert_eq!(recreated.get(session.id).await.unwrap().title, "hello");
        assert_eq!(recreated.messages(session.id, 50).await.unwrap().len(), 1);
    }
    #[tokio::test]
    async fn archive_hides_default() {
        let temp = tempdir().unwrap();
        let storage = Arc::new(Storage::open_in_memory(temp.path().join("blobs")).unwrap());
        let service = SessionService::new(storage);
        let session = service.create("archive me").await.unwrap();
        service.archive(session.id).await.unwrap();
        assert!(service.list(false).await.unwrap().is_empty());
        assert_eq!(service.list(true).await.unwrap().len(), 1);
    }
    #[tokio::test]
    async fn large_content_uses_blob() {
        let temp = tempdir().unwrap();
        let storage = Arc::new(Storage::open_in_memory(temp.path().join("blobs")).unwrap());
        let service = SessionService::new(storage);
        let session = service.create("blob").await.unwrap();
        let message = service
            .append_blob(session.id, MessageRole::Tool, vec![b'x'; 256 * 1024])
            .await
            .unwrap();
        assert!(matches!(message.body, PayloadRef::Blob { .. }));
    }
}
