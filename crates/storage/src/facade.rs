//! Public storage entry point: opens/initializes the v2 workspace DB, holds blobs, exposes writer/catalog APIs.
//! ponytail: BlobStore file layout not unified with v2 `blobs` table yet (payloads inline only). Upgrade: content-addressed blob spillover.
use crate::writer_v2::{NewMessage, NewSession};
use crate::{BlobStore, CatalogV2, SchemaV2, StorageError, V2Writer};
use opencode_rk_contracts::SessionId;
use rusqlite::{params, Connection, TransactionBehavior};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};
const MEMORY_SCHEMA_SQL: &str = include_str!("../schema/v2/workspace.sql");
const CONNECT_POLICY: &str = "PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL; PRAGMA busy_timeout=5000; PRAGMA trusted_schema=OFF; PRAGMA cache_size=-8192; PRAGMA temp_store=FILE; PRAGMA mmap_size=0;";
const STARTUP_RECOVERY_LIMIT: i64 = 500;
static FACADE_ID_COUNTER: AtomicU64 = AtomicU64::new(1);
pub struct StorageFacade {
    connection: Mutex<Connection>,
    blobs: BlobStore,
    path: Option<PathBuf>,
    writer: V2Writer,
    catalog: CatalogV2,
}
impl StorageFacade {
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let nonempty = path.exists() && fs::metadata(path)?.len() > 0;
        let connection = if nonempty {
            let mut connection = SchemaV2::open_existing(path)?;
            recover_existing(&mut connection)?;
            connection
        } else {
            SchemaV2::initialize_workspace(
                path,
                fresh_id_16(b"workspace"),
                fresh_id_16(b"epoch"),
                now_us(),
            )?
        };
        let blob_root = path
            .parent()
            .map(|p| {
                if p.as_os_str().is_empty() {
                    PathBuf::from("blobs")
                } else {
                    p.join("blobs")
                }
            })
            .unwrap_or_else(|| PathBuf::from("blobs"));
        fs::create_dir_all(&blob_root)?;
        Ok(Self {
            connection: Mutex::new(connection),
            blobs: BlobStore::new(blob_root),
            path: Some(path.to_path_buf()),
            writer: V2Writer,
            catalog: CatalogV2,
        })
    }
    pub fn open_memory() -> Result<Self, StorageError> {
        let mut connection = Connection::open_in_memory()?;
        init_memory(
            &mut connection,
            fresh_id_16(b"workspace"),
            fresh_id_16(b"epoch"),
            now_us(),
        )?;
        let dir = std::env::temp_dir().join(format!(
            "opencode-rk-facade-{}-{}-{}.blobs",
            std::process::id(),
            now_us(),
            FACADE_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir)?;
        Ok(Self {
            connection: Mutex::new(connection),
            blobs: BlobStore::new(dir),
            path: None,
            writer: V2Writer,
            catalog: CatalogV2,
        })
    }
    #[must_use]
    pub fn writer(&self) -> &V2Writer {
        &self.writer
    }
    #[must_use]
    pub fn catalog(&self) -> &CatalogV2 {
        &self.catalog
    }
    #[must_use]
    pub fn blob_store(&self) -> &BlobStore {
        &self.blobs
    }
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
    pub fn with_connection<F, T>(&self, f: F) -> Result<T, StorageError>
    where
        F: FnOnce(&Connection) -> Result<T, StorageError>,
    {
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        f(&connection)
    }
    pub fn create_session(&self, session: &NewSession) -> Result<(), StorageError> {
        let mut connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        V2Writer::create_session(&mut connection, session)
    }
    pub fn append_message(&self, message: &NewMessage) -> Result<(), StorageError> {
        let mut connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        V2Writer::append_message(&mut connection, message)
    }
    pub fn append_outbox_event(
        &self,
        session_id: SessionId,
        kind: &str,
        payload_json: &str,
    ) -> Result<(), StorageError> {
        let mut connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        V2Writer::append_outbox_event(&mut connection, session_id, kind, payload_json)
    }
    pub fn list_recent_sessions(
        &self,
        state: u8,
        before: Option<(i64, i64)>,
        limit: usize,
    ) -> Result<Vec<(i64, i64)>, StorageError> {
        let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
        V2Writer::list_recent_sessions(&connection, state, before, limit)
    }
    pub fn close(self) -> Result<(), StorageError> {
        {
            let connection = self.connection.lock().map_err(|_| StorageError::Poisoned)?;
            let _ =
                connection.execute("UPDATE workspace_state SET clean_shutdown=1 WHERE id=1", []);
            let _ = connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        }
        Ok(())
    }
}
fn recover_existing(connection: &mut Connection) -> Result<(), StorageError> {
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let (owner_generation, _clean_shutdown): (i64, i64) = tx.query_row(
        "SELECT owner_generation, clean_shutdown FROM workspace_state WHERE id=1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let next_generation = owner_generation.checked_add(1).ok_or_else(|| {
        StorageError::Sqlite(rusqlite::Error::InvalidParameterName(
            "workspace owner_generation overflow".to_owned(),
        ))
    })?;
    let changed = tx.execute(
        "UPDATE workspace_state
         SET owner_generation=?1, clean_shutdown=0
         WHERE id=1 AND owner_generation=?2",
        params![next_generation, owner_generation],
    )?;
    if changed != 1 {
        return Err(StorageError::Sqlite(rusqlite::Error::InvalidQuery));
    }

    // Select the same prior-generation roots for each child update before the
    // root state changes. The partial indexes keep every recovery scan bounded.
    tx.execute(
        "UPDATE provider_attempts
         SET state=4, finished_at_us=NULL
         WHERE pk IN (
             SELECT a.pk
             FROM provider_attempts AS a INDEXED BY attempts_recovery_idx
             JOIN executions AS e ON e.pk=a.execution_pk
             WHERE e.owner_generation < ?1
               AND e.state IN (0,1,4)
               AND e.state=1
               AND a.state IN (0,1,4)
               AND a.state IN (0,1)
             ORDER BY a.pk
             LIMIT ?2
         )",
        params![next_generation, STARTUP_RECOVERY_LIMIT],
    )?;
    tx.execute(
        "UPDATE tool_calls
         SET state=5, finished_at_us=NULL
         WHERE pk IN (
             SELECT t.pk
             FROM tool_calls AS t INDEXED BY tools_recovery_idx
             JOIN executions AS e ON e.pk=t.execution_pk
             WHERE e.owner_generation < ?1
               AND e.state IN (0,1,4)
               AND e.state=1
               AND t.state IN (0,1,5)
               AND t.state IN (0,1)
             ORDER BY t.pk
             LIMIT ?2
         )",
        params![next_generation, STARTUP_RECOVERY_LIMIT],
    )?;
    tx.execute(
        "UPDATE executions
         SET state=4, finished_at_us=NULL
         WHERE pk IN (
             SELECT e.pk
             FROM executions AS e INDEXED BY executions_recovery_idx
             WHERE e.owner_generation < ?1
               AND e.state IN (0,1,4)
               AND e.state=1
             ORDER BY e.pk
             LIMIT ?2
         )",
        params![next_generation, STARTUP_RECOVERY_LIMIT],
    )?;
    tx.commit()?;
    Ok(())
}
fn now_us() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}
// ponytail: storage has no rand/uuid dep; blake3(counter+time+pid) unique per process run. Upgrade: Uuid::new_v4.
fn fresh_id_16(tag: &[u8]) -> [u8; 16] {
    let n = FACADE_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut input = Vec::with_capacity(64);
    input.extend_from_slice(tag);
    input.extend_from_slice(&n.to_le_bytes());
    input.extend_from_slice(&now_us().to_le_bytes());
    input.extend_from_slice(&std::process::id().to_le_bytes());
    let hash = blake3::hash(&input);
    let mut id = [0u8; 16];
    id.copy_from_slice(&hash.as_bytes()[..16]);
    id
}
// Memory conns skip WAL verification (:memory: reports journal_mode=memory); same DDL/seed/markers as SchemaV2 otherwise.
fn init_memory(
    connection: &mut Connection,
    workspace_id: [u8; 16],
    cursor_epoch: [u8; 16],
    created_at_us: i64,
) -> Result<(), StorageError> {
    connection.execute_batch("PRAGMA page_size=4096; PRAGMA auto_vacuum=INCREMENTAL;")?;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute_batch(MEMORY_SCHEMA_SQL)?;
    tx.execute("INSERT INTO workspace_state (id, workspace_id, cursor_epoch, format_version, created_at_us) VALUES (?1, ?2, ?3, ?4, ?5)", params![1, &workspace_id[..], &cursor_epoch[..], 2, created_at_us])?;
    let checksum = SchemaV2::workspace_checksum();
    tx.execute(
        "INSERT INTO schema_migrations (version, checksum, applied_at_us) VALUES (?1, ?2, ?3)",
        params![2, &checksum[..], created_at_us],
    )?;
    tx.execute_batch("PRAGMA user_version = 2; PRAGMA application_id = 0x4F525732;")?;
    tx.commit()?;
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version != 2 {
        return Err(StorageError::Sqlite(rusqlite::Error::InvalidParameterName(
            format!("user_version mismatch: {version}"),
        )));
    }
    connection.execute_batch(CONNECT_POLICY)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use opencode_rk_contracts::{MessageId, MessageRole, PayloadRef};
    use std::sync::Arc;
    use tempfile::tempdir;
    fn session(title: &str, ts: i64) -> NewSession {
        NewSession {
            id: SessionId::new(),
            title: title.to_owned(),
            created_at_us: ts,
            updated_at_us: ts,
        }
    }
    fn message(session_id: SessionId, text: &str, ts: i64) -> NewMessage {
        NewMessage {
            id: MessageId::new(),
            session_id,
            role: MessageRole::User,
            body: PayloadRef::Inline {
                text: text.to_owned(),
            },
            created_at_us: ts,
        }
    }
    fn tables(facade: &StorageFacade) -> Vec<String> {
        facade
            .with_connection(|c| {
                let mut s =
                    c.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
                let mut v = Vec::new();
                for r in s.query_map([], |r| r.get::<_, String>(0))? {
                    v.push(r?);
                }
                Ok(v)
            })
            .unwrap()
    }
    #[test]
    fn open_and_init() {
        let dir = tempdir().unwrap();
        let facade = StorageFacade::open(&dir.path().join("ws.db")).unwrap();
        let found = tables(&facade);
        for t in [
            "sessions",
            "messages",
            "payloads",
            "blobs",
            "event_outbox",
            "workspace_state",
            "schema_migrations",
        ] {
            assert!(found.iter().any(|x| x == t), "missing {t}: {found:?}");
        }
    }
    #[test]
    fn write_and_read() {
        let dir = tempdir().unwrap();
        let facade = StorageFacade::open(&dir.path().join("ws.db")).unwrap();
        let s = session("hello", 1000);
        facade.create_session(&s).unwrap();
        facade.append_message(&message(s.id, "ping", 1001)).unwrap();
        facade
            .append_outbox_event(s.id, "test.event", "{}")
            .unwrap();
        assert_eq!(facade.list_recent_sessions(0, None, 10).unwrap().len(), 1);
        let body: Vec<u8> = facade.with_connection(|c| c.query_row("SELECT p.inline_data FROM messages m JOIN message_parts mp ON mp.message_pk=m.pk JOIN payloads p ON p.pk=mp.payload_pk WHERE m.session_pk=(SELECT pk FROM sessions WHERE id=?1)", params![s.id.as_uuid().as_bytes().as_slice()], |r| r.get(0)).map_err(StorageError::from)).unwrap();
        assert_eq!(body, b"ping");
        let events: i64 = facade
            .with_connection(|c| {
                c.query_row("SELECT COUNT(*) FROM event_outbox", [], |r| r.get(0))
                    .map_err(StorageError::from)
            })
            .unwrap();
        assert_eq!(events, 1);
    }
    #[test]
    fn reopen_persists() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("ws.db");
        {
            let facade = StorageFacade::open(&path).unwrap();
            facade.create_session(&session("keep", 1000)).unwrap();
            facade.close().unwrap();
        }
        let facade = StorageFacade::open(&path).unwrap();
        let titles: Vec<String> = facade
            .with_connection(|c| {
                let mut s = c.prepare("SELECT title FROM sessions")?;
                let mut v = Vec::new();
                for r in s.query_map([], |r| r.get(0))? {
                    v.push(r?);
                }
                Ok(v)
            })
            .unwrap();
        assert_eq!(titles, vec!["keep".to_owned()]);
    }
    #[test]
    fn open_memory_works() {
        let facade = StorageFacade::open_memory().unwrap();
        let s = session("mem", 1000);
        facade.create_session(&s).unwrap();
        facade.append_message(&message(s.id, "hi", 1001)).unwrap();
        assert_eq!(facade.list_recent_sessions(0, None, 10).unwrap().len(), 1);
        assert!(facade.path().is_none());
    }
    #[test]
    fn concurrent_access() {
        let facade = Arc::new(StorageFacade::open_memory().unwrap());
        let mut handles = Vec::new();
        for t in 0..2 {
            let f = Arc::clone(&facade);
            handles.push(std::thread::spawn(move || {
                for i in 0..25 {
                    let s = session(&format!("t{t}-{i}"), 1000 + i);
                    f.create_session(&s).unwrap();
                    f.append_message(&message(s.id, "x", 1001 + i)).unwrap();
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        let sessions: i64 = facade
            .with_connection(|c| {
                c.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
                    .map_err(StorageError::from)
            })
            .unwrap();
        let messages: i64 = facade
            .with_connection(|c| {
                c.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
                    .map_err(StorageError::from)
            })
            .unwrap();
        assert_eq!(sessions, 50);
        assert_eq!(messages, 50);
    }
}
