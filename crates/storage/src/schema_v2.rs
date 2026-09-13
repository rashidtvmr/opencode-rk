//! Format-2 workspace schema initializer.
//! NEW EMPTY FILE ONLY: never overwrites a format-1 database.
//!
//! ponytail: docs/STORAGE.md specifies SHA-256. `sha2` is not a workspace
//! dependency. Using blake3-256 (32-byte digest, already a dep) as the
//! migration checksum. Upgrade path: add `sha2` to workspace deps when
//! engine qualification lands, then swap the digest computation.
use std::path::Path;
use blake3::hash as blake3_hash;
use rusqlite::{params, Connection, TransactionBehavior};

const WORKSPACE_SQL: &str = include_str!("../schema/v2/workspace.sql");
const APP_ID: i64 = 0x4F525732;
const USER_VERSION: i64 = 2;
const FORMAT_VERSION: i64 = 2;
const MIGRATION_VERSION: i64 = 2;

const PRAGMA_INIT: &str = "PRAGMA page_size=4096; PRAGMA auto_vacuum=INCREMENTAL; PRAGMA journal_mode=WAL;";

const CONNECT_POLICY: &str = "PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL; PRAGMA busy_timeout=5000; PRAGMA trusted_schema=OFF; PRAGMA cache_size=-8192; PRAGMA temp_store=FILE; PRAGMA mmap_size=0;";

pub struct SchemaV2;

impl SchemaV2 {
    pub fn initialize_workspace(
        path: &Path,
        workspace_id: [u8; 16],
        cursor_epoch: [u8; 16],
        created_at_us: i64,
    ) -> Result<Connection, crate::StorageError> {
        // fail-closed: refuse non-empty existing files
        if path.exists() {
            let meta = std::fs::metadata(path)?;
            if meta.len() > 0 {
                return Err(crate::StorageError::Sqlite(
                    rusqlite::Error::InvalidParameterName("file exists and is non-empty".into()),
                ));
            }
        }
        let mut conn = Connection::open(path)?;
        conn.execute_batch(PRAGMA_INIT)?;
        // verify WAL
        let journal: String =
            conn.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
        if journal.to_lowercase() != "wal" {
            return Err(crate::StorageError::Sqlite(rusqlite::Error::InvalidParameterName(format!(
                "journal_mode is not wal: {journal}"
            ))));
        }
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(WORKSPACE_SQL)?;
        tx.execute(
            "INSERT INTO workspace_state (id, workspace_id, cursor_epoch, format_version, created_at_us) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![1, &workspace_id[..], &cursor_epoch[..], FORMAT_VERSION, created_at_us],
        )?;
        let checksum = Self::workspace_checksum();
        tx.execute(
            "INSERT INTO schema_migrations (version, checksum, applied_at_us) VALUES (?1, ?2, ?3)",
            params![MIGRATION_VERSION, &checksum[..], created_at_us],
        )?;
        // PRAGMA user_version and application_id are transactional in SQLite
        // and persist on commit; no post-commit fallback needed.
        tx.execute_batch("PRAGMA user_version = 2; PRAGMA application_id = 0x4F525732;")?;
        tx.commit()?;
        // verify user_version and application_id persisted
        let actual_user_version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if actual_user_version != USER_VERSION {
            return Err(crate::StorageError::Sqlite(rusqlite::Error::InvalidParameterName(format!(
                "user_version mismatch after commit: expected {USER_VERSION}, got {actual_user_version}"
            ))));
        }
        let actual_app_id: i64 = conn.pragma_query_value(None, "application_id", |row| row.get(0))?;
        if actual_app_id != APP_ID {
            return Err(crate::StorageError::Sqlite(rusqlite::Error::InvalidParameterName(format!(
                "application_id mismatch after commit: expected {APP_ID:#x}, got {actual_app_id:#x}"
            ))));
        }
        // apply full connection policy (foreign_keys, synchronous=FULL, etc.)
        conn.execute_batch(CONNECT_POLICY)?;
        // final WAL verification
        let journal: String = conn.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
        if journal.to_lowercase() != "wal" {
            return Err(crate::StorageError::Sqlite(rusqlite::Error::InvalidParameterName(format!(
                "journal_mode is not wal: {journal}"
            ))));
        }
        Ok(conn)
    }

