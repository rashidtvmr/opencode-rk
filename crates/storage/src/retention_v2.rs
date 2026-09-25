//! Bounded retention sweeps for the format-2 workspace schema.
//!
//! Closes unbounded growth of expired operation receipts, resolved terminal
//! approvals (with their resources), and orphan inline payloads that no root
//! references. Every sweep is bounded by a clamped LIMIT so a single call can
//! never delete more than 500 rows. Payload reachability is evaluated per-arm
//! against each root table joined through `payloads`; a payload referenced by
//! any root is never deleted.
#![forbid(unsafe_code)]

use rusqlite::{params, Connection};

use crate::StorageError;

const MAX_SWEEP_ROWS: usize = 500;

/// Per-arm reachability predicate used by the orphan inline payload sweep.
/// A payload is collectible only when no arm finds a root referencing it.
/// Correlates on `payloads.pk`. `NOT EXISTS` is used (never `NOT IN`) so a
/// NULL foreign key cannot void the predicate.
const REFERENCED_ARMS: &str = "
  AND NOT EXISTS (
    SELECT 1 FROM message_parts mp WHERE mp.payload_pk = payloads.pk
  )
  AND NOT EXISTS (
    SELECT 1 FROM session_input_parts sp WHERE sp.payload_pk = payloads.pk
  )
  AND NOT EXISTS (
    SELECT 1 FROM executions e WHERE e.config_payload_pk = payloads.pk
  )
  AND NOT EXISTS (
    SELECT 1 FROM tool_calls t
    WHERE t.input_payload_pk = payloads.pk
       OR t.output_payload_pk = payloads.pk
       OR t.error_payload_pk = payloads.pk
  )
  AND NOT EXISTS (
    SELECT 1 FROM context_epochs ce
    WHERE ce.baseline_payload_pk = payloads.pk
       OR ce.snapshot_payload_pk = payloads.pk
  )
  AND NOT EXISTS (
    SELECT 1 FROM compaction_checkpoints cc
    WHERE cc.summary_payload_pk = payloads.pk
       OR cc.recent_payload_pk = payloads.pk
  )
  AND NOT EXISTS (
    SELECT 1 FROM retained_payloads rp WHERE rp.payload_pk = payloads.pk
  )
";

const ORPHAN_PAYLOAD_MATCH: &str = "
  WHERE inline_data IS NOT NULL
    AND raw_bytes >= 0
    AND created_at_us <= ?1
";

/// Bounded retention sweeps for receipts, approvals, and orphan inline
/// payloads, plus a backlog report of all three.
pub struct RetentionV2;

impl RetentionV2 {
    /// Delete at most `limit` (clamped 1..=500) operation receipts whose
    /// `retry_until_us` has passed. Returns the number deleted.
    pub fn sweep_expired_receipts(
        connection: &Connection,
        now_us: i64,
        limit: usize,
    ) -> Result<usize, StorageError> {
        let limit = limit.clamp(1, MAX_SWEEP_ROWS) as i64;
        let changed = connection.execute(
            "DELETE FROM operation_receipts
             WHERE operation_id IN (
                 SELECT operation_id FROM operation_receipts
                 WHERE retry_until_us <= ?1
                 ORDER BY retry_until_us ASC, operation_id ASC
                 LIMIT ?2
             )",
            params![now_us, limit],
        )?;
        Ok(changed as usize)
    }

    /// Delete at most `limit` (clamped 1..=500) approvals in a terminal state
    /// resolved at or before `older_than_us`. `approval_resources` rows cascade
    /// via the ON DELETE CASCADE foreign key. Returns the number deleted.
    pub fn sweep_resolved_approvals(
        connection: &Connection,
        older_than_us: i64,
        limit: usize,
    ) -> Result<usize, StorageError> {
        let limit = limit.clamp(1, MAX_SWEEP_ROWS) as i64;
        let changed = connection.execute(
            "DELETE FROM approvals
             WHERE pk IN (
                 SELECT pk FROM approvals
                  WHERE state IN (1, 2, 3, 4)
                   AND resolved_at_us IS NOT NULL
                   AND resolved_at_us <= ?1
                 ORDER BY resolved_at_us ASC, pk ASC
                 LIMIT ?2
             )",
            params![older_than_us, limit],
        )?;
        Ok(changed as usize)
    }

