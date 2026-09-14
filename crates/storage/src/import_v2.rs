//! Format-1 (read-only source) to format-2 importer: paged, resumable, verified.
//!
//! The source Connection is opened by the caller and is strictly read-only here:
//! this module never writes to it and never opens a user database. The destination
//! session is expected to already exist in the format-2 workspace (created by
//! `V2Writer::create_session`); this importer only appends its messages against the
//! given `dest_session_pk` and advances `next_message_seq`.
//!
//! `ponytail:` fail-closed importer. Source blob-backed messages (inline is NULL)
//! and inline payloads wider than the v2 8 KiB inline ceiling cannot be copied
//! faithfully without the source blob root or a destination blob spool, so
//! `page` returns `Err(StorageError::InlinePayloadTooLarge)` instead of writing
//! a zero-byte payload with provenance metadata. A blob-copy lane can lift this
//! once the caller supplies a source blob store and a destination CAS writer.
//! A mid-import failure leaves the already-committed prefix in place; the
//! offending row and everything after it are not written.
use opencode_rk_contracts::MessageId;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::StorageError;

/// Bounded page size for reading format-1 messages (docs/STORAGE.md gates).
const PAGE_BUDGET: i64 = 500;
/// v2 inline payload ceiling (schema/v2/workspace.sql payloads CHECK).
const V2_INLINE_CEILING: usize = 8192;

pub struct ImportV2;

impl ImportV2 {
    /// Copy every message of a format-1 session (found by its TEXT id) into the
    /// format-2 session identified by `dest_session_pk`, paging the source with a
    /// bounded LIMIT. Returns the number of messages imported.
    pub fn import_session(
        dest: &mut Connection,
        source: &Connection,
        session_id: &str,
        new_session_id: &[u8; 16],
        dest_session_pk: i64,
    ) -> Result<u64, StorageError> {
        // Integrity guard: the dest session row must exist and match the caller's id.
        let existing: Option<Vec<u8>> = dest
            .query_row(
                "SELECT id FROM sessions WHERE pk=?1",
                params![dest_session_pk],
                |row| row.get(0),
            )
            .optional()?;
        let Some(existing) = existing else {
            return Err(StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows));
        };
        if existing.as_slice() != new_session_id {
            return Err(StorageError::Sqlite(rusqlite::Error::InvalidQuery));
        }

        let mut total: u64 = 0;
        let mut cursor: i64 = 0;
        loop {
            let (imported, next) = page(dest, source, session_id, dest_session_pk, cursor)?;
            total += imported;
            if imported == 0 || next == cursor {
                break; // no progress: exhausted
            }
            cursor = next;
        }
        Ok(total)
    }

    /// Whole-session message-count verification: `COUNT(*)` over the destination
    /// session must equal `expected_messages` or the call fails.
    ///
    /// Counts only: a matching count does not prove bodies survived. Pair with
    /// `verify_payload_bytes` when the source byte total is known.
    pub fn verify_counts(
        dest: &Connection,
        dest_session_pk: i64,
        expected_messages: u64,
    ) -> Result<(), StorageError> {
        let count: i64 = dest.query_row(
            "SELECT COUNT(*) FROM messages WHERE session_pk=?1",
            params![dest_session_pk],
            |row| row.get(0),
        )?;
        if count as u64 != expected_messages {
            return Err(StorageError::Sqlite(rusqlite::Error::InvalidQuery));
        }
        Ok(())
    }

    /// Whole-session payload-byte verification: `SUM(payloads.raw_bytes)` over
    /// `message_parts` of the destination session must equal `expected_bytes`.
    /// Catches body loss that `verify_counts` cannot see.
    pub fn verify_payload_bytes(
        dest: &Connection,
        dest_session_pk: i64,
        expected_bytes: u64,
    ) -> Result<(), StorageError> {
        let total: i64 = dest.query_row(
            "SELECT COALESCE(SUM(p.raw_bytes),0) FROM messages m \
              JOIN message_parts mp ON mp.message_pk=m.pk \
              JOIN payloads p ON p.pk=mp.payload_pk \
              WHERE m.session_pk=?1",
            params![dest_session_pk],
            |row| row.get(0),
        )?;
        if total as u64 != expected_bytes {
            return Err(StorageError::Sqlite(rusqlite::Error::InvalidQuery));
        }
        Ok(())
    }

    /// Import one bounded page of format-1 messages with `rowid > after_message_rowid`
    /// (resumable cursor). Returns `(imported_this_page, next_cursor)` where
    /// `next_cursor` is the last source rowid read; re-invoking with `next_cursor`
    /// resumes seamlessly.
    ///
    /// The public signature carries no source session id, so the page is scoped to
    /// the next source session encountered at the cursor. This is exact for a source
    /// connection holding a single format-1 session; for multi-session sources use
    /// `import_session`, which scopes by TEXT id: `import_is_resumable` can
    /// otherwise land another session's messages into this destination session.
    /// `ponytail:` a per-session cursor would need the source session id threaded
    /// into this signature.
    pub fn import_is_resumable(
        dest: &mut Connection,
        source: &Connection,
        dest_session_pk: i64,
        after_message_rowid: i64,
    ) -> Result<(u64, i64), StorageError> {
        let next_session: Option<String> = source
            .query_row(
                "SELECT session_id FROM messages WHERE rowid>?1 ORDER BY rowid LIMIT 1",
                params![after_message_rowid],
                |row| row.get(0),
            )
            .optional()?;
        let Some(next_session) = next_session else {
            return Ok((0, after_message_rowid));
        };
        page(
            dest,
            source,
            &next_session,
            dest_session_pk,
            after_message_rowid,
        )
    }
}