    pub fn open_existing(path: &Path) -> Result<Connection, crate::StorageError> {
        let conn = Connection::open(path)?;
        // verify application_id
        let app_id: i64 = conn.pragma_query_value(None, "application_id", |row| row.get(0))?;
        if app_id != APP_ID {
            return Err(crate::StorageError::Sqlite(rusqlite::Error::InvalidParameterName(format!(
                "application_id mismatch: expected {APP_ID:#x}, got {app_id:#x}"
            ))));
        }
        // verify user_version
        let user_version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if user_version != USER_VERSION {
            return Err(crate::StorageError::Sqlite(rusqlite::Error::InvalidParameterName(format!(
                "user_version mismatch: expected {USER_VERSION}, got {user_version}"
            ))));
        }
        // verify schema_migrations version + checksum
        let (stored_version, stored_checksum): (i64, Vec<u8>) = conn.query_row(
            "SELECT version, checksum FROM schema_migrations WHERE version = 2",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?)),
        )?;
        if stored_version != MIGRATION_VERSION {
            return Err(crate::StorageError::Sqlite(rusqlite::Error::InvalidParameterName(format!(
                "schema_migrations version mismatch: expected {MIGRATION_VERSION}, got {stored_version}"
            ))));
        }
        let computed = Self::workspace_checksum();
        if stored_checksum != computed.to_vec() {
            return Err(crate::StorageError::Sqlite(rusqlite::Error::InvalidParameterName(format!(
                "checksum mismatch: stored {}, computed {}",
                Self::hex_str(&stored_checksum),
                Self::hex_str(&computed)
            ))));
        }
        // apply connection policy
        conn.execute_batch(CONNECT_POLICY)?;
        // re-verify WAL after configuring
        let journal: String = conn.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
        if journal.to_lowercase() != "wal" {
            return Err(crate::StorageError::Sqlite(rusqlite::Error::InvalidParameterName(format!(
                "journal_mode is not wal: {journal}"
            ))));
        }
        Ok(conn)
    }

    pub fn workspace_checksum() -> [u8; 32] {
        let hash = blake3_hash(WORKSPACE_SQL.as_bytes());
        let mut arr = [0u8; 32];
        arr.copy_from_slice(hash.as_bytes());
        arr
    }

    fn hex_str(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn init_then_reopen_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("workspace.db");
        let conn = SchemaV2::initialize_workspace(&path, [1_u8; 16], [2_u8; 16], 10).unwrap();
        drop(conn);
        let reopened = SchemaV2::open_existing(&path).unwrap();
        assert_eq!(
            reopened.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0)).unwrap(),
            2
        );
    }

    #[test]
    fn nonempty_file_rejected() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("not-a-workspace.db");
        let original = b"pre-existing bytes";
        fs::write(&path, original).unwrap();
        assert!(
            SchemaV2::initialize_workspace(&path, [1_u8; 16], [2_u8; 16], 10).is_err(),
            "non-empty files must fail closed"
        );
        assert_eq!(fs::read(&path).unwrap(), original);
    }

    #[test]
    fn tampered_checksum_rejected() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("workspace.db");
        let conn = SchemaV2::initialize_workspace(&path, [1_u8; 16], [2_u8; 16], 10).unwrap();
        conn.execute(
            "UPDATE schema_migrations SET checksum=?1 WHERE version=2",
            params![vec![0_u8; 32]],
        )
        .unwrap();
        drop(conn);
        assert!(
            SchemaV2::open_existing(&path).is_err(),
            "checksum verification is a readiness gate"
        );
    }
}