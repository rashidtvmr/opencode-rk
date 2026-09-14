//! Bounded snapshot/export owner for the format-2 workspace schema.
//!
//! Owns `context_epochs`, `compaction_checkpoints`, `retained_payloads` reads
//! for export, and `event_outbox` reads. All reads are keyset-bounded; all
//! writes are single statements so `&Connection` suffices. The deferred FK on
//! `compaction_checkpoints(session_pk, boundary_message_seq)` is enforced by
//! SQLite at statement commit, so a bad boundary seq surfaces as Err.
//!
//! ponytail: errors reuse `invalid_input()` (Sqlite InvalidQuery) instead of
//! dedicated StorageError variants to avoid widening the shared error enum.
//! Upgrade path: add snapshot-specific variants when the error surface is
//! versioned.

use rusqlite::{params, Connection};

use crate::StorageError;

const MAX_PAGE_SIZE: usize = 500;
const MAX_SNAPSHOT_READ: usize = 200;

pub struct SnapshotV2;

impl SnapshotV2 {
    /// Open a new context epoch; returns the epoch number (max+1).
    /// A second open while one is open fails via `context_open_idx`.
    pub fn open_epoch(
        connection: &Connection,
        session_pk: i64,
        baseline_payload_pk: i64,
        snapshot_payload_pk: i64,
        now_us: i64,
    ) -> Result<i64, StorageError> {
        connection.execute(
            "INSERT INTO context_epochs
             (session_pk, epoch, baseline_payload_pk, snapshot_payload_pk, created_at_us)
             SELECT ?1, COALESCE(MAX(epoch), 0) + 1, ?2, ?3, ?4
             FROM context_epochs WHERE session_pk = ?1",
            params![session_pk, baseline_payload_pk, snapshot_payload_pk, now_us],
        )?;
        let epoch: i64 = connection.query_row(
            "SELECT epoch FROM context_epochs WHERE pk = ?1",
            params![connection.last_insert_rowid()],
            |row| row.get(0),
        )?;
        Ok(epoch)
    }

    /// Close the single open epoch for a session. Errs if none is open.
    pub fn close_epoch(
        connection: &Connection,
        session_pk: i64,
        closed_us: i64,
    ) -> Result<(), StorageError> {
        let changed = connection.execute(
            "UPDATE context_epochs SET closed_at_us = ?1
             WHERE session_pk = ?2 AND closed_at_us IS NULL",
            params![closed_us, session_pk],
        )?;
        if changed != 1 {
            return Err(invalid_input());
        }
        Ok(())
    }

    /// Record a compaction checkpoint. Returns the checkpoint pk.
    /// The deferred FK to `messages(session_pk, seq)` rejects a boundary
    /// seq with no matching message at commit.
    pub fn checkpoint(
        connection: &Connection,
        session_pk: i64,
        boundary_seq: i64,
        summary_payload_pk: i64,
        recent: Option<i64>,
        now_us: i64,
    ) -> Result<i64, StorageError> {
        if boundary_seq <= 0 {
            return Err(invalid_input());
        }
        connection.execute(
            "INSERT INTO compaction_checkpoints
             (session_pk, boundary_message_seq, summary_payload_pk,
              recent_payload_pk, created_at_us)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![session_pk, boundary_seq, summary_payload_pk, recent, now_us],
        )?;
        Ok(connection.last_insert_rowid())
    }

    /// Pin a payload for an owner/purpose. Duplicate pins error.
    pub fn pin(
        connection: &Connection,
        owner_id: &[u8],
        purpose: u8,
        payload_pk: i64,
        now_us: i64,
    ) -> Result<(), StorageError> {
        if purpose > 2 {
            return Err(invalid_input());
        }
        connection.execute(
            "INSERT INTO retained_payloads (owner_id, purpose, payload_pk, created_at_us)
             VALUES (?1, ?2, ?3, ?4)",
            params![owner_id, i64::from(purpose), payload_pk, now_us],
        )?;
        Ok(())
    }

    /// Release a pin. Errs if the pin does not exist.
    pub fn unpin(
        connection: &Connection,
        owner_id: &[u8],
        purpose: u8,
        payload_pk: i64,
    ) -> Result<(), StorageError> {
        let changed = connection.execute(
            "DELETE FROM retained_payloads
             WHERE owner_id = ?1 AND purpose = ?2 AND payload_pk = ?3",
            params![owner_id, i64::from(purpose), payload_pk],
        )?;
        if changed != 1 {
            return Err(invalid_input());
        }
        Ok(())
    }

    /// List epochs for a session, newest epoch first.
    /// Limit clamped to 1..=200. Returns `(epoch, created_at_us, closed_at_us)`.
    pub fn list_epochs(
        connection: &Connection,
        session_pk: i64,
        limit: usize,
    ) -> Result<Vec<(i64, i64, Option<i64>)>, StorageError> {
        let limit = limit.clamp(1, MAX_SNAPSHOT_READ) as i64;
        let mut statement = connection.prepare(
            "SELECT epoch, created_at_us, closed_at_us FROM context_epochs
             WHERE session_pk = ?1
             ORDER BY epoch DESC LIMIT ?2",
        )?;
        let rows = statement.query_map(params![session_pk, limit], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, Option<i64>>(2)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Newest checkpoint for a session by boundary seq, or None when absent.
    /// Returns `(boundary_seq, summary_payload_pk, created_at_us, recent_count)`.
    /// `recent_count` is 1 when `recent_payload_pk` is set, else 0.
    pub fn latest_checkpoint(
        connection: &Connection,
        session_pk: i64,
    ) -> Result<Option<(i64, i64, i64, Option<i64>)>, StorageError> {
        let mut statement = connection.prepare(
            "SELECT boundary_message_seq, summary_payload_pk, created_at_us,
                    CASE WHEN recent_payload_pk IS NULL THEN 0 ELSE 1 END
             FROM compaction_checkpoints
             WHERE session_pk = ?1
             ORDER BY boundary_message_seq DESC LIMIT 1",
        )?;
        let mut rows = statement.query_map(params![session_pk], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<i64>>(3)?,
            ))
        })?;
        match rows.next() {
            None => Ok(None),
            Some(row) => Ok(Some(row?)),
        }
    }

    /// List `(owner_id, purpose)` pins for a payload. Limit clamped to 1..=200.
    pub fn list_pins(
        connection: &Connection,
        payload_pk: i64,
        limit: usize,
    ) -> Result<Vec<(Vec<u8>, i64)>, StorageError> {
        let limit = limit.clamp(1, MAX_SNAPSHOT_READ) as i64;
        let mut statement = connection.prepare(
            "SELECT owner_id, purpose FROM retained_payloads
             WHERE payload_pk = ?1
             ORDER BY owner_id ASC, purpose ASC LIMIT ?2",
        )?;
        let rows = statement.query_map(params![payload_pk, limit], |row| {
            Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, i64>(1)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Keyset export of `(seq, message_pk)` after `after_seq`.
    /// Limit clamped to 1..=500. Watermark is max seq, or `after_seq` empty.
    pub fn export_page(
        connection: &Connection,
        session_pk: i64,
        after_seq: i64,
        limit: usize,
    ) -> Result<(Vec<(i64, i64)>, i64), StorageError> {
        let limit = limit.clamp(1, MAX_PAGE_SIZE) as i64;
        let mut statement = connection.prepare(
            "SELECT seq, pk FROM messages
             WHERE session_pk = ?1 AND seq > ?2
             ORDER BY seq ASC LIMIT ?3",
        )?;
        let rows = statement.query_map(params![session_pk, after_seq, limit], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        })?;
        let mut out = Vec::new();
        let mut watermark = after_seq;
        for row in rows {
            let (seq, pk) = row?;
            if seq > watermark {
                watermark = seq;
            }
            out.push((seq, pk));
        }
        Ok((out, watermark))
    }

    /// Bounded outbox read: `(seq, payload_json)` with `seq > after_seq`
    /// and `seq <= upto_head`, ordered by seq. Watermark is last seq seen,
    /// or `after_seq` when empty.
    pub fn outbox_page(
        connection: &Connection,
        session_id: Option<&[u8]>,
        after_seq: i64,
        upto_head: i64,
        limit: usize,
    ) -> Result<(Vec<(i64, String)>, i64), StorageError> {
        let limit = limit.clamp(1, MAX_PAGE_SIZE) as i64;
        let mut out = Vec::new();
        let mut watermark = after_seq;
        match session_id {
            Some(id) => {
                let mut statement = connection.prepare(
                    "SELECT seq, payload_json FROM event_outbox
                     WHERE seq > ?1 AND seq <= ?2 AND session_id = ?3
                     ORDER BY seq ASC LIMIT ?4",
                )?;
                let rows = statement
                    .query_map(params![after_seq, upto_head, id, limit], |row| {
                        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                    })?;
                for row in rows {
                    let (seq, payload) = row?;
                    if seq > watermark {
                        watermark = seq;
                    }
                    out.push((seq, payload));
                }
            }
            None => {
                let mut statement = connection.prepare(
                    "SELECT seq, payload_json FROM event_outbox
                     WHERE seq > ?1 AND seq <= ?2
                     ORDER BY seq ASC LIMIT ?3",
                )?;
                let rows = statement.query_map(params![after_seq, upto_head, limit], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?;
                for row in rows {
                    let (seq, payload) = row?;
                    if seq > watermark {
                        watermark = seq;
                    }
                    out.push((seq, payload));
                }
            }
        }
        Ok((out, watermark))
    }
}

fn invalid_input() -> StorageError {
    StorageError::Sqlite(rusqlite::Error::InvalidQuery)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencode_rk_contracts::{MessageId, MessageRole, PayloadRef, SessionId};
    use std::path::Path;

    fn workspace(path: &Path) -> Connection {
        crate::schema_v2::SchemaV2::initialize_workspace(path, [1_u8; 16], [2_u8; 16], 10).unwrap()
    }

    fn make_session(conn: &mut Connection, title: &str) -> (SessionId, i64) {
        let id = SessionId::new();
        crate::writer_v2::V2Writer::create_session(
            conn,
            &crate::writer_v2::NewSession {
                id,
                title: title.to_owned(),
                created_at_us: 100,
                updated_at_us: 100,
            },
        )
        .unwrap();
        let pk: i64 = conn
            .query_row(
                "SELECT pk FROM sessions WHERE id = ?1",
                params![id.as_uuid().as_bytes().as_slice()],
                |row| row.get(0),
            )
            .unwrap();
        (id, pk)
    }

    fn append(conn: &mut Connection, session: SessionId, body: &str) {
        crate::writer_v2::V2Writer::append_message(
            conn,
            &crate::writer_v2::NewMessage {
                id: MessageId::new(),
                session_id: session,
                role: MessageRole::User,
                body: PayloadRef::Inline {
                    text: body.to_owned(),
                },
                created_at_us: 1000,
            },
        )
        .unwrap();
    }

    fn mk_payload(conn: &Connection, data: &[u8]) -> i64 {
        conn.query_row(
            "INSERT INTO payloads (inline_data, raw_bytes, created_at_us)
             VALUES (?1, ?2, 1) RETURNING pk",
            params![data, data.len() as i64],
            |row| row.get(0),
        )
        .unwrap()
    }

    fn insert_outbox(conn: &Connection, seq: i64, session: Option<&[u8]>, payload: &str) {
        conn.execute(
            "INSERT INTO event_outbox (seq, session_id, kind, payload_json, created_at_us)
             VALUES (?1, ?2, 'k', ?3, 1)",
            params![seq, session, payload],
        )
        .unwrap();
    }

    #[test]
    fn second_open_epoch_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = workspace(&dir.path().join("w.db"));
        let (_, pk) = make_session(&mut conn, "s");
        let base = mk_payload(&conn, b"b");
        let snap = mk_payload(&conn, b"s");
        assert_eq!(SnapshotV2::open_epoch(&conn, pk, base, snap, 5).unwrap(), 1);
        assert!(
            SnapshotV2::open_epoch(&conn, pk, base, snap, 6).is_err(),
            "context_open_idx must reject a second open epoch"
        );
    }

    #[test]
    fn close_then_reopen_increments_epoch() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = workspace(&dir.path().join("w.db"));
        let (_, pk) = make_session(&mut conn, "s");
        let base = mk_payload(&conn, b"b");
        let snap = mk_payload(&conn, b"s");
        SnapshotV2::open_epoch(&conn, pk, base, snap, 5).unwrap();
        SnapshotV2::close_epoch(&conn, pk, 7).unwrap();
        assert_eq!(SnapshotV2::open_epoch(&conn, pk, base, snap, 8).unwrap(), 2);
    }

    #[test]
    fn checkpoint_bad_seq_errs_at_commit() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = workspace(&dir.path().join("w.db"));
        let (id, pk) = make_session(&mut conn, "s");
        append(&mut conn, id, "m1");
        append(&mut conn, id, "m2");
        let summary = mk_payload(&conn, b"sum");
        assert!(
            SnapshotV2::checkpoint(&conn, pk, 99, summary, None, 9).is_err(),
            "deferred FK must reject a boundary seq with no message"
        );
        let got = SnapshotV2::checkpoint(&conn, pk, 2, summary, Some(summary), 9).unwrap();
        assert!(got > 0);
    }

    #[test]
    fn pin_unpin_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let conn = workspace(&dir.path().join("w.db"));
        let payload = mk_payload(&conn, b"p");
        let owner = [7_u8; 16];
        SnapshotV2::pin(&conn, &owner, 1, payload, 3).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM retained_payloads", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
        SnapshotV2::unpin(&conn, &owner, 1, payload).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM retained_payloads", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn empty_export_watermark_eq_after_seq() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = workspace(&dir.path().join("w.db"));
        let (_, pk) = make_session(&mut conn, "s");
        let (rows, watermark) = SnapshotV2::export_page(&conn, pk, 41, 10).unwrap();
        assert!(rows.is_empty());
        assert_eq!(watermark, 41);
    }

    #[test]
    fn export_page_keyset_paging() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = workspace(&dir.path().join("w.db"));
        let (id, pk) = make_session(&mut conn, "s");
        append(&mut conn, id, "m1");
        append(&mut conn, id, "m2");
        append(&mut conn, id, "m3");
        let (page1, wm1) = SnapshotV2::export_page(&conn, pk, 0, 2).unwrap();
        assert_eq!(page1.len(), 2);
        assert_eq!(page1[0].0, 1);
        assert_eq!(page1[1].0, 2);
        assert_eq!(wm1, 2);
        let (page2, wm2) = SnapshotV2::export_page(&conn, pk, wm1, 2).unwrap();
        assert_eq!(page2.len(), 1);
        assert_eq!(page2[0].0, 3);
        assert_eq!(wm2, 3);
    }

    #[test]
    fn outbox_page_bounded_by_head_and_session() {
        let dir = tempfile::tempdir().unwrap();
        let conn = workspace(&dir.path().join("w.db"));
        let a = [11_u8; 16];
        let b = [22_u8; 16];
        insert_outbox(&conn, 1, Some(&a), r#"{"n":1}"#);
        insert_outbox(&conn, 2, Some(&a), r#"{"n":2}"#);
        insert_outbox(&conn, 3, Some(&b), r#"{"n":3}"#);
        let (rows, wm) = SnapshotV2::outbox_page(&conn, Some(&a), 0, 100, 10).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], (1, r#"{"n":1}"#.to_owned()));
        assert_eq!(rows[1], (2, r#"{"n":2}"#.to_owned()));
        assert_eq!(wm, 2);
        let (rows, wm) = SnapshotV2::outbox_page(&conn, None, 0, 2, 10).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(wm, 2);
        let (rows, wm) = SnapshotV2::outbox_page(&conn, Some(&a), 99, 100, 10).unwrap();
        assert!(rows.is_empty());
        assert_eq!(wm, 99);
    }

    #[test]
    fn list_epochs_newest_first_and_limit() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = workspace(&dir.path().join("w.db"));
        let (_, pk) = make_session(&mut conn, "s");
        let base = mk_payload(&conn, b"b");
        let snap = mk_payload(&conn, b"s");
        SnapshotV2::open_epoch(&conn, pk, base, snap, 5).unwrap();
        SnapshotV2::close_epoch(&conn, pk, 6).unwrap();
        SnapshotV2::open_epoch(&conn, pk, base, snap, 7).unwrap();
        SnapshotV2::close_epoch(&conn, pk, 8).unwrap();
        SnapshotV2::open_epoch(&conn, pk, base, snap, 9).unwrap();
        let epochs = SnapshotV2::list_epochs(&conn, pk, 200).unwrap();
        assert_eq!(epochs.len(), 3);
        assert_eq!(epochs[0].0, 3);
        assert_eq!(epochs[1].0, 2);
        assert_eq!(epochs[2].0, 1);
        assert_eq!(epochs[0].1, 9);
        assert!(epochs[0].2.is_none(), "open epoch must have NULL closed_at_us");
        assert_eq!(epochs[1].2, Some(8));
        assert_eq!(epochs[2].2, Some(6));
        let one = SnapshotV2::list_epochs(&conn, pk, 1).unwrap();
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].0, 3);
    }

    #[test]
    fn closed_epoch_shows_closed_at_us() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = workspace(&dir.path().join("w.db"));
        let (_, pk) = make_session(&mut conn, "s");
        let base = mk_payload(&conn, b"b");
        let snap = mk_payload(&conn, b"s");
        SnapshotV2::open_epoch(&conn, pk, base, snap, 5).unwrap();
        SnapshotV2::close_epoch(&conn, pk, 11).unwrap();
        let epochs = SnapshotV2::list_epochs(&conn, pk, 10).unwrap();
        assert_eq!(epochs.len(), 1);
        assert_eq!(epochs[0], (1, 5, Some(11)));
    }

    #[test]
    fn latest_checkpoint_none_then_some() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = workspace(&dir.path().join("w.db"));
        let (id, pk) = make_session(&mut conn, "s");
        assert!(SnapshotV2::latest_checkpoint(&conn, pk).unwrap().is_none());
        append(&mut conn, id, "m1");
        append(&mut conn, id, "m2");
        let summary = mk_payload(&conn, b"sum");
        SnapshotV2::checkpoint(&conn, pk, 2, summary, None, 9).unwrap();
        let got = SnapshotV2::latest_checkpoint(&conn, pk).unwrap().unwrap();
        assert_eq!(got, (2, summary, 9, Some(0)));
        let recent = mk_payload(&conn, b"recent");
        append(&mut conn, id, "m3");
        SnapshotV2::checkpoint(&conn, pk, 3, summary, Some(recent), 10).unwrap();
        let got = SnapshotV2::latest_checkpoint(&conn, pk).unwrap().unwrap();
        assert_eq!(got, (3, summary, 10, Some(1)));
    }

    #[test]
    fn list_pins_empty_then_populated() {
        let dir = tempfile::tempdir().unwrap();
        let conn = workspace(&dir.path().join("w.db"));
        let payload = mk_payload(&conn, b"p");
        assert!(SnapshotV2::list_pins(&conn, payload, 10).unwrap().is_empty());
        let owner_a = [7_u8; 16];
        let owner_b = [9_u8; 16];
        SnapshotV2::pin(&conn, &owner_a, 1, payload, 3).unwrap();
        SnapshotV2::pin(&conn, &owner_b, 2, payload, 4).unwrap();
        let pins = SnapshotV2::list_pins(&conn, payload, 10).unwrap();
        assert_eq!(pins.len(), 2);
        assert_eq!(pins[0], (owner_a.to_vec(), 1));
        assert_eq!(pins[1], (owner_b.to_vec(), 2));
        let one = SnapshotV2::list_pins(&conn, payload, 1).unwrap();
        assert_eq!(one.len(), 1);
    }
}