/// Read one bounded, session-scoped page of format-1 messages and append them to v2.
fn page(
    dest: &mut Connection,
    source: &Connection,
    session_id: &str,
    dest_session_pk: i64,
    after_message_rowid: i64,
) -> Result<(u64, i64), StorageError> {
    let mut statement = source.prepare(
        "SELECT rowid,role,inline_text,blob_hash,byte_len,created_at \
         FROM messages WHERE session_id=?1 AND rowid>?2 \
         ORDER BY rowid ASC LIMIT ?3",
    )?;
    let rows = statement.query_map(
        params![session_id, after_message_rowid, PAGE_BUDGET],
        |row| {
            let rowid: i64 = row.get(0)?;
            let role: String = row.get(1)?;
            let inline: Option<String> = row.get(2)?;
            let blob: Option<String> = row.get(3)?;
            let byte_len: i64 = row.get(4)?;
            let created_at: String = row.get(5)?;
            Ok((rowid, role, inline, blob, byte_len, created_at))
        },
    )?;

    let mut imported: u64 = 0;
    let mut next_cursor: i64 = after_message_rowid;
    for result in rows {
        let (rowid, role, inline, _blob, _byte_len, created_at) = result?;
        let created_us = parse_us(&created_at)?;
        let role_i = encode_role(&role)
            .ok_or_else(|| StorageError::Sqlite(rusqlite::Error::InvalidQuery))?;
        // Fail closed: blob-backed rows (inline is NULL) and inline bodies over
        // the v2 ceiling cannot be represented faithfully, so reject them with
        // the same variant `V2Writer::append_message` uses. Never write a
        // zero-byte placeholder that `verify_counts` would accept as success.
        let body: Vec<u8> = match inline {
            Some(text) => {
                let bytes = text.into_bytes();
                if bytes.len() > V2_INLINE_CEILING {
                    return Err(StorageError::InlinePayloadTooLarge);
                }
                bytes
            }
            None => return Err(StorageError::InlinePayloadTooLarge),
        };
        append_to_v2(dest, dest_session_pk, role_i, &body, created_us, None)?;
        imported += 1;
        next_cursor = rowid;
    }
    Ok((imported, next_cursor))
}

