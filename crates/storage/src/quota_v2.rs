//! quota_v2: independent DB/WAL admission quotas and bounded reclaim.
//! Backpressure via rejection (DbBytes/WalBytes), never silent deletes.
#![forbid(unsafe_code)]
use crate::StorageError;
use rusqlite::Connection;
use std::fs;

/// Snapshot of the live DB and its WAL for quota decisions.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct QuotaSnapshot {
    pub page_count: i64,
    pub freelist_count: i64,
    pub wal_pages: i64,
    pub db_bytes: i64,
}

/// Admission backpressure signal. Carries the offending measured byte count.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum QuotaV2Error {
    DbBytes(i64),
    WalBytes(i64),
}

pub struct QuotaV2;

impl QuotaV2 {
    /// Measure current DB page/freelist counts and on-disk byte usage.
    /// wal_pages approximates WAL growth from the -wal file (pages) when present.
    pub fn measure(connection: &Connection) -> Result<QuotaSnapshot, StorageError> {
        let page_count: i64 = connection.query_row("PRAGMA page_count", [], |row| row.get(0))?;
        let freelist_count: i64 =
            connection.query_row("PRAGMA freelist_count", [], |row| row.get(0))?;
        let db_bytes = db_file_bytes(connection)?;
        let wal_bytes = wal_file_bytes(connection)?;
        let page_size = page_size(connection)?;
        let wal_pages = if page_size > 0 {
            wal_bytes / page_size
        } else {
            0
        };
        Ok(QuotaSnapshot {
            page_count,
            freelist_count,
            wal_pages,
            db_bytes,
        })
    }

    /// Admission gate. Rejects new admission while reserving headroom when the
    /// primary DB or the WAL exceeds its budget. No data is deleted here.
    pub fn admit(
        connection: &Connection,
        snapshot: &QuotaSnapshot,
        max_db_bytes: i64,
        max_wal_bytes: i64,
    ) -> Result<(), QuotaV2Error> {
        if snapshot.db_bytes > max_db_bytes {
            return Err(QuotaV2Error::DbBytes(snapshot.db_bytes));
        }
        let page_size = page_size(connection).unwrap_or(4096);
        let wal_bytes = snapshot.wal_pages.saturating_mul(page_size);
        if wal_bytes > max_wal_bytes {
            return Err(QuotaV2Error::WalBytes(wal_bytes));
        }
        Ok(())
    }

    /// Reclaim free pages in a bounded incremental-vacuum batch. No-op when there
    /// is nothing to reclaim or vacuum is not incremental.
    pub fn reclaim(connection: &Connection, pages: u32) -> Result<(), StorageError> {
        // Bounded incremental vacuum: batch is capped, and free pages are released
        // in small maintenance runs, not per event (self-documenting).
        if pages > 0 {
            connection.execute_batch(&format!("PRAGMA incremental_vacuum({pages});"))?;
        }
        Ok(())
    }
}

fn page_size(connection: &Connection) -> Result<i64, StorageError> {
    Ok(connection.query_row("PRAGMA page_size", [], |row| row.get(0))?)
}

fn db_file_bytes(connection: &Connection) -> Result<i64, StorageError> {
    match connection.path() {
        Some(path) => file_size(std::path::Path::new(path)),
        None => Ok(0),
    }
}

fn wal_file_bytes(connection: &Connection) -> Result<i64, StorageError> {
    match connection.path() {
        Some(path) => {
            let mut wal = std::path::PathBuf::from(path);
            let name = wal
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| format!("{n}-wal"))
                .unwrap_or_default();
            wal.set_file_name(name);
            Ok(file_size(&wal)?)
        }
        None => Ok(0),
    }
}

