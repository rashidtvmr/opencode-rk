//! Dual rollout record: append-only JSONL truth plus sqlite queryable snapshot.
//!
//! JSONL is the authoritative log. The sqlite snapshot is a queryable mirror.
//! `append` writes JSONL first, then sqlite. `replay_from_jsonl` recovers truth
//! from the JSONL file even if sqlite is corrupted. `verify_consistency` checks
//! JSONL line count matches sqlite row count.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

/// A single rollout event record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RolloutRecord {
    pub id: String,
    pub timestamp_us: i64,
    pub event_type: String,
    pub payload_json: String,
}

/// Recorder owning both the JSONL truth log and the sqlite snapshot.
pub struct RolloutRecorder {
    writer: BufWriter<std::fs::File>,
    jsonl_path: std::path::PathBuf,
    conn: Connection,
}

impl RolloutRecorder {
    /// Create the `_rollout_events` table and open the JSONL file for append.
    pub fn new<P: AsRef<Path>>(
        jsonl_path: P,
        conn: Connection,
    ) -> Result<Self, crate::StorageError> {
        let path = jsonl_path.as_ref();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS _rollout_events (
                id TEXT PRIMARY KEY,
                timestamp_us INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL
            );",
        )?;
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let writer = BufWriter::new(file);
        let jsonl_path = path.to_path_buf();
        Ok(Self {
            writer,
            jsonl_path,
            conn,
        })
    }

    /// Write record to JSONL first (truth), then INSERT into sqlite snapshot.
    pub fn append(&mut self, record: &RolloutRecord) -> Result<(), crate::StorageError> {
        let line = serde_json::to_string(record)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        writeln!(self.writer, "{}", line)?;
        self.writer.flush()?;
        self.conn.execute(
            "INSERT INTO _rollout_events (id, timestamp_us, event_type, payload_json)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                record.id,
                record.timestamp_us,
                record.event_type,
                record.payload_json
            ],
        )?;
        Ok(())
    }

    /// Query records by event_type from the sqlite snapshot, ordered by timestamp.
    pub fn query_by_type(
        &self,
        event_type: &str,
        limit: usize,
    ) -> Result<Vec<RolloutRecord>, crate::StorageError> {
        let limit = limit.min(10_000) as i64;
        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp_us, event_type, payload_json
             FROM _rollout_events
             WHERE event_type = ?1
             ORDER BY timestamp_us ASC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![event_type, limit], |row| {
            Ok(RolloutRecord {
                id: row.get(0)?,
                timestamp_us: row.get(1)?,
                event_type: row.get(2)?,
                payload_json: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(crate::StorageError::from)
    }

    /// Replay all records from a JSONL file (the truth source).
    pub fn replay_from_jsonl<P: AsRef<Path>>(
        path: P,
    ) -> Result<Vec<RolloutRecord>, std::io::Error> {
        let file = std::fs::File::open(path)?;
        let reader = BufReader::new(file);
        let mut records = Vec::new();
        for line in reader.lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let record: RolloutRecord = serde_json::from_str(line)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            records.push(record);
        }
        Ok(records)
    }

    /// Verify JSONL line count matches sqlite row count.
    pub fn verify_consistency(&self) -> Result<bool, crate::StorageError> {
        let jsonl_count = count_jsonl_lines(&self.jsonl_path)?;
        let sqlite_count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM _rollout_events", [], |row| row.get(0))?;
        Ok(jsonl_count as i64 == sqlite_count)
    }

    /// Flush and close the writer (idempotent-ish; writer dropped on drop).
    pub fn flush(&mut self) -> Result<(), crate::StorageError> {
        self.writer.flush()?;
        Ok(())
    }

    /// Number of rows in the sqlite snapshot.
    pub fn sqlite_row_count(&self) -> Result<i64, crate::StorageError> {
        self.conn
            .query_row("SELECT COUNT(*) FROM _rollout_events", [], |row| row.get(0))
            .map_err(crate::StorageError::from)
    }
}

fn count_jsonl_lines(path: &Path) -> Result<usize, std::io::Error> {
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut count = 0;
    for line in reader.lines() {
        let line = line?;
        if !line.trim().is_empty() {
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn record(seq: u8, et: &str) -> RolloutRecord {
        RolloutRecord {
            id: format!("id-{seq}"),
            timestamp_us: (seq as i64) * 1_000,
            event_type: et.to_owned(),
            payload_json: format!(r#"{{"seq":{seq}}}"#),
        }
    }

    fn new_recorder(dir: &std::path::Path) -> RolloutRecorder {
        let jsonl = dir.join("rollout.jsonl");
        let conn = Connection::open_in_memory().unwrap();
        RolloutRecorder::new(&jsonl, conn).unwrap()
    }

    #[test]
    fn append_and_query() {
        let dir = tempdir().unwrap();
        let mut r = new_recorder(dir.path());
        r.append(&record(1, "deploy")).unwrap();
        r.append(&record(2, "deploy")).unwrap();
        r.append(&record(3, "rollback")).unwrap();
        let deploys = r.query_by_type("deploy", 10).unwrap();
        assert_eq!(deploys.len(), 2);
        assert_eq!(deploys[0].id, "id-1");
        assert_eq!(deploys[1].id, "id-2");
        let rollbacks = r.query_by_type("rollback", 10).unwrap();
        assert_eq!(rollbacks.len(), 1);
        assert_eq!(rollbacks[0].id, "id-3");
    }

    #[test]
    fn jsonl_is_truth() {
        let dir = tempdir().unwrap();
        let jsonl = dir.path().join("rollout.jsonl");
        let conn = Connection::open_in_memory().unwrap();
        {
            let mut r = RolloutRecorder::new(&jsonl, conn).unwrap();
            r.append(&record(1, "deploy")).unwrap();
            r.append(&record(2, "deploy")).unwrap();
        }
        // Drop the real snapshot table and recreate empty - simulates corruption
        let conn2 = Connection::open_in_memory().unwrap();
        conn2
            .execute(
                "CREATE TABLE _rollout_events (
                    id TEXT PRIMARY KEY,
                    timestamp_us INTEGER NOT NULL,
                    event_type TEXT NOT NULL,
                    payload_json TEXT NOT NULL
                )",
                [],
            )
            .unwrap();

        // Replay from JSONL should still recover both records
        let replayed = RolloutRecorder::replay_from_jsonl(&jsonl).unwrap();
        assert_eq!(replayed.len(), 2);
        assert_eq!(replayed[0].id, "id-1");
        assert_eq!(replayed[1].id, "id-2");
        // conn2 is empty - truth came from JSONL
        let empty_count: i64 = conn2
            .query_row("SELECT COUNT(*) FROM _rollout_events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(empty_count, 0);
    }

    #[test]
    fn replay_matches() {
        let dir = tempdir().unwrap();
        let jsonl = dir.path().join("rollout.jsonl");
        let conn = Connection::open_in_memory().unwrap();
        let mut r = RolloutRecorder::new(&jsonl, conn).unwrap();
        let r1 = record(1, "deploy");
        let r2 = record(2, "rollback");
        let r3 = record(3, "deploy");
        r.append(&r1).unwrap();
        r.append(&r2).unwrap();
        r.append(&r3).unwrap();
        let replayed = RolloutRecorder::replay_from_jsonl(&jsonl).unwrap();
        assert_eq!(replayed.len(), 3);
        assert_eq!(replayed[0], r1);
        assert_eq!(replayed[1], r2);
        assert_eq!(replayed[2], r3);
    }

    #[test]
    fn consistency_check() {
        let dir = tempdir().unwrap();
        let mut r = new_recorder(dir.path());
        assert!(r.verify_consistency().unwrap()); // both empty
        r.append(&record(1, "deploy")).unwrap();
        r.append(&record(2, "deploy")).unwrap();
        assert!(r.verify_consistency().unwrap());
    }

    #[test]
    fn query_by_type_filters() {
        let dir = tempdir().unwrap();
        let mut r = new_recorder(dir.path());
        r.append(&record(1, "deploy")).unwrap();
        r.append(&record(2, "rollback")).unwrap();
        r.append(&record(3, "deploy")).unwrap();
        r.append(&record(4, "deploy")).unwrap();
        r.append(&record(5, "rollback")).unwrap();
        let deploys = r.query_by_type("deploy", 10).unwrap();
        assert_eq!(deploys.len(), 3);
        assert_eq!(
            deploys.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec!["id-1", "id-3", "id-4"]
        );
        let rollbacks = r.query_by_type("rollback", 10).unwrap();
        assert_eq!(rollbacks.len(), 2);
        let none = r.query_by_type("other", 10).unwrap();
        assert_eq!(none.len(), 0);
        // limit test
        let limited = r.query_by_type("deploy", 2).unwrap();
        assert_eq!(limited.len(), 2);
    }
}