    /// Delete at most `limit` (clamped 1..=500) inline payloads that are old
    /// and unreferenced by every root. A payload referenced by *any* root is
    /// never deleted. Returns the number deleted.
    pub fn sweep_orphan_inline_payloads(
        connection: &Connection,
        older_than_us: i64,
        limit: usize,
    ) -> Result<usize, StorageError> {
        let limit = limit.clamp(1, MAX_SWEEP_ROWS) as i64;
        let sql = format!(
            "DELETE FROM payloads
             WHERE pk IN (
                 SELECT payloads.pk FROM payloads {ORPHAN_PAYLOAD_MATCH}{REFERENCED_ARMS}
                 ORDER BY created_at_us ASC, pk ASC
                 LIMIT ?2
             )"
        );
        let changed = connection.execute(&sql, params![older_than_us, limit])?;
        Ok(changed as usize)
    }

    /// `(expired_receipts, resolved_approvals, orphan_inline_payloads)` counts
    /// for the three sweeps, used to decide whether and how much to sweep.
    pub fn retention_backlog(
        connection: &Connection,
        now_us: i64,
    ) -> Result<(i64, i64, i64), StorageError> {
        let expired_receipts: i64 = connection.query_row(
            "SELECT COUNT(*) FROM operation_receipts WHERE retry_until_us <= ?1",
            params![now_us],
            |row| row.get(0),
        )?;
        let resolved_approvals: i64 = connection.query_row(
            "SELECT COUNT(*) FROM approvals
             WHERE state IN (1, 2, 3, 4)
               AND resolved_at_us IS NOT NULL
               AND resolved_at_us <= ?1",
            params![now_us],
            |row| row.get(0),
        )?;
        let orphan_payloads: i64 = {
            let sql =
                format!("SELECT COUNT(*) FROM payloads {ORPHAN_PAYLOAD_MATCH}{REFERENCED_ARMS}");
            connection.query_row(&sql, params![now_us], |row| row.get(0))?
        };
        Ok((expired_receipts, resolved_approvals, orphan_payloads))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema_v2::SchemaV2;
    use tempfile::TempDir;

    fn workspace() -> (TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = SchemaV2::initialize_workspace(
            &dir.path().join("workspace.db"),
            [1_u8; 16],
            [2_u8; 16],
            0,
        )
        .unwrap();
        (dir, conn)
    }

    /// Insert a receipt with the given retry_until_us.
    fn insert_receipt(conn: &Connection, retry_until_us: i64) {
        conn.execute(
            "INSERT INTO operation_receipts (operation_id, request_hash, kind, result_json, created_at_us, retry_until_us)
             VALUES (randomblob(16), randomblob(32), 'k', '{}', 0, ?1)",
            params![retry_until_us],
        )
        .unwrap();
    }

    fn receipt_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM operation_receipts", [], |r| r.get(0))
            .unwrap()
    }