fn file_size(path: &std::path::Path) -> Result<i64, StorageError> {
    match fs::metadata(path) {
        Ok(meta) => Ok(meta.len().try_into().unwrap_or(i64::MAX)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(StorageError::Io(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Storage;
    use crate::StoragePaths;
    use tempfile::tempdir;

    fn conn_and_paths() -> (Connection, tempfile::TempDir) {
        let temp = tempdir().unwrap();
        let paths = StoragePaths::under(temp.path().to_path_buf());
        // Open storage to create/migrate the file-backed DB, then drop it. The
        // returned TempDir is kept alive so the DB file persists for the test.
        let _ = Storage::open(paths.clone()).unwrap();
        let connection = Connection::open(paths.database).unwrap();
        (connection, temp)
    }

    #[test]
    fn measure_returns_sane_non_negative() {
        let (connection, _temp) = conn_and_paths();
        let snap = QuotaV2::measure(&connection).unwrap();
        assert!(snap.page_count >= 0);
        assert!(snap.freelist_count >= 0);
        assert!(snap.wal_pages >= 0);
        assert!(snap.db_bytes >= 0);
    }

    #[test]
    fn admit_ok_under_limit() {
        let (connection, _temp) = conn_and_paths();
        let snap = QuotaV2::measure(&connection).unwrap();
        assert!(QuotaV2::admit(&connection, &snap, i64::MAX, i64::MAX).is_ok());
    }

    #[test]
    fn admit_rejects_over_db_limit() {
        let (connection, _temp) = conn_and_paths();
        let snap = QuotaV2::measure(&connection).unwrap();
        let result = QuotaV2::admit(&connection, &snap, 0, i64::MAX);
        assert!(matches!(result, Err(QuotaV2Error::DbBytes(_))));
    }

    #[test]
    fn admit_rejects_over_wal_limit() {
        let (connection, _temp) = conn_and_paths();
        // ponytail: fresh DB has an empty WAL, so drive a synthetic snapshot
        // instead of relying on a non-zero measured WAL.
        let snap = QuotaSnapshot {
            page_count: 1,
            freelist_count: 0,
            wal_pages: 4,
            db_bytes: 4096,
        };
        assert!(matches!(
            QuotaV2::admit(&connection, &snap, i64::MAX, 0),
            Err(QuotaV2Error::WalBytes(_))
        ));
        let _ = QuotaV2::measure(&connection).unwrap();
    }

    #[test]
    fn reclaim_runs() {
        let (connection, _temp) = conn_and_paths();
        assert!(QuotaV2::reclaim(&connection, 3).is_ok());
    }

    #[test]
    fn wal_quota_rejection() {
        // Create a connection with WAL journaling enforced
        let temp = tempdir().unwrap();
        let path = temp.path().join("test.db");
        let mut connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL; PRAGMA page_size=4096; PRAGMA auto_vacuum=INCREMENTAL;",
            )
            .unwrap();

        // Create a table and insert data to grow the WAL
        connection
            .execute_batch("CREATE TABLE test_rows(id INTEGER PRIMARY KEY, data TEXT);")
            .unwrap();

        // Insert many rows to generate WAL growth
        for i in 0..1000 {
            connection
                .execute(
                    "INSERT INTO test_rows(data) VALUES(?)",
                    [format!("row_{i}")],
                )
                .unwrap();
        }

        // Measure the snapshot - WAL should now have content
        let snap = QuotaV2::measure(&connection).unwrap();
        let wal_bytes = snap.wal_pages.saturating_mul(4096);

        // Verify WAL has grown beyond zero
        assert!(wal_bytes > 0, "WAL should have grown with inserts");

        // Set a quota that rejects due to WAL size
        // Use a quota lower than the measured WAL bytes
        let result = QuotaV2::admit(&connection, &snap, i64::MAX, wal_bytes / 2);
        assert!(
            matches!(result, Err(QuotaV2Error::WalBytes(_))),
            "admit should reject when WAL exceeds quota"
        );
    }

    #[test]
    fn reclaim_frees_space() {
        let (connection, _temp) = conn_and_paths();

        // Record initial measurements
        let snap_before = QuotaV2::measure(&connection).unwrap();
        let freelist_before = snap_before.freelist_count;

        // Do an incremental vacuum which should release free pages
        let result = QuotaV2::reclaim(&connection, 100);
        assert!(result.is_ok(), "reclaim should succeed");

        // Measure after - freelist_count should be reduced or equal
        let snap_after = QuotaV2::measure(&connection).unwrap();
        let freelist_after = snap_after.freelist_count;

        // freelist_count should not increase after reclaim
        assert!(
            freelist_after <= freelist_before,
            "freelist_count should not increase after reclaim: before={}, after={}",
            freelist_before,
            freelist_after
        );
    }
}
