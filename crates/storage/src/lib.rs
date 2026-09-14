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
    MessageId, MessageRecord, MessageRole, PayloadRef, SessionId, SessionState, SessionSummary,
    Timestamp, MAX_INLINE_PAYLOAD_BYTES,
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
        let (inline_text, blob_hash, byte_len) = match &message.body {
            PayloadRef::Inline { text } => {
                if text.len() > MAX_INLINE_PAYLOAD_BYTES {
                    return Err(StorageError::InlinePayloadTooLarge);
                }
                (Some(text.as_str()), None, text.len() as u64)
            }
            PayloadRef::Blob { hash, bytes } => (None, Some(hash.as_str()), *bytes),
        };
        let mut connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("INSERT INTO messages (id,session_id,role,inline_text,blob_hash,byte_len,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",params![message.id.to_string(),message.session_id.to_string(),encode_role(message.role),inline_text,blob_hash,byte_len as i64,message.created_at.to_string()])?;
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
        connection.execute_batch("CREATE TABLE IF NOT EXISTS schema_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY,title TEXT NOT NULL,state TEXT NOT NULL CHECK(state IN ('active','archived')),created_at TEXT NOT NULL,updated_at TEXT NOT NULL,archived_at TEXT); CREATE INDEX IF NOT EXISTS sessions_updated_idx ON sessions(updated_at DESC); CREATE TABLE IF NOT EXISTS messages(id TEXT PRIMARY KEY,session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,role TEXT NOT NULL,inline_text TEXT,blob_hash TEXT,byte_len INTEGER NOT NULL CHECK(byte_len>=0),created_at TEXT NOT NULL,CHECK((inline_text IS NULL)!=(blob_hash IS NULL))); CREATE INDEX IF NOT EXISTS messages_session_idx ON messages(session_id,created_at,id); CREATE TABLE IF NOT EXISTS recent_events(seq INTEGER PRIMARY KEY AUTOINCREMENT,session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,kind TEXT NOT NULL,payload_json TEXT NOT NULL,created_at TEXT NOT NULL); CREATE INDEX IF NOT EXISTS recent_events_session_idx ON recent_events(session_id,seq); INSERT INTO schema_meta(key,value) VALUES('schema_version','1') ON CONFLICT(key) DO UPDATE SET value=excluded.value;")?;
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
