//! Format-2 installation catalog initializer.
//! NEW EMPTY FILE ONLY: never overwrites an existing database.
//!
//! ponytail: docs/STORAGE.md specifies SHA-256. `sha2` is not a workspace
//! dependency. Using blake3-256 (32-byte digest, already a dep) as the
//! migration checksum. Upgrade path: add `sha2` to workspace deps when
//! engine qualification lands, then swap the digest computation.
#![forbid(unsafe_code)]

use std::path::Path;

use blake3::hash as blake3_hash;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::StorageError;

const CATALOG_SQL: &str = include_str!("../schema/v2/catalog.sql");
const APP_ID: i64 = 0x4F524332; // 0x4F524332 is "ORC2", decimal 1330791218.
const USER_VERSION: i64 = 2;
const FORMAT_VERSION: i64 = 2;
const MIGRATION_VERSION: i64 = 2;

const PRAGMA_INIT: &str =
    "PRAGMA page_size=4096; PRAGMA auto_vacuum=INCREMENTAL; PRAGMA journal_mode=WAL;";

const CONNECT_POLICY: &str = "PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL; PRAGMA busy_timeout=5000; PRAGMA trusted_schema=OFF; PRAGMA cache_size=-8192; PRAGMA temp_store=FILE; PRAGMA mmap_size=0;";

pub struct CatalogV2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceRegistration {
    pub id: [u8; 16],
    pub label: String,
    pub project_root: Option<String>,
    pub status: u8,
    pub created_at_us: i64,
}

impl CatalogV2 {
    pub fn initialize_catalog(
        path: &Path,
        installation_id: [u8; 16],
        created_at_us: i64,
    ) -> Result<Connection, StorageError> {
        // fail-closed: refuse non-empty existing files
        if path.exists() {
            let meta = std::fs::metadata(path)?;
            if meta.len() > 0 {
                return Err(StorageError::Sqlite(rusqlite::Error::InvalidParameterName(
                    "file exists and is non-empty".into(),
                )));
            }
        }

        let mut conn = Connection::open(path)?;
        conn.execute_batch(PRAGMA_INIT)?;

        // verify WAL
        let journal: String = conn.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
        if journal.to_lowercase() != "wal" {
            return Err(StorageError::Sqlite(rusqlite::Error::InvalidParameterName(
                format!("journal_mode is not wal: {journal}"),
            )));
        }

        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(CATALOG_SQL)?;
        tx.execute(
            "INSERT INTO catalog_state (id, installation_id, format_version) VALUES (?1, ?2, ?3)",
            params![1, &installation_id[..], FORMAT_VERSION],
        )?;
        let checksum = Self::catalog_checksum();
        tx.execute(
            "INSERT INTO schema_migrations (version, checksum, applied_at_us) VALUES (?1, ?2, ?3)",
            params![MIGRATION_VERSION, &checksum[..], created_at_us],
        )?;

        // PRAGMA user_version and application_id are transactional in SQLite
        // and persist on commit; no post-commit fallback needed.
        tx.execute_batch("PRAGMA user_version = 2; PRAGMA application_id = 0x4F524332;")?;
        tx.commit()?;

        // verify user_version and application_id persisted
        let actual_user_version: i64 =
            conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if actual_user_version != USER_VERSION {
            return Err(StorageError::Sqlite(rusqlite::Error::InvalidParameterName(
                format!(
                    "user_version mismatch after commit: expected {USER_VERSION}, got {actual_user_version}"
                ),
            )));
        }
        let actual_app_id: i64 =
            conn.pragma_query_value(None, "application_id", |row| row.get(0))?;
        if actual_app_id != APP_ID {
            return Err(StorageError::Sqlite(rusqlite::Error::InvalidParameterName(
                format!(
                    "application_id mismatch after commit: expected {APP_ID:#x}, got {actual_app_id:#x}"
                ),
            )));
        }

        // apply full connection policy (foreign_keys, synchronous=FULL, etc.)
        conn.execute_batch(CONNECT_POLICY)?;
        // final WAL verification
        let journal: String = conn.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
        if journal.to_lowercase() != "wal" {
            return Err(StorageError::Sqlite(rusqlite::Error::InvalidParameterName(
                format!("journal_mode is not wal: {journal}"),
            )));
        }
        Ok(conn)
    }

