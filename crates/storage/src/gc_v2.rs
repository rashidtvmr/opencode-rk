//! GC lifecycle owner for the format-2 workspace schema.
//!
//! Owns the `blobs` ready/deleting/unavailable lifecycle, orphan reachability,
//! and `event_outbox` prefix retention. Reachability is evaluated per-arm
//! against each root table (`message_parts`, `session_input_parts`,
//! `executions`, `tool_calls`, `context_epochs`, `compaction_checkpoints`,
//! `retained_payloads`) joined through `payloads`; the `payload_roots` view
//! is never used with `NOT IN` (NULL rows would void the predicate).
//!
//! The DDL triggers `blob_gc_claim` and `blob_no_resurrection` stay as
//! defense-in-depth: a claim that races a new payload reference fails closed
//! instead of collecting live content.
#![forbid(unsafe_code)]

use rusqlite::{params, Connection, TransactionBehavior};

use crate::StorageError;

const MAX_CLAIM_ROWS: usize = 500;

/// Per-arm unreachability predicate shared by claim and finish. Each arm
/// joins `payloads` to one root table; a blob is collectible only when no
/// arm finds a payload pointing at it. Correlates on `blobs.pk`.
const UNREFERENCED_ARMS: &str = "
  AND NOT EXISTS (
    SELECT 1 FROM payloads p
    JOIN message_parts mp ON mp.payload_pk = p.pk
    WHERE p.blob_pk = blobs.pk
  )
  AND NOT EXISTS (
    SELECT 1 FROM payloads p
    JOIN session_input_parts sp ON sp.payload_pk = p.pk
    WHERE p.blob_pk = blobs.pk
  )
  AND NOT EXISTS (
    SELECT 1 FROM payloads p
    JOIN executions e ON e.config_payload_pk = p.pk
    WHERE p.blob_pk = blobs.pk
  )
  AND NOT EXISTS (
    SELECT 1 FROM payloads p
    JOIN tool_calls t ON t.input_payload_pk = p.pk
      OR t.output_payload_pk = p.pk
      OR t.error_payload_pk = p.pk
    WHERE p.blob_pk = blobs.pk
  )
  AND NOT EXISTS (
    SELECT 1 FROM payloads p
    JOIN context_epochs ce ON ce.baseline_payload_pk = p.pk
      OR ce.snapshot_payload_pk = p.pk
    WHERE p.blob_pk = blobs.pk
  )
  AND NOT EXISTS (
    SELECT 1 FROM payloads p
    JOIN compaction_checkpoints cc ON cc.summary_payload_pk = p.pk
      OR cc.recent_payload_pk = p.pk
    WHERE p.blob_pk = blobs.pk
  )
  AND NOT EXISTS (
    SELECT 1 FROM payloads p
    JOIN retained_payloads rp ON rp.payload_pk = p.pk
    WHERE p.blob_pk = blobs.pk
  )
";

const CLAIM_HEAD: &str = "UPDATE blobs SET state = 1 WHERE pk IN ( \
     SELECT pk FROM blobs \
     WHERE state = 0 AND created_at_us <= ?1";
const CLAIM_TAIL: &str = " ORDER BY created_at_us ASC, pk ASC LIMIT ?2)";

/// Per-arm unreachability predicate for payloads. A payload is collectible
/// when it is not referenced by any root table. Mirrors UNREFERENCED_ARMS
/// but targets payloads directly (inline and blob-backed alike).
const UNREFERENCED_PAYLOAD_ARM: &str = "
  AND NOT EXISTS (
    SELECT 1 FROM message_parts mp WHERE mp.payload_pk = p.pk
  )
  AND NOT EXISTS (
    SELECT 1 FROM session_input_parts sp WHERE sp.payload_pk = p.pk
  )
  AND NOT EXISTS (
    SELECT 1 FROM executions e WHERE e.config_payload_pk = p.pk
  )
  AND NOT EXISTS (
    SELECT 1 FROM tool_calls t WHERE t.input_payload_pk = p.pk
      OR t.output_payload_pk = p.pk
      OR t.error_payload_pk = p.pk
  )
  AND NOT EXISTS (
    SELECT 1 FROM context_epochs ce WHERE ce.baseline_payload_pk = p.pk
      OR ce.snapshot_payload_pk = p.pk
  )
  AND NOT EXISTS (
    SELECT 1 FROM compaction_checkpoints cc WHERE cc.summary_payload_pk = p.pk
      OR cc.recent_payload_pk = p.pk
  )
  AND NOT EXISTS (
    SELECT 1 FROM retained_payloads rp WHERE rp.payload_pk = p.pk
  )