/// Append one v2 message (messages + payloads + message_parts) against an existing
/// session, mirroring `V2Writer::append_message` but keyed by `session_pk` directly.
fn append_to_v2(
    dest: &mut Connection,
    dest_session_pk: i64,
    role: i64,
    body: &[u8],
    created_us: i64,
    metadata_json: Option<&str>,
) -> Result<(), StorageError> {
    if body.len() > V2_INLINE_CEILING {
        return Err(StorageError::InlinePayloadTooLarge);
    }
    let transaction = dest.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let sequence: i64 = transaction
        .query_row(
            "SELECT next_message_seq FROM sessions WHERE pk=?1",
            params![dest_session_pk],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows))?;
    let next_sequence = sequence
        .checked_add(1)
        .ok_or_else(|| StorageError::Sqlite(rusqlite::Error::InvalidQuery))?;
    transaction.execute(
        "UPDATE sessions SET next_message_seq=?1, updated_at_us=?2 WHERE pk=?3",
        params![next_sequence, created_us, dest_session_pk],
    )?;
    let message_id = MessageId::new();
    transaction.execute(
        "INSERT INTO messages (id,session_pk,seq,role,status,created_at_us,completed_at_us) \
         VALUES (?1,?2,?3,?4,1,?5,?5)",
        params![
            message_id.as_uuid().as_bytes().as_slice(),
            dest_session_pk,
            sequence,
            role,
            created_us
        ],
    )?;
    let message_pk = transaction.last_insert_rowid();
    transaction.execute(
        "INSERT INTO payloads (inline_data,raw_bytes,created_at_us) VALUES (?1,?2,?3)",
        params![body, body.len() as i64, created_us],
    )?;
    let payload_pk = transaction.last_insert_rowid();
    transaction.execute(
        "INSERT INTO message_parts (message_pk,ordinal,kind,payload_pk,metadata_json) \
         VALUES (?1,0,0,?2,?3)",
        params![message_pk, payload_pk, metadata_json],
    )?;
    transaction.commit()?;
    Ok(())
}

fn parse_us(value: &str) -> Result<i64, StorageError> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.timestamp_micros())
        .map_err(|_| StorageError::Sqlite(rusqlite::Error::InvalidQuery))
}

