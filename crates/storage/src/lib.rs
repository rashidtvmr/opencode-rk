//! Embedded persistence with bounded relational state and content-addressed blobs.
#![forbid(unsafe_code)]
pub mod admission_v2;
pub mod approvals_v2;
pub mod backup_v2;
pub mod catalog_v2;
pub mod content_addr_v2;
pub mod execution_v2;
pub mod facade;
pub mod fork_v2;
pub mod gc_v2;
pub mod import_blobs;
pub mod import_v2;
pub mod migrations;
pub mod quota_v2;
pub mod retention_v2;
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
    sync::{Arc, Mutex},
};
use thiserror::Error;
pub use writer_v2::{NewMessage, NewSession, V2Writer};
pub const DEFAULT_MAX_EVENTS_PER_SESSION: usize = 10_000;
pub const MAX_EVENT_PAYLOAD_BYTES: usize = 64 * 1024;
pub const BLOB_COMPRESSION_LEVEL: i32 = 3;
const TYPED_HISTORY_FEATURE: &str = "typed_tool_history";
const TYPED_FIELD_LIMIT: usize = 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolRound {
    pub round_id: String,
    pub session_id: SessionId,
    pub turn_message_id: MessageId,
    pub round_ordinal: u32,
    pub expected_pairs: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolPair {
    pub call_id: String,
    pub name: String,
    pub input: PayloadRef,
    pub output: PayloadRef,
    pub message: MessageRecord,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolHistoryItem {
    pub round_id: Arc<str>,
    pub pair_index: u64,
    pub kind: String,
    pub call_id: String,
    pub name: String,
    pub payload: String,
    pub message_id: MessageId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HistoryItem {
    Message(MessageRecord),
    Tool(ToolHistoryItem),
}
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
    #[error(
        "unsupported SQLite engine {version} (source id {source_id}): WAL-reset race fix requires >= 3.51.3 or audited backport 3.44.6+/3.50.7+; refusing WAL mode fail-closed"
    )]
    UnsupportedSqliteEngine { version: String, source_id: String },
    #[error("typed tool history schema is incompatible")]
    TypedHistorySchema,
    #[error("typed tool history identity is invalid")]
    TypedHistoryIdentity,
    #[error("typed tool history pair is incomplete or inconsistent")]
    TypedHistoryIncomplete,
    #[error("typed tool history bound exceeded")]
    TypedHistoryLimit,
    #[error("typed tool history contains an unlinked legacy tool message")]
    UnlinkedToolHistory,
}
/// Minimum bundled engine for the WAL path.
///
/// SQLite documents a rare WAL-reset race (checkpointer vs. writer wrapping to
/// the start of the WAL) fixed in 3.51.3 and backported to 3.44.6 / 3.50.7
/// (`docs/storage/ENGINE_GATE.md`). `Storage::configure` enters WAL mode, so it
/// refuses to open on engines below this floor instead of silently running
/// unqualified. Numeric form of the `SQLITE_VERSION_NUMBER` scheme
/// (major * 1_000_000 + minor * 1_000 + patch).
pub const MIN_SQLITE_VERSION_NUMBER: i32 = 3_051_003;
/// Returns true when a numeric `SQLITE_VERSION_NUMBER`-style value meets the
/// WAL-reset gate. Backport releases are rejected until their exact
/// `sqlite_source_id()` values are audited and allowlisted.
fn sqlite_version_supported(version_number: i32) -> bool {
    version_number >= MIN_SQLITE_VERSION_NUMBER
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

    pub fn begin_tool_round(
        &self,
        session_id: SessionId,
        turn_message_id: MessageId,
        round_id: &str,
        round_ordinal: u32,
        expected_pairs: u32,
    ) -> Result<ToolRound, StorageError> {
        validate_typed_text(round_id)?;
        let mut connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let role: String = tx
            .query_row(
                "SELECT role FROM messages WHERE id=?1 AND session_id=?2",
                params![turn_message_id.to_string(), session_id.to_string()],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(StorageError::TypedHistoryIdentity)?;
        if role != "user" {
            return Err(StorageError::TypedHistoryIdentity);
        }
        tx.execute(
            "INSERT INTO tool_rounds(round_id,session_id,turn_message_id,round_ordinal,expected_pairs,created_at) VALUES(?1,?2,?3,?4,?5,?6)",
            params![round_id, session_id.to_string(), turn_message_id.to_string(), checked_i64(round_ordinal as u64)?, checked_i64(expected_pairs as u64)?, Timestamp::now().to_string()],
        )?;
        tx.commit()?;
        Ok(ToolRound {
            round_id: round_id.to_owned(),
            session_id,
            turn_message_id,
            round_ordinal,
            expected_pairs,
        })
    }

    pub fn append_tool_pair(
        &self,
        round: &ToolRound,
        pair_index: u64,
        pair: &ToolPair,
    ) -> Result<(), StorageError> {
        validate_typed_text(&pair.call_id)?;
        validate_typed_text(&pair.name)?;
        let input = payload_text_bounded(&self.blobs, &pair.input, TYPED_FIELD_LIMIT)?;
        let output = payload_text_bounded(&self.blobs, &pair.output, TYPED_FIELD_LIMIT)?;
        let mut connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (session, expected_pairs): (String, i64) = tx
            .query_row(
                "SELECT session_id, expected_pairs FROM tool_rounds WHERE round_id=?1",
                params![round.round_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or(StorageError::TypedHistoryIdentity)?;
        if session != round.session_id.to_string() {
            return Err(StorageError::TypedHistoryIdentity);
        }
        let expected_pairs =
            u64::try_from(expected_pairs).map_err(|_| StorageError::TypedHistoryIncomplete)?;
        if pair.message.role != MessageRole::Tool || pair.message.session_id.to_string() != session
        {
            return Err(StorageError::TypedHistoryIdentity);
        }
        let count: i64 = tx.query_row(
            "SELECT count(*) FROM typed_tool_records WHERE round_id=?1 AND pair_index=?2",
            params![round.round_id, checked_i64(pair_index)?],
            |row| row.get(0),
        )?;
        if count != 0 || pair_index >= expected_pairs {
            return Err(StorageError::TypedHistoryIncomplete);
        }
        let now = Timestamp::now().to_string();
        let message_text = match &pair.message.body {
            PayloadRef::Inline { text } if text.len() <= MAX_INLINE_PAYLOAD_BYTES => text,
            _ => return Err(StorageError::InlinePayloadTooLarge),
        };
        tx.execute(
            "INSERT INTO messages(id,session_id,role,inline_text,blob_hash,byte_len,created_at) VALUES(?1,?2,'tool',?3,NULL,?4,?5)",
            params![pair.message.id.to_string(), pair.message.session_id.to_string(), message_text, checked_i64(message_text.len() as u64)?, pair.message.created_at.to_string()],
        )?;
        for (kind, payload) in [("call", input), ("output", output)] {
            tx.execute(
                "INSERT INTO typed_tool_records(round_id,pair_index,kind,message_id,call_id,name,payload,byte_len,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![round.round_id, checked_i64(pair_index)?, kind, pair.message.id.to_string(), pair.call_id, pair.name, payload, payload.len() as i64, now],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn bounded_history(
        &self,
        session_id: SessionId,
        max_provider_items: usize,
        max_provider_bytes: usize,
    ) -> Result<Vec<HistoryItem>, StorageError> {
        if max_provider_items == 0 || max_provider_bytes == 0 {
            return Ok(Vec::new());
        }
        let mut connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let header_limit = i64::try_from(
            max_provider_items
                .checked_add(1)
                .ok_or(StorageError::TypedHistoryLimit)?,
        )
        .map_err(|_| StorageError::TypedHistoryLimit)?;
        let mut headers = tx.prepare(
            "SELECT rowid,length(CAST(round_id AS BLOB)),expected_pairs FROM tool_rounds WHERE session_id=?1 ORDER BY created_at,round_id LIMIT ?2",
        )?;
        let header_rows =
            headers.query_map(params![session_id.to_string(), header_limit], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?;
        let mut header_count = 0usize;
        let mut header_items = 0usize;
        for row in header_rows {
            header_count = header_count
                .checked_add(1)
                .ok_or(StorageError::TypedHistoryLimit)?;
            if header_count > max_provider_items {
                return Err(StorageError::TypedHistoryLimit);
            }
            let (round_rowid, round_id_len, expected_raw) = row?;
            let round_id_len =
                usize::try_from(round_id_len).map_err(|_| StorageError::TypedHistoryLimit)?;
            if round_id_len > max_provider_bytes {
                return Err(StorageError::TypedHistoryLimit);
            }
            let round_id: String = tx.query_row(
                "SELECT round_id FROM tool_rounds WHERE rowid=?1",
                params![round_rowid],
                |r| r.get(0),
            )?;
            let expected =
                u32::try_from(expected_raw).map_err(|_| StorageError::TypedHistoryIncomplete)?;
            let pair_items = usize::try_from(expected)
                .ok()
                .and_then(|n| n.checked_mul(2))
                .ok_or(StorageError::TypedHistoryLimit)?;
            header_items = header_items
                .checked_add(pair_items)
                .ok_or(StorageError::TypedHistoryLimit)?;
            if header_items > max_provider_items {
                return Err(StorageError::TypedHistoryLimit);
            }
            let (count, bad_domain, bad_identity): (i64, i64, i64) = tx.query_row(
                "SELECT count(*),
                        sum(CASE WHEN pair_index < 0 OR pair_index >= ?2 THEN 1 ELSE 0 END),
                        sum(CASE WHEN kind NOT IN ('call','output') THEN 1 ELSE 0 END)
                 FROM typed_tool_records WHERE round_id=?1",
                params![round_id, i64::from(expected)],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                        r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                    ))
                },
            )?;
            if expected > 0 && count == 0 {
                return Err(StorageError::TypedHistoryIncomplete);
            }
            if bad_domain != 0 || bad_identity != 0 || count != i64::from(expected) * 2 {
                return Err(StorageError::TypedHistoryIncomplete);
            }
            let mismatches: i64 = tx.query_row(
                "SELECT count(*) FROM (SELECT pair_index FROM typed_tool_records WHERE round_id=?1 GROUP BY pair_index HAVING count(*) != 2 OR count(DISTINCT kind) != 2 OR count(DISTINCT call_id) != 1 OR count(DISTINCT name) != 1 OR count(DISTINCT message_id) != 1)",
                params![round_id], |r| r.get(0),
            )?;
            if mismatches != 0 {
                return Err(StorageError::TypedHistoryIncomplete);
            }
        }
        drop(headers);
        let mut messages = tx.prepare(
            "SELECT rowid,length(CAST(id AS BLOB)),length(CAST(role AS BLOB)),length(CAST(inline_text AS BLOB)),blob_hash IS NOT NULL,byte_len
             FROM messages WHERE session_id=?1 ORDER BY rowid LIMIT ?2",
        )?;
        let lim = i64::try_from(
            max_provider_items
                .checked_add(1)
                .ok_or(StorageError::TypedHistoryLimit)?,
        )
        .map_err(|_| StorageError::TypedHistoryLimit)?;
        let rows = messages.query_map(params![session_id.to_string(), lim], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<i64>>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })?;
        let mut result = Vec::new();
        let mut used = 0usize;
        let mut emitted_rounds = std::collections::HashSet::new();
        let mut items_used = 0usize;
        let mut message_count = 0usize;
        for row in rows {
            message_count = message_count
                .checked_add(1)
                .ok_or(StorageError::TypedHistoryLimit)?;
            if message_count > max_provider_items {
                return Err(StorageError::TypedHistoryLimit);
            }
            let (message_rowid, id_len, role_len, inline_len, _has_blob, byte_len) = row?;
            let id_len =
                usize::try_from(id_len).map_err(|_| StorageError::TypedHistoryIncomplete)?;
            let role_len =
                usize::try_from(role_len).map_err(|_| StorageError::TypedHistoryIncomplete)?;
            let _validated_byte_len =
                usize::try_from(byte_len).map_err(|_| StorageError::TypedHistoryIncomplete)?;
            let metadata_bytes = id_len
                .checked_add(role_len)
                .ok_or(StorageError::TypedHistoryLimit)?;
            used = used
                .checked_add(metadata_bytes)
                .ok_or(StorageError::TypedHistoryLimit)?;
            if used > max_provider_bytes {
                return Err(StorageError::TypedHistoryLimit);
            }
            let (id, role, blob, created): (String, String, Option<String>, String) = tx
                .query_row(
                    "SELECT id,role,blob_hash,created_at FROM messages WHERE rowid=?1",
                    params![message_rowid],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )?;
            let message_id =
                MessageId::from_str(&id).map_err(|_| StorageError::TypedHistoryIdentity)?;
            if role == "tool" {
                let round: Option<(i64, i64, i64)> = tx.query_row(
                    "SELECT DISTINCT r.rowid,length(CAST(r.round_id AS BLOB)),r.expected_pairs FROM typed_tool_records t JOIN tool_rounds r ON r.round_id=t.round_id WHERE t.message_id=?1",
                    params![id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).optional()?;
                let Some((round_rowid, round_id_len, expected)) = round else {
                    return Err(StorageError::UnlinkedToolHistory);
                };
                let round_id_len = usize::try_from(round_id_len)
                    .map_err(|_| StorageError::TypedHistoryIncomplete)?;
                let round_id: String = tx.query_row(
                    "SELECT round_id FROM tool_rounds WHERE rowid=?1",
                    params![round_rowid],
                    |r| r.get(0),
                )?;
                if !emitted_rounds.insert(round_id.clone()) {
                    continue;
                }
                if used
                    .checked_add(round_id_len)
                    .ok_or(StorageError::TypedHistoryLimit)?
                    > max_provider_bytes
                {
                    return Err(StorageError::TypedHistoryLimit);
                }
                used = used
                    .checked_add(round_id_len)
                    .ok_or(StorageError::TypedHistoryLimit)?;
                let shared_round_id: Arc<str> = Arc::from(round_id);
                let expected =
                    u32::try_from(expected).map_err(|_| StorageError::TypedHistoryIncomplete)?;
                let pair_items = usize::try_from(expected)
                    .ok()
                    .and_then(|n| n.checked_mul(2))
                    .ok_or(StorageError::TypedHistoryLimit)?;
                items_used = items_used
                    .checked_add(pair_items)
                    .ok_or(StorageError::TypedHistoryLimit)?;
                if items_used > max_provider_items {
                    return Err(StorageError::TypedHistoryLimit);
                }
                let mut typed = tx.prepare(
                    "SELECT rowid,pair_index,CASE kind WHEN 'call' THEN 0 ELSE 1 END,length(CAST(call_id AS BLOB)),length(CAST(name AS BLOB)),length(CAST(message_id AS BLOB)),byte_len FROM typed_tool_records WHERE round_id=?1 ORDER BY CASE kind WHEN 'call' THEN 0 ELSE 1 END,pair_index LIMIT ?2",
                )?;
                let expected_count = usize::try_from(expected)
                    .ok()
                    .and_then(|v| v.checked_mul(2))
                    .ok_or(StorageError::TypedHistoryLimit)?;
                let mut record_count = 0usize;
                for row in typed.query_map(params![shared_round_id.as_ref(), lim], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                    ))
                })? {
                    record_count = record_count
                        .checked_add(1)
                        .ok_or(StorageError::TypedHistoryLimit)?;
                    if record_count > expected_count {
                        return Err(StorageError::TypedHistoryIncomplete);
                    }
                    let (
                        typed_rowid,
                        pair_index,
                        kind_code,
                        call_len,
                        name_len,
                        message_len,
                        byte_len,
                    ) = row?;
                    let call_len = usize::try_from(call_len)
                        .map_err(|_| StorageError::TypedHistoryIncomplete)?;
                    let name_len = usize::try_from(name_len)
                        .map_err(|_| StorageError::TypedHistoryIncomplete)?;
                    let message_len = usize::try_from(message_len)
                        .map_err(|_| StorageError::TypedHistoryIncomplete)?;
                    let actual = usize::try_from(byte_len)
                        .map_err(|_| StorageError::TypedHistoryIncomplete)?;
                    let metadata_bytes = call_len
                        .checked_add(name_len)
                        .and_then(|v| v.checked_add(message_len))
                        .ok_or(StorageError::TypedHistoryLimit)?;
                    used = used
                        .checked_add(metadata_bytes)
                        .and_then(|v| v.checked_add(actual))
                        .ok_or(StorageError::TypedHistoryLimit)?;
                    if used > max_provider_bytes {
                        return Err(StorageError::TypedHistoryLimit);
                    }
                    let (call_id, name, _record_message): (String, String, String) = tx.query_row(
                        "SELECT call_id,name,message_id FROM typed_tool_records WHERE rowid=?1",
                        params![typed_rowid],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                    )?;
                    let kind = if kind_code == 0 {
                        "call".to_string()
                    } else {
                        "output".to_string()
                    };
                    let payload: String = tx.query_row("SELECT payload FROM typed_tool_records WHERE round_id=?1 AND pair_index=?2 AND kind=?3 AND byte_len=length(CAST(payload AS BLOB))", params![shared_round_id.as_ref(), pair_index, kind], |row| row.get(0))?;
                    if payload.len() != actual {
                        return Err(StorageError::TypedHistoryIncomplete);
                    }
                    result.push(HistoryItem::Tool(ToolHistoryItem {
                        round_id: Arc::clone(&shared_round_id),
                        pair_index: u64::try_from(pair_index)
                            .map_err(|_| StorageError::TypedHistoryIncomplete)?,
                        kind,
                        call_id,
                        name,
                        payload,
                        message_id,
                    }));
                }
                if record_count != expected_count {
                    return Err(StorageError::TypedHistoryIncomplete);
                }
                continue;
            }
            items_used = items_used
                .checked_add(1)
                .ok_or(StorageError::TypedHistoryLimit)?;
            if items_used > max_provider_items {
                return Err(StorageError::TypedHistoryLimit);
            }
            let actual =
                usize::try_from(byte_len).map_err(|_| StorageError::TypedHistoryIncomplete)?;
            let declared = inline_len
                .or_else(|| blob.as_ref().map(|_| i64::try_from(actual).unwrap_or(-1)))
                .ok_or(StorageError::TypedHistoryIncomplete)?;
            if declared < 0 || declared != byte_len {
                return Err(StorageError::TypedHistoryIncomplete);
            }
            let body = match (inline_len, blob) {
                (Some(_), None) => {
                    used = used
                        .checked_add(actual)
                        .ok_or(StorageError::TypedHistoryLimit)?;
                    if used > max_provider_bytes {
                        return Err(StorageError::TypedHistoryLimit);
                    }
                    let text: String = tx.query_row("SELECT inline_text FROM messages WHERE id=?1 AND length(CAST(inline_text AS BLOB))=?2 AND byte_len=length(CAST(inline_text AS BLOB))", params![id, actual as i64], |row| row.get(0))?;
                    PayloadRef::Inline { text }
                }
                (None, Some(hash)) => {
                    let bytes = self
                        .blobs
                        .get_bounded(&hash, max_provider_bytes.saturating_sub(used))?;
                    if bytes.len() != actual {
                        return Err(StorageError::TypedHistoryIncomplete);
                    }
                    used = used
                        .checked_add(actual)
                        .ok_or(StorageError::TypedHistoryLimit)?;
                    if used > max_provider_bytes {
                        return Err(StorageError::TypedHistoryLimit);
                    }
                    PayloadRef::Blob {
                        hash,
                        bytes: actual as u64,
                    }
                }
                _ => return Err(StorageError::TypedHistoryIncomplete),
            };
            let role = decode_role(&role).map_err(StorageError::Sqlite)?;
            result.push(HistoryItem::Message(MessageRecord {
                id: message_id,
                session_id,
                role,
                body,
                created_at: parse_timestamp(created)?,
            }));
        }
        drop(messages);
        tx.commit()?;
        Ok(result)
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
        Self::require_supported_engine(connection)?;
        connection.execute_batch("PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000; PRAGMA synchronous=NORMAL; PRAGMA auto_vacuum=INCREMENTAL;")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        Ok(())
    }
    /// Fail-closed WAL-reset engine gate (`docs/STORAGE.md:164-168`).
    ///
    /// Runs before any PRAGMA write or schema creation. Refuses engines below
    /// the fixed floor with [`StorageError::UnsupportedSqliteEngine`] instead
    /// of silently entering WAL mode.
    ///
    /// Durability boundary: this bootstrap path uses `synchronous=NORMAL` and
    /// relies on same-host WAL filesystem semantics; format-2 writers use
    /// `synchronous=FULL` (`SchemaV2`/`CatalogV2`). The bundled engine
    /// (rusqlite `bundled`, currently 3.53.x) is production qualification; the
    /// local Python test engine (3.46.x, affected range) is DDL-test only.
    fn require_supported_engine(connection: &Connection) -> Result<(), StorageError> {
        let version_number = rusqlite::version_number();
        if sqlite_version_supported(version_number) {
            return Ok(());
        }
        let source_id: String = connection
            .query_row("SELECT sqlite_source_id()", [], |row| row.get(0))
            .unwrap_or_else(|_| "unknown".to_owned());
        Err(StorageError::UnsupportedSqliteEngine {
            version: rusqlite::version().to_owned(),
            source_id,
        })
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
        Self::migrate_typed_history(connection)?;
        Ok(())
    }

    fn migrate_typed_history(connection: &Connection) -> Result<(), StorageError> {
        connection.execute_batch("BEGIN IMMEDIATE;")?;
        let result = (|| -> Result<(), StorageError> {
            if let Some((kind, sql)) = connection
                .query_row(
                    "SELECT type,sql FROM sqlite_master WHERE name=?1",
                    params!["feature_migrations"],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
                )
                .optional()?
            {
                if kind != "table" || sql.as_deref().unwrap_or("").to_ascii_lowercase().replace(' ', "") != "createtablefeature_migrations(featuretextprimarykey,versionintegernotnullcheck(version>=1),applied_attextnotnull)" { return Err(StorageError::TypedHistorySchema); }
            } else {
                connection.execute_batch("CREATE TABLE feature_migrations(feature TEXT PRIMARY KEY,version INTEGER NOT NULL CHECK(version >= 1),applied_at TEXT NOT NULL);")?;
            }
            if let Some(version) = connection
                .query_row(
                    "SELECT version FROM feature_migrations WHERE feature=?1",
                    params![TYPED_HISTORY_FEATURE],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?
            {
                if version != 1 {
                    return Err(StorageError::TypedHistorySchema);
                }
            }
            let ledger_type: Option<String> = connection
                .query_row(
                    "SELECT typeof(version) FROM feature_migrations WHERE feature=?1",
                    params![TYPED_HISTORY_FEATURE],
                    |row| row.get(0),
                )
                .optional()?;
            if ledger_type.as_deref().is_some_and(|kind| kind != "integer") {
                return Err(StorageError::TypedHistorySchema);
            }
            let expected_round = "CREATE TABLE tool_rounds(round_id TEXT PRIMARY KEY,session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,turn_message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,round_ordinal INTEGER NOT NULL CHECK(round_ordinal >= 0),expected_pairs INTEGER NOT NULL CHECK(expected_pairs >= 0),created_at TEXT NOT NULL,UNIQUE(turn_message_id,round_ordinal))";
            let expected_record = "CREATE TABLE typed_tool_records(round_id TEXT NOT NULL REFERENCES tool_rounds(round_id) ON DELETE CASCADE,pair_index INTEGER NOT NULL CHECK(pair_index >= 0),kind TEXT NOT NULL CHECK(kind IN ('call','output')),message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,call_id TEXT NOT NULL CHECK(typeof(call_id)='text' AND length(CAST(call_id AS BLOB)) BETWEEN 1 AND 1048576),name TEXT NOT NULL CHECK(typeof(name)='text' AND length(CAST(name AS BLOB)) BETWEEN 1 AND 1048576),payload TEXT NOT NULL CHECK(typeof(payload)='text' AND length(CAST(payload AS BLOB)) BETWEEN 0 AND 1048576),byte_len INTEGER NOT NULL CHECK(byte_len >= 0 AND byte_len = length(CAST(payload AS BLOB))),created_at TEXT NOT NULL,PRIMARY KEY(round_id,pair_index,kind),UNIQUE(round_id,kind,call_id))";
            for (name, expected) in [
                ("tool_rounds", expected_round),
                ("typed_tool_records", expected_record),
            ] {
                if let Some(sql) = connection
                    .query_row(
                        "SELECT sql FROM sqlite_master WHERE type='table' AND name=?1",
                        params![name],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?
                {
                    if sql
                        .to_ascii_lowercase()
                        .split_whitespace()
                        .collect::<String>()
                        != expected
                            .to_ascii_lowercase()
                            .split_whitespace()
                            .collect::<String>()
                    {
                        return Err(StorageError::TypedHistorySchema);
                    }
                } else {
                    connection.execute_batch(expected)?;
                }
            }
            validate_typed_index(
                connection,
                "tool_rounds_session_idx",
                false,
                &["session_id", "created_at", "round_id"],
            )?;
            validate_typed_index(
                connection,
                "typed_tool_round_idx",
                false,
                &["round_id", "pair_index", "kind"],
            )?;
            validate_typed_index(
                connection,
                "tool_rounds_identity_idx",
                true,
                &["round_id", "turn_message_id", "round_ordinal"],
            )?;
            connection.execute_batch("CREATE INDEX IF NOT EXISTS tool_rounds_session_idx ON tool_rounds(session_id,created_at,round_id); CREATE INDEX IF NOT EXISTS typed_tool_round_idx ON typed_tool_records(round_id,pair_index,kind); CREATE UNIQUE INDEX IF NOT EXISTS tool_rounds_identity_idx ON tool_rounds(round_id,turn_message_id,round_ordinal);")?;
            validate_typed_index(
                connection,
                "tool_rounds_session_idx",
                false,
                &["session_id", "created_at", "round_id"],
            )?;
            validate_typed_index(
                connection,
                "typed_tool_round_idx",
                false,
                &["round_id", "pair_index", "kind"],
            )?;
            validate_typed_index(
                connection,
                "tool_rounds_identity_idx",
                true,
                &["round_id", "turn_message_id", "round_ordinal"],
            )?;
            connection.execute("INSERT INTO feature_migrations(feature,version,applied_at) VALUES(?1,1,?2) ON CONFLICT(feature) DO NOTHING", params![TYPED_HISTORY_FEATURE, Timestamp::now().to_string()])?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                connection.execute_batch("COMMIT")?;
                Ok(())
            }
            Err(error) => {
                let _ = connection.execute_batch("ROLLBACK");
                Err(error)
            }
        }
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
    pub fn get_bounded(&self, hash: &str, limit: usize) -> Result<Vec<u8>, StorageError> {
        let path = self.path_for(hash)?;
        let file = fs::File::open(path)?;
        let decoder = zstd::Decoder::new(file)?;
        let mut bytes = Vec::new();
        decoder
            .take(
                u64::try_from(
                    limit
                        .checked_add(1)
                        .ok_or(StorageError::TypedHistoryLimit)?,
                )
                .map_err(|_| StorageError::TypedHistoryLimit)?,
            )
            .read_to_end(&mut bytes)?;
        if bytes.len() > limit {
            return Err(StorageError::TypedHistoryLimit);
        }
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
fn checked_i64(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::TypedHistoryLimit)
}
fn validate_typed_text(value: &str) -> Result<(), StorageError> {
    if value.is_empty() || value.len() > TYPED_FIELD_LIMIT {
        return Err(StorageError::TypedHistoryLimit);
    }
    Ok(())
}
fn payload_text_bounded(
    blobs: &BlobStore,
    payload: &PayloadRef,
    limit: usize,
) -> Result<String, StorageError> {
    match payload {
        PayloadRef::Inline { text } => {
            if text.len() > limit {
                return Err(StorageError::TypedHistoryLimit);
            }
            Ok(text.clone())
        }
        PayloadRef::Blob { hash, bytes } => {
            if *bytes > limit as u64 {
                return Err(StorageError::TypedHistoryLimit);
            }
            let data = blobs.get_bounded(hash, limit)?;
            if data.len() as u64 != *bytes {
                return Err(StorageError::BlobHashMismatch);
            }
            String::from_utf8(data).map_err(|_| StorageError::TypedHistoryIdentity)
        }
    }
}
fn validate_typed_index(
    connection: &Connection,
    name: &str,
    unique: bool,
    columns: &[&str],
) -> Result<(), StorageError> {
    let table = if name == "typed_tool_round_idx" {
        "typed_tool_records"
    } else {
        "tool_rounds"
    };
    let existing: Option<(String, String)> = connection
        .query_row(
            "SELECT type,tbl_name FROM sqlite_master WHERE name=?1",
            params![name],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if let Some((kind, table_name)) = existing {
        if kind != "index" || table_name != table {
            return Err(StorageError::TypedHistorySchema);
        }
    }
    let found: Option<i64> = connection
        .query_row(
            "SELECT `unique` FROM pragma_index_list(?1) WHERE name=?2",
            params![table, name],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(flag) = found {
        let mut statement =
            connection.prepare("SELECT name FROM pragma_index_info(?1) ORDER BY seqno")?;
        let mut actual: Vec<String> = statement
            .query_map(params![name], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        let mut expected: Vec<String> = columns.iter().map(|s| (*s).to_owned()).collect();
        actual.sort();
        expected.sort();
        if (flag != 0) != unique || actual != expected {
            return Err(StorageError::TypedHistorySchema);
        }
    }
    Ok(())
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