";

/// GC lifecycle: claim old unreferenced blobs as tombstones, finish their
/// deletion, prune the outbox prefix, and report retention pressure.
pub struct GcV2;

impl GcV2 {
    /// Tombstone (`state` 0 -> 1) at most `limit` ready blobs that are old
    /// enough and unreferenced. Limit clamped to 1..=500. Returns the number
    /// of blobs claimed. A claim that races a new reference fails closed
    /// via the `blob_gc_claim` trigger.
    pub fn claim_unreferenced_for_deletion(
        connection: &Connection,
        older_than_us: i64,
        limit: usize,
    ) -> Result<usize, StorageError> {
        let limit = limit.clamp(1, MAX_CLAIM_ROWS) as i64;
        let sql = format!("{CLAIM_HEAD}{UNREFERENCED_ARMS}{CLAIM_TAIL}");
        let changed = connection.execute(&sql, params![older_than_us, limit])?;
        Ok(changed as usize)
    }

    /// Delete a tombstoned blob after re-verifying it is still `state = 1`
    /// and still unreferenced. Any other state, a missing row, or a new
    /// reference surfaces as `changed == 0` and errors.
    pub fn finish_deletion(connection: &Connection, blob_pk: i64) -> Result<(), StorageError> {
        let sql = format!(
            "DELETE FROM blobs WHERE pk = ?1 AND state = 1 {UNREFERENCED_ARMS}"
        );
        let changed = connection.execute(&sql, params![blob_pk])?;
        if changed == 0 {
            return Err(invalid_input(
                "gc finish_deletion requires a tombstoned unreferenced blob",
            ));
        }
        Ok(())
    }

    /// Delete the outbox prefix (`seq < keep_from_seq`) and advance
    /// `workspace_state.event_floor_seq` to `keep_from_seq - 1` in the same
    /// IMMEDIATE transaction. `event_head_seq` is never touched. Returns the
    /// number of rows deleted. A floor past head fails closed on the DDL
    /// CHECK and rolls the whole prefix delete back.
    pub fn prune_outbox_prefix(
        connection: &mut Connection,
        keep_from_seq: i64,
    ) -> Result<u64, StorageError> {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let deleted = transaction.execute(
            "DELETE FROM event_outbox WHERE seq < ?1",
            params![keep_from_seq],
        )?;
        if deleted > 0 {
            let floor = keep_from_seq.saturating_sub(1);
            let changed = transaction.execute(
                "UPDATE workspace_state
                 SET event_floor_seq = MAX(event_floor_seq, ?1)
                 WHERE id = 1",
                params![floor],
            )?;
            if changed != 1 {
                return Err(invalid_input("gc prune requires workspace_state id=1"));
            }
        }
        transaction.commit()?;
        Ok(deleted as u64)
    }

    /// Collect unreferenced `payloads` rows (inline or blob-backed) older than
    /// the cutoff via per-arm NOT EXISTS (same 7+ roots used by claim_unreferenced_for_deletion
    /// and retention sweep), then DELETE them bounded LIMIT clamp(1,500).
    /// Returns count. MUST NOT delete referenced payloads.
    pub fn claim_orphan_payloads(
        connection: &Connection,
        older_than_us: i64,
        limit: usize,
    ) -> Result<usize, StorageError> {
        let limit = limit.clamp(1, MAX_CLAIM_ROWS) as i64;
        let sql = format!(
            "DELETE FROM payloads WHERE pk IN ( \
             SELECT p.pk FROM payloads p \
             WHERE p.created_at_us <= ?1 \
             {UNREFERENCED_PAYLOAD_ARM} \
             ORDER BY p.created_at_us ASC, p.pk ASC LIMIT ?2)"
        );
        let deleted = connection.execute(&sql, params![older_than_us, limit])?;
        Ok(deleted as usize)
    }