fn encode_role(value: &str) -> Option<i64> {
    match value {
        "system" => Some(0),
        "user" => Some(1),
        "assistant" => Some(2),
        "tool" => Some(3),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema_v2::SchemaV2;
    use crate::writer_v2::{NewSession, V2Writer};
    use opencode_rk_contracts::SessionId;
    use rusqlite::Connection;
    use tempfile::tempdir;

    const SOURCE_DDL: &str = "
        CREATE TABLE sessions(id TEXT PRIMARY KEY,title TEXT NOT NULL,state TEXT NOT NULL,
            created_at TEXT NOT NULL,updated_at TEXT NOT NULL,archived_at TEXT);
        CREATE TABLE messages(id TEXT PRIMARY KEY,session_id TEXT NOT NULL REFERENCES sessions(id),
            role TEXT NOT NULL,inline_text TEXT,blob_hash TEXT,byte_len INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            CHECK((inline_text IS NULL)!=(blob_hash IS NULL)));
    ";

    fn now_rfc3339() -> String {
        "2026-01-01T00:00:00Z".to_string()
    }

    /// Format-1 source Connection with one session and `count` inline messages.
    fn source_db(count: u64) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SOURCE_DDL).unwrap();
        conn.execute(
            "INSERT INTO sessions(id,title,state,created_at,updated_at,archived_at) \
             VALUES ('src1','s','active',?1,?1,NULL)",
            params![now_rfc3339()],
        )
        .unwrap();
        for i in 0..count {
            let mid = format!("m{i}");
            conn.execute(
                "INSERT INTO messages(id,session_id,role,inline_text,blob_hash,byte_len,created_at) \
                 VALUES (?1,'src1','user',?2,NULL,?3,?4)",
                params![mid, format!("hello {i}"), 8, now_rfc3339()],
            )
            .unwrap();
        }
        conn
    }

    /// Format-2 workspace with one created session; returns (dest, session_pk, new_id).
    fn dest_db() -> (Connection, i64, [u8; 16]) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("workspace.db");
        let mut conn = SchemaV2::initialize_workspace(&path, [1_u8; 16], [2_u8; 16], 10).unwrap();
        let sid = SessionId::new();
        let new_id = *sid.as_uuid().as_bytes();
        let ns = NewSession {
            id: sid,
            title: "imported".to_string(),
            created_at_us: 10,
            updated_at_us: 10,
        };
        V2Writer::create_session(&mut conn, &ns).unwrap();
        let pk: i64 = conn
            .query_row(
                "SELECT pk FROM sessions WHERE id=?1",
                params![&new_id[..]],
                |r| r.get(0),
            )
            .unwrap();
        (conn, pk, new_id)
    }

    #[test]
    fn import_copies_messages() {
        let source = source_db(3);
        let (mut dest, pk, new_id) = dest_db();
        let imported = ImportV2::import_session(&mut dest, &source, "src1", &new_id, pk).unwrap();
        assert_eq!(imported, 3);
        ImportV2::verify_counts(&dest, pk, 3).unwrap();
        drop(source);
    }

    #[test]
    fn verify_counts_detects_mismatch() {
        let source = source_db(2);
        let (mut dest, pk, new_id) = dest_db();
        let imported = ImportV2::import_session(&mut dest, &source, "src1", &new_id, pk).unwrap();
        assert_eq!(imported, 2);
        assert!(ImportV2::verify_counts(&dest, pk, 3).is_err());
    }

    #[test]
    fn resumable_pages() {
        let source = source_db(1200);
        let (mut dest, pk, _new_id) = dest_db();
        // first page is bounded by PAGE_BUDGET
        let (imported, next) = ImportV2::import_is_resumable(&mut dest, &source, pk, 0).unwrap();
        assert_eq!(imported, PAGE_BUDGET as u64);
        assert!(next > 0);
        // drain the rest to completion
        let mut total = imported;
        let mut cursor = next;
        loop {
            let (i, n) = ImportV2::import_is_resumable(&mut dest, &source, pk, cursor).unwrap();
            total += i;
            if i == 0 || n == cursor {
                break;
            }
            cursor = n;
        }
        assert_eq!(total, 1200);
        ImportV2::verify_counts(&dest, pk, 1200).unwrap();
    }

    #[test]
    fn import_rejects_blob_backed_row() {
        let source = source_db(1);
        source
            .execute(
                "INSERT INTO messages(id,session_id,role,inline_text,blob_hash,byte_len,created_at) \
                 VALUES ('blob1','src1','user',NULL,'deadbeef',100,?1)",
                params![now_rfc3339()],
            )
            .unwrap();
        let (mut dest, pk, new_id) = dest_db();
        let err = ImportV2::import_session(&mut dest, &source, "src1", &new_id, pk).unwrap_err();
        assert!(matches!(err, StorageError::InlinePayloadTooLarge));
        let zero_bytes: i64 = dest
            .query_row("SELECT COUNT(*) FROM payloads WHERE raw_bytes=0", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(zero_bytes, 0, "no zero-byte placeholder may be written");
    }

    #[test]
    fn import_rejects_oversize_inline_row() {
        let source = source_db(0);
        let big = "x".repeat(V2_INLINE_CEILING + 1);
        let big_len = big.len() as i64;
        source
            .execute(
                "INSERT INTO messages(id,session_id,role,inline_text,blob_hash,byte_len,created_at) \
                 VALUES ('big1','src1','user',?1,NULL,?2,?3)",
                params![big, big_len, now_rfc3339()],
            )
            .unwrap();
        let (mut dest, pk, new_id) = dest_db();
        let err = ImportV2::import_session(&mut dest, &source, "src1", &new_id, pk).unwrap_err();
        assert!(matches!(err, StorageError::InlinePayloadTooLarge));
        ImportV2::verify_counts(&dest, pk, 0).unwrap();
    }

    #[test]
    fn verify_payload_bytes_accepts_exact_sum_rejects_others() {
        let source = source_db(3);
        let (mut dest, pk, new_id) = dest_db();
        ImportV2::import_session(&mut dest, &source, "src1", &new_id, pk).unwrap();
        // "hello 0".."hello 2" are 7 bytes each.
        ImportV2::verify_payload_bytes(&dest, pk, 21).unwrap();
        assert!(ImportV2::verify_payload_bytes(&dest, pk, 22).is_err());
        assert!(ImportV2::verify_payload_bytes(&dest, pk, 0).is_err());
    }

    #[test]
    fn source_is_unchanged() {
        let source = source_db(4);
        let before: i64 = source
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE session_id='src1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let (mut dest, pk, new_id) = dest_db();
        ImportV2::import_session(&mut dest, &source, "src1", &new_id, pk).unwrap();
        let after: i64 = source
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE session_id='src1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(before, 4);
        assert_eq!(after, before);
        let sample: String = source
            .query_row("SELECT inline_text FROM messages WHERE id='m1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(sample, "hello 1");
    }
}