    pub fn open_existing(path: &Path) -> Result<Connection, StorageError> {
        let conn = Connection::open(path)?;

        // verify application_id
        let app_id: i64 = conn.pragma_query_value(None, "application_id", |row| row.get(0))?;
        if app_id != APP_ID {
            return Err(StorageError::Sqlite(rusqlite::Error::InvalidParameterName(
                format!("application_id mismatch: expected {APP_ID:#x}, got {app_id:#x}"),
            )));
        }

        // verify user_version
        let user_version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if user_version != USER_VERSION {
            return Err(StorageError::Sqlite(rusqlite::Error::InvalidParameterName(
                format!("user_version mismatch: expected {USER_VERSION}, got {user_version}"),
            )));
        }

        // verify schema_migrations version + checksum
        let (stored_version, stored_checksum): (i64, Vec<u8>) = conn.query_row(
            "SELECT version, checksum FROM schema_migrations WHERE version = 2",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?)),
        )?;
        if stored_version != MIGRATION_VERSION {
            return Err(StorageError::Sqlite(rusqlite::Error::InvalidParameterName(
                format!(
                    "schema_migrations version mismatch: expected {MIGRATION_VERSION}, got {stored_version}"
                ),
            )));
        }
        let computed = Self::catalog_checksum();
        if stored_checksum != computed.to_vec() {
            return Err(StorageError::Sqlite(rusqlite::Error::InvalidParameterName(
                format!(
                    "checksum mismatch: stored {}, computed {}",
                    Self::hex_str(&stored_checksum),
                    Self::hex_str(&computed)
                ),
            )));
        }

        // apply connection policy
        conn.execute_batch(CONNECT_POLICY)?;

        // re-verify WAL after configuring
        let journal: String = conn.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
        if journal.to_lowercase() != "wal" {
            return Err(StorageError::Sqlite(rusqlite::Error::InvalidParameterName(
                format!("journal_mode is not wal: {journal}"),
            )));
        }
        Ok(conn)
    }

    /// Computes the migration checksum from the exact embedded SQL bytes.
    pub fn catalog_checksum() -> [u8; 32] {
        let hash = blake3_hash(CATALOG_SQL.as_bytes());
        let mut arr = [0_u8; 32];
        arr.copy_from_slice(hash.as_bytes());
        arr
    }

    pub fn register_workspace(
        conn: &mut Connection,
        reg: &WorkspaceRegistration,
    ) -> Result<i64, StorageError> {
        if reg.label.len() > 1024 {
            return Err(invalid_input("workspace label exceeds 1024 bytes"));
        }
        if reg
            .project_root
            .as_ref()
            .is_some_and(|project_root| project_root.len() > 4096)
        {
            return Err(invalid_input("workspace project_root exceeds 4096 bytes"));
        }
        if reg.status > 3 {
            return Err(invalid_input("workspace status must be between 0 and 3"));
        }

        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO workspaces (id, label, project_root, status, created_at_us) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                &reg.id[..],
                &reg.label,
                reg.project_root.as_deref(),
                reg.status,
                reg.created_at_us
            ],
        )?;
        let rowid = tx.last_insert_rowid();
        tx.commit()?;
        Ok(rowid)
    }

    pub fn list_workspaces(
        conn: &Connection,
        limit: usize,
    ) -> Result<Vec<WorkspaceRegistration>, StorageError> {
        let limit = limit.clamp(1, 500) as i64;
        let mut statement = conn.prepare(
            "SELECT id, label, project_root, status, created_at_us FROM workspaces ORDER BY pk ASC LIMIT ?1",
        )?;
        let rows = statement.query_map([limit], |row| {
            let id_bytes: Vec<u8> = row.get(0)?;
            let id: [u8; 16] = id_bytes
                .try_into()
                .map_err(|_| rusqlite::Error::InvalidQuery)?;
            let status: u8 = row.get(3)?;
            Ok(WorkspaceRegistration {
                id,
                label: row.get(1)?,
                project_root: row.get(2)?,
                status,
                created_at_us: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    pub fn put_setting(
        conn: &mut Connection,
        key: &str,
        value_json: &str,
        updated_at_us: i64,
    ) -> Result<(), StorageError> {
        if !(1..=128).contains(&key.len()) {
            return Err(invalid_input("setting key must be between 1 and 128 bytes"));
        }
        if value_json.len() > 4096 || serde_json::from_str::<serde_json::Value>(value_json).is_err()
        {
            return Err(invalid_input(
                "setting value_json must be valid JSON and at most 4096 bytes",
            ));
        }

        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO app_settings (key, value_json, revision, updated_at_us) VALUES (?1, ?2, 1, ?3) ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, revision = app_settings.revision + 1, updated_at_us = excluded.updated_at_us",
            params![key, value_json, updated_at_us],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn get_setting(
        conn: &Connection,
        key: &str,
    ) -> Result<Option<(String, i64)>, StorageError> {
        conn.query_row(
            "SELECT value_json, revision FROM app_settings WHERE key = ?1",
            [key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(StorageError::from)
    }

    fn hex_str(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

fn invalid_input(message: &str) -> StorageError {
    StorageError::Sqlite(rusqlite::Error::InvalidParameterName(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn catalog_smoke() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("catalog.db");
        let mut conn = CatalogV2::initialize_catalog(&path, [7_u8; 16], 10).unwrap();
        assert_eq!(
            conn.pragma_query_value(None, "application_id", |row| row.get::<_, i64>(0))
                .unwrap(),
            APP_ID
        );
        assert!(CatalogV2::initialize_catalog(&path, [8_u8; 16], 11).is_err());

        drop(conn);
        let mut conn = CatalogV2::open_existing(&path).unwrap();
        assert_eq!(
            conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            USER_VERSION
        );

        let registration = WorkspaceRegistration {
            id: [1_u8; 16],
            label: "demo".to_owned(),
            project_root: Some("/tmp/demo".to_owned()),
            status: 1,
            created_at_us: 12,
        };
        CatalogV2::register_workspace(&mut conn, &registration).unwrap();
        assert_eq!(
            CatalogV2::list_workspaces(&conn, 0).unwrap(),
            vec![registration]
        );

        CatalogV2::put_setting(&mut conn, "theme", r#"{"dark":true}"#, 13).unwrap();
        assert_eq!(
            CatalogV2::get_setting(&conn, "theme").unwrap(),
            Some((r#"{"dark":true}"#.to_owned(), 1))
        );
    }
}