    /// `(ready, deleting/tombstone, unavailable)` blob counts by state:
    /// `state = 0`, `state = 1`, and anything else.
    pub fn retention_counts(connection: &Connection) -> Result<(i64, i64, i64), StorageError> {
        let counts = connection.query_row(
            "SELECT COALESCE(SUM(state = 0), 0),
                    COALESCE(SUM(state = 1), 0),
                    COALESCE(SUM(state NOT IN (0, 1)), 0)
             FROM blobs",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        Ok(counts)
    }
}

fn invalid_input(message: &str) -> StorageError {
    StorageError::Sqlite(rusqlite::Error::InvalidParameterName(message.into()))
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

    fn insert_blob(connection: &Connection, state: i64, created_us: i64) -> i64 {
        connection
            .execute(
                "INSERT INTO blobs (hash, state, codec, raw_bytes, stored_bytes, created_at_us)
                 VALUES (randomblob(32), ?1, 0, 9000, 9052, ?2)",
                params![state, created_us],
            )
            .unwrap();
        connection.last_insert_rowid()
    }

    fn state_of(connection: &Connection, pk: i64) -> Option<i64> {
        connection
            .query_row("SELECT state FROM blobs WHERE pk = ?1", [pk], |row| {
                row.get(0)
            })
            .ok()
    }

    fn blob_payload(connection: &Connection, blob_pk: i64) -> i64 {
        connection
            .query_row(
                "INSERT INTO payloads (blob_pk, raw_bytes, created_at_us)
                 VALUES (?1, 9000, 0) RETURNING pk",
                params![blob_pk],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn reference_via_message(connection: &Connection, blob_pk: i64) {
        connection
            .execute(
                "INSERT INTO sessions (id, title, created_at_us, updated_at_us)
                 VALUES (randomblob(16), 't', 0, 1)",
                [],
            )
            .unwrap();
        let session_pk = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO messages (id, session_pk, seq, role, status, created_at_us, completed_at_us)
                 VALUES (randomblob(16), ?1, 1, 1, 1, 0, 0)",
                params![session_pk],
            )
            .unwrap();
        let message_pk = connection.last_insert_rowid();
        let payload_pk = blob_payload(connection, blob_pk);
        connection
            .execute(
                "INSERT INTO message_parts (message_pk, ordinal, kind, payload_pk)
                 VALUES (?1, 0, 0, ?2)",
                params![message_pk, payload_pk],
            )
            .unwrap();
    }

    fn reference_via_retained(connection: &Connection, blob_pk: i64) {
        let payload_pk = blob_payload(connection, blob_pk);
        connection
            .execute(
                "INSERT INTO retained_payloads (owner_id, purpose, payload_pk, created_at_us)
                 VALUES (randomblob(16), 2, ?1, 0)",
                params![payload_pk],
            )
            .unwrap();
    }

    fn insert_outbox_seq(connection: &Connection, seq: i64) {
        connection
            .execute(
                "INSERT INTO event_outbox (seq, session_id, kind, payload_json, created_at_us)
                 VALUES (?1, NULL, 'k', '{}', 0)",
                params![seq],
            )
            .unwrap();
    }

    fn floor_and_head(connection: &Connection) -> (i64, i64) {
        connection
            .query_row(
                "SELECT event_floor_seq, event_head_seq FROM workspace_state WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap()
    }

    #[test]
    fn claim_takes_old_unreferenced_blob() {
        let (_dir, conn) = workspace();
        let pk = insert_blob(&conn, 0, 10);
        assert_eq!(
            GcV2::claim_unreferenced_for_deletion(&conn, 10, 500).unwrap(),
            1
        );
        assert_eq!(state_of(&conn, pk), Some(1));
    }

    #[test]
    fn claim_skips_referenced_blob() {
        let (_dir, conn) = workspace();
        let via_message = insert_blob(&conn, 0, 10);
        reference_via_message(&conn, via_message);
        let via_retained = insert_blob(&conn, 0, 10);
        reference_via_retained(&conn, via_retained);
        assert_eq!(
            GcV2::claim_unreferenced_for_deletion(&conn, i64::MAX, 500).unwrap(),
            0
        );
        assert_eq!(state_of(&conn, via_message), Some(0));
        assert_eq!(state_of(&conn, via_retained), Some(0));
    }

    #[test]
    fn claim_respects_age_and_limit() {
        let (_dir, conn) = workspace();
        let young = insert_blob(&conn, 0, 2000);
        let old_a = insert_blob(&conn, 0, 10);
        let old_b = insert_blob(&conn, 0, 20);
        let old_c = insert_blob(&conn, 0, 30);
        assert_eq!(
            GcV2::claim_unreferenced_for_deletion(&conn, 1000, 500).unwrap(),
            3
        );
        assert_eq!(state_of(&conn, young), Some(0));
        assert_eq!(state_of(&conn, old_a), Some(1));
        assert_eq!(state_of(&conn, old_b), Some(1));
        assert_eq!(state_of(&conn, old_c), Some(1));

        let extra_a = insert_blob(&conn, 0, 5);
        let extra_b = insert_blob(&conn, 0, 6);
        let extra_c = insert_blob(&conn, 0, 7);
        assert_eq!(
            GcV2::claim_unreferenced_for_deletion(&conn, i64::MAX, 2).unwrap(),
            2
        );
        // Limit 0 clamps to 1: exactly one of the three is claimed here.
        assert_eq!(
            GcV2::claim_unreferenced_for_deletion(&conn, i64::MAX, 0).unwrap(),
            1
        );
        let claimed = [extra_a, extra_b, extra_c]
            .iter()
            .filter(|pk| state_of(&conn, **pk) == Some(1))
            .count();
        assert_eq!(claimed, 3);
    }

    #[test]
    fn finish_deletes_only_tombstoned_unreferenced() {
        let (_dir, conn) = workspace();
        assert!(GcV2::finish_deletion(&conn, 999_999).is_err());
        let ready = insert_blob(&conn, 0, 10);
        assert!(GcV2::finish_deletion(&conn, ready).is_err());
        let unavailable = insert_blob(&conn, 2, 10);
        assert!(GcV2::finish_deletion(&conn, unavailable).is_err());

        let referenced = insert_blob(&conn, 0, 10);
        reference_via_message(&conn, referenced);
        assert!(GcV2::finish_deletion(&conn, referenced).is_err());
        assert_eq!(state_of(&conn, referenced), Some(0));

        let tombstone = insert_blob(&conn, 0, 10);
        assert_eq!(
            GcV2::claim_unreferenced_for_deletion(&conn, i64::MAX, 500).unwrap(),
            2
        );
        assert_eq!(state_of(&conn, tombstone), Some(1));
        GcV2::finish_deletion(&conn, tombstone).unwrap();
        assert_eq!(state_of(&conn, tombstone), None);
        assert!(GcV2::finish_deletion(&conn, tombstone).is_err());
    }

    #[test]
    fn prune_advances_floor_not_head() {
        let (_dir, mut conn) = workspace();
        conn.execute("UPDATE workspace_state SET event_head_seq = 3 WHERE id = 1", [])
            .unwrap();
        insert_outbox_seq(&conn, 1);
        insert_outbox_seq(&conn, 2);
        insert_outbox_seq(&conn, 3);
        assert_eq!(GcV2::prune_outbox_prefix(&mut conn, 1).unwrap(), 0);
        assert_eq!(floor_and_head(&conn), (0, 3));
        assert_eq!(GcV2::prune_outbox_prefix(&mut conn, 3).unwrap(), 2);
        assert_eq!(floor_and_head(&conn), (2, 3));
        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM event_outbox", [], |row| row.get(0))
            .unwrap();
        assert_eq!(remaining, 1);
        assert_eq!(GcV2::prune_outbox_prefix(&mut conn, 4).unwrap(), 1);
        assert_eq!(floor_and_head(&conn), (3, 3));
    }

    #[test]
    fn retention_counts_correct() {
        let (_dir, conn) = workspace();
        assert_eq!(GcV2::retention_counts(&conn).unwrap(), (0, 0, 0));
        insert_blob(&conn, 0, 1);
        insert_blob(&conn, 0, 2);
        insert_blob(&conn, 1, 3);
        insert_blob(&conn, 2, 4);
        assert_eq!(GcV2::retention_counts(&conn).unwrap(), (2, 1, 1));
    }

    fn insert_inline_payload(connection: &Connection, created_at_us: i64) -> i64 {
        connection
            .query_row(
                "INSERT INTO payloads (inline_data, raw_bytes, created_at_us)
                 VALUES (randomblob(8192), 8192, ?1) RETURNING pk",
                params![created_at_us],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn insert_session_message_parts(connection: &Connection) -> (i64, i64, i64) {
        connection
            .execute(
                "INSERT INTO sessions (id, title, created_at_us, updated_at_us)
                 VALUES (randomblob(16), 't', 0, 1)",
                [],
            )
            .unwrap();
        let session_pk = connection.last_insert_rowid();

        connection
            .execute(
                "INSERT INTO messages (id, session_pk, seq, role, status, created_at_us, completed_at_us)
                 VALUES (randomblob(16), ?1, 1, 1, 1, 0, 0)",
                params![session_pk],
            )
            .unwrap();
        let message_pk = connection.last_insert_rowid();

        let payload_pk = connection
            .query_row(
                "INSERT INTO payloads (inline_data, raw_bytes, created_at_us)
                 VALUES (randomblob(8192), 8192, 0) RETURNING pk",
                [],
                |row| row.get(0),
            )
            .unwrap();

        connection
            .execute(
                "INSERT INTO message_parts (message_pk, ordinal, kind, payload_pk)
                 VALUES (?1, 0, 0, ?2)",
                params![message_pk, payload_pk],
            )
            .unwrap();

        (session_pk, message_pk, payload_pk)
    }

    fn reference_payload_via_message(connection: &Connection, payload_pk: i64) {
        connection
            .execute(
                "INSERT INTO sessions (id, title, created_at_us, updated_at_us)
                 VALUES (randomblob(16), 't', 0, 1)",
                [],
            )
            .unwrap();
        let session_pk = connection.last_insert_rowid();

        connection
            .execute(
                "INSERT INTO messages (id, session_pk, seq, role, status, created_at_us, completed_at_us)
                 VALUES (randomblob(16), ?1, 1, 1, 1, 0, 0)",
                params![session_pk],
            )
            .unwrap();
        let message_pk = connection.last_insert_rowid();

        connection
            .execute(
                "INSERT INTO message_parts (message_pk, ordinal, kind, payload_pk)
                 VALUES (?1, 0, 0, ?2)",
                params![message_pk, payload_pk],
            )
            .unwrap();
    }

    #[test]
    fn claim_orphan_payloads_deletes_old_orphan_keeps_referenced() {
        let (_dir, conn) = workspace();

        // Insert old orphan inline payload (unreferenced, created_at_us = 1)
        let orphan_pk = insert_inline_payload(&conn, 1);

        // Insert referenced inline payload (created_at_us = 2, but referenced via message)
        let referenced_payload_pk = insert_inline_payload(&conn, 2);
        reference_payload_via_message(&conn, referenced_payload_pk);

        let cutoff = 10;
        let deleted = GcV2::claim_orphan_payloads(&conn, cutoff, 500).unwrap();
        assert_eq!(deleted, 1);

        // Verify orphan payload is gone
        let orphan_exists: i64 = conn
            .query_row("SELECT COUNT(*) FROM payloads WHERE pk = ?1", [orphan_pk], |row| row.get(0))
            .unwrap();
        assert_eq!(orphan_exists, 0);

        // Verify referenced payload remains
        let referenced_exists: i64 = conn
            .query_row("SELECT COUNT(*) FROM payloads WHERE pk = ?1", [referenced_payload_pk], |row| row.get(0))
            .unwrap();
        assert_eq!(referenced_exists, 1);
    }

    #[test]
    fn claim_is_fail_closed_when_referenced() {
        let (_dir, conn) = workspace();

        // Insert a blob with state=0 (ready), per DDL: hash 32 bytes, codec 0, raw_bytes >= 0, stored_bytes >= 52
        let blob_pk = insert_blob(&conn, 0, 1);

        // Claim it to tombstone (state 0 -> 1) - should succeed since blob is unreferenced
        let claimed = GcV2::claim_unreferenced_for_deletion(&conn, 1, 500).unwrap();
        assert_eq!(claimed, 1);
        assert_eq!(state_of(&conn, blob_pk), Some(1));

        // Attempt to create a payload referencing the tombstoned blob.
        // The payload_ready trigger checks `state = 0`, so it must abort.
        // A tombstoned blob cannot be resurrected via a new payload reference.
        let result = conn.execute(
            "INSERT INTO payloads (blob_pk, raw_bytes, created_at_us)
             VALUES (?1, 9000, 0)",
            params![blob_pk],
        );
        assert!(result.is_err(), "tombstoned blob must not accept new payload");

        // Attempt to finish deletion of the tombstoned blob (no payload exists).
        // This should succeed since the blob is now unreferenced.
        assert!(GcV2::finish_deletion(&conn, blob_pk).is_ok());
        assert_eq!(state_of(&conn, blob_pk), None);

        // Attempt a double-finish - should fail (blob gone).
        assert!(GcV2::finish_deletion(&conn, blob_pk).is_err());
    }
}