    /// Insert a session and an approval in the given state/resolved_at_us.
    fn insert_approval(conn: &Connection, state: i64, resolved_at_us: Option<i64>) -> i64 {
        conn.execute(
            "INSERT INTO sessions (id, title, created_at_us, updated_at_us)
             VALUES (randomblob(16), 't', 0, 1)",
            [],
        )
        .unwrap();
        let session_pk = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO approvals
             (id, session_pk, intent_hash, policy_generation, action, state,
              mandatory_human, created_at_us, expires_at_us, resolved_at_us)
             VALUES (randomblob(16), ?1, randomblob(32), 0, 'act', ?2, 0, 5, 100, ?3)",
            params![session_pk, state, resolved_at_us],
        )
        .unwrap();
        let approval_pk = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO approval_resources (approval_pk, ordinal, resource)
             VALUES (?1, 0, 'file:///tmp/x')",
            params![approval_pk],
        )
        .unwrap();
        approval_pk
    }

    fn approval_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM approvals", [], |r| r.get(0))
            .unwrap()
    }

    fn resource_count_for(conn: &Connection, approval_pk: i64) -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM approval_resources WHERE approval_pk = ?1",
            params![approval_pk],
            |r| r.get(0),
        )
        .unwrap()
    }

    /// Insert an inline payload at the given created_at_us.
    fn insert_inline_payload(conn: &Connection, created_at_us: i64) -> i64 {
        conn.execute(
            "INSERT INTO payloads (inline_data, raw_bytes, created_at_us)
             VALUES (x'deadbeef', 4, ?1)",
            params![created_at_us],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn payload_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM payloads", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn sweep_expired_receipts_deletes_expired_keeps_fresh() {
        let (_dir, conn) = workspace();
        insert_receipt(&conn, 10); // expired
        insert_receipt(&conn, 20); // expired
        insert_receipt(&conn, 1000); // fresh
        assert_eq!(
            RetentionV2::sweep_expired_receipts(&conn, 50, 500).unwrap(),
            2
        );
        assert_eq!(receipt_count(&conn), 1);
    }

    #[test]
    fn sweep_expired_receipts_respects_limit_clamp() {
        let (_dir, conn) = workspace();
        for i in 1..=3 {
            insert_receipt(&conn, i);
        }
        // Limit 0 clamps to 1: exactly one deleted.
        assert_eq!(
            RetentionV2::sweep_expired_receipts(&conn, 99, 0).unwrap(),
            1
        );
        assert_eq!(receipt_count(&conn), 2);
        assert_eq!(
            RetentionV2::sweep_expired_receipts(&conn, 99, 1).unwrap(),
            1
        );
        assert_eq!(
            RetentionV2::sweep_expired_receipts(&conn, 99, 500).unwrap(),
            1
        );
        assert_eq!(receipt_count(&conn), 0);
    }

    #[test]
    fn sweep_resolved_approvals_deletes_terminal_keeps_pending_and_cascades() {
        let (_dir, conn) = workspace();
        let allowed = insert_approval(&conn, 1, Some(5)); // state 1 resolved, swept
        insert_approval(&conn, 4, None); // consumed but unresolved, kept
        let pending = insert_approval(&conn, 0, Some(5)); // pending state 0, kept
        assert_eq!(approval_count(&conn), 3);
        assert_eq!(
            RetentionV2::sweep_resolved_approvals(&conn, 50, 500).unwrap(),
            1
        );
        assert_eq!(approval_count(&conn), 2);
        // cascade removed the swept approval's resources
        assert_eq!(resource_count_for(&conn, allowed), 0);
        assert_eq!(resource_count_for(&conn, pending), 1);
    }

    #[test]
    fn sweep_orphan_inline_payloads_deletes_old_keeps_new() {
        let (_dir, conn) = workspace();
        insert_inline_payload(&conn, 10); // old, orphan
        insert_inline_payload(&conn, 20); // old, orphan
        insert_inline_payload(&conn, 1000); // fresh, kept regardless
        assert_eq!(
            RetentionV2::sweep_orphan_inline_payloads(&conn, 50, 500).unwrap(),
            2
        );
        assert_eq!(payload_count(&conn), 1);
    }

    #[test]
    fn sweep_orphan_keeps_referenced_payload() {
        let (_dir, conn) = workspace();
        // Referenced payloads use a blob-backed backing? No: inline is fine to reference.
        let orphan = insert_inline_payload(&conn, 1);
        let referenced = insert_inline_payload(&conn, 2);
        conn.execute(
            "INSERT INTO sessions (id, title, created_at_us, updated_at_us)
             VALUES (randomblob(16), 't', 0, 1)",
            [],
        )
        .unwrap();
        let session_pk = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO messages (id, session_pk, seq, role, status, created_at_us, completed_at_us)
             VALUES (randomblob(16), ?1, 1, 1, 1, 0, 0)",
            params![session_pk],
        )
        .unwrap();
        let message_pk = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO message_parts (message_pk, ordinal, kind, payload_pk)
             VALUES (?1, 0, 0, ?2)",
            params![message_pk, referenced],
        )
        .unwrap();
        assert_eq!(
            RetentionV2::sweep_orphan_inline_payloads(&conn, 999, 500).unwrap(),
            1
        );
        assert_eq!(payload_count(&conn), 1);
        let remaining: i64 = conn
            .query_row(
                "SELECT pk FROM payloads WHERE pk = ?1",
                params![referenced],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(remaining, referenced);
        // orphan row is gone
        assert!(conn
            .query_row(
                "SELECT 1 FROM payloads WHERE pk = ?1",
                params![orphan],
                |r| r.get::<_, i64>(0),
            )
            .is_err());
    }

    #[test]
    fn backlog_counts_correct() {
        let (_dir, conn) = workspace();
        insert_receipt(&conn, 10); // expired
        insert_receipt(&conn, 1000); // live
        insert_approval(&conn, 1, Some(5)); // resolved
        insert_approval(&conn, 0, None); // pending
        insert_inline_payload(&conn, 1); // orphan
        assert_eq!(
            RetentionV2::retention_backlog(&conn, 50).unwrap(),
            (1, 1, 1)
        );
    }
}
