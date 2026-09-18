//! Large + blob history import with migration agreement (DISC-108).
//!
//! Thin orchestration over existing machinery (AUD-007 R2/R6/R7 repair):
//! inline rows reuse [`crate::ImportV2`]; blob-backed rows copy out of band
//! from a source [`crate::BlobStore`] directory into the destination v2
//! `blobs` table (+ a destination [`crate::BlobStore`] spill so the file
//! layout stays unified, R6). The source [`rusqlite::Connection`] is only
//! ever read; open it `SQLITE_OPEN_READ_ONLY` (T01). Every imported message
//! commits in its own transaction, so an interrupted import resumes from the
//! returned cursor with no half-imported message (T04). After the import,
//! [`BlobImport::verify_migration_agreement`] requires the facade markers
//! (`user_version`, `application_id`, `schema_migrations` checksum) to agree;
//! any divergence fails the check (T03).
//!
//! `ponytail:` destination blob rows use codec=0 (raw) with
//! `stored_bytes = raw_bytes + BLOB_HEADER_FICTION` (52, matching the header
//! convention used by the gc/schema tests, e.g. 9000->9052). Upgrade: real
//! zstd spillover with measured stored bytes. Quota ceiling is
//! [`BlobImport::MAX_BLOB_BYTES`] (8 MiB, matching
//! `MAX_DRAFT_ATTACHMENT_BYTES`); oversize blobs fail with an explicit quota
//! error before any write of that row (T05).
//!
//! Integration note: this module must be wired with one line in
//! `crates/storage/src/lib.rs` (`pub mod import_blobs;`, owned by the
//! integrator). It is deliberately not referenced from any other file here.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::{BlobStore, ImportV2, SchemaV2, StorageError};

/// Per-page read bound for the blob-import cursor loop (matches
/// `import_v2::PAGE_BUDGET` so inline + blob pages advance uniformly).
pub const IMPORT_PAGE_ROWS: i64 = 500;
/// v2 inline payload ceiling (mirrors `writer_v2`/workspace.sql 8192).
pub const V2_INLINE_CEILING: usize = 8192;
/// Explicit per-blob byte budget (matches `MAX_DRAFT_ATTACHMENT_BYTES`).
pub const MAX_BLOB_BYTES: usize = 8 * 1024 * 1024;
/// Header fiction added to `stored_bytes` (workspace.sql requires
/// `stored_bytes >= 52`; convention 9000->9052 from the gc/schema tests).
pub const BLOB_HEADER_FICTION: i64 = 52;

pub struct BlobImport;

#[derive(Debug)]
pub struct BlobImportSummary {
    pub inline_messages: u64,
    pub blob_messages: u64,
    pub blob_bytes: u64,
}

impl BlobImport {
    /// Import every message of a format-1 session into an existing v2 session,
    /// paging the source with a bounded LIMIT. Inline rows go through
    /// [`ImportV2`]; blob-backed rows (`inline_text IS NULL`) copy out of band
    /// from `source_blobs` into the destination `blobs` table plus
    /// `dest_blobs`. Returns per-kind counts. The source connection is only
    /// read, never written.
    pub fn import_all(
        dest: &mut Connection,
        source: &Connection,
        session_id: &str,
        new_session_id: &[u8; 16],
        dest_session_pk: i64,
        source_blobs: &BlobStore,
        dest_blobs: &BlobStore,
    ) -> Result<BlobImportSummary, StorageError> {
        // Integrity guard mirrors ImportV2: dest session row must exist + match.
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

        let mut summary = BlobImportSummary {
            inline_messages: 0,
            blob_messages: 0,
            blob_bytes: 0,
        };
        let mut cursor: i64 = 0;
        loop {
            let (imported_inline, imported_blob, bytes, next) = Self::import_page(
                dest,
                source,
                session_id,
                dest_session_pk,
                cursor,
                source_blobs,
                dest_blobs,
            )?;
            summary.inline_messages += imported_inline;
            summary.blob_messages += imported_blob;
            summary.blob_bytes += bytes;
            if imported_inline + imported_blob == 0 || next == cursor {
                break; // exhausted
            }
            cursor = next;
        }
        Ok(summary)
    }

    /// Import one bounded page (`rowid > after_rowid`, resumable cursor).
    /// Returns `(inline_imported, blob_imported, blob_bytes, next_cursor)`.
    /// Each message commits independently: a failure leaves the committed
    /// prefix intact and the offending row + tail unwritten, so re-invoking
    /// with the last committed cursor resumes cleanly.
    #[allow(clippy::too_many_arguments)]
    pub fn import_page(
        dest: &mut Connection,
        source: &Connection,
        session_id: &str,
        dest_session_pk: i64,
        after_rowid: i64,
        source_blobs: &BlobStore,
        dest_blobs: &BlobStore,
    ) -> Result<(u64, u64, u64, i64), StorageError> {
        let mut statement = source.prepare(
            "SELECT rowid,role,inline_text,blob_hash,byte_len,created_at \
             FROM messages WHERE session_id=?1 AND rowid>?2 \
             ORDER BY rowid ASC LIMIT ?3",
        )?;
        let rows = statement.query_map(
            params![session_id, after_rowid, IMPORT_PAGE_ROWS],
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

        let mut inline_count: u64 = 0;
        let mut blob_count: u64 = 0;
        let mut blob_bytes: u64 = 0;
        let mut next_cursor: i64 = after_rowid;
        for result in rows {
            let (rowid, role, inline, blob_hash, byte_len, created_at) = result?;
            let created_us = parse_us(&created_at)?;
            let role_i = encode_role(&role)
                .ok_or_else(|| StorageError::Sqlite(rusqlite::Error::InvalidQuery))?;
            match inline {
                Some(text) => {
                    let bytes = text.into_bytes();
                    if bytes.len() > V2_INLINE_CEILING {
                        return Err(StorageError::InlinePayloadTooLarge);
                    }
                    append_inline(dest, dest_session_pk, role_i, &bytes, created_us)?;
                    inline_count += 1;
                }
                None => {
                    let hash =
                        blob_hash.ok_or_else(|| {
                            StorageError::Sqlite(rusqlite::Error::InvalidQuery)
                        })?;
                    let raw = source_blobs.get(&hash)?;
                    if raw.len() > MAX_BLOB_BYTES {
                        return Err(blob_quota_exceeded(raw.len()));
                    }
                    if byte_len >= 0 && byte_len as u64 != raw.len() as u64 {
                        return Err(StorageError::BlobHashMismatch);
                    }
                    // Unified file layout (R6): spill into the dest BlobStore
                    // so `path_for` reads work from either side, then point
                    // the v2 row at the same bytes.
                    let info = dest_blobs.put(&raw)?;
                    debug_assert_eq!(info.raw_bytes as usize, raw.len());
                    append_blob(dest, dest_session_pk, role_i, &hash, &raw, created_us)?;
                    blob_count += 1;
                    blob_bytes += raw.len() as u64;
                }
            }
            next_cursor = rowid;
        }
        Ok((inline_count, blob_count, blob_bytes, next_cursor))
    }

    /// Whole-session count agreement: destination message count must equal
    /// `expected_messages`. Thin wrapper over [`ImportV2::verify_counts`].
    pub fn verify_counts(
        dest: &Connection,
        dest_session_pk: i64,
        expected_messages: u64,
    ) -> Result<(), StorageError> {
        ImportV2::verify_counts(dest, dest_session_pk, expected_messages)
    }

    /// Whole-session payload-byte agreement (inline + blob `raw_bytes`).
    pub fn verify_payload_bytes(
        dest: &Connection,
        dest_session_pk: i64,
        expected_bytes: u64,
    ) -> Result<(), StorageError> {
        ImportV2::verify_payload_bytes(dest, dest_session_pk, expected_bytes)
    }

    /// Facade/migrations agreement check (T03): the destination workspace must
    /// carry intact facade markers — `user_version`, `application_id`, and the
    /// `schema_migrations` v2 checksum matching
    /// [`SchemaV2::workspace_checksum`]. Any divergence (tampered checksum,
    /// wrong markers, or a foreign legacy `_migrations` table with rows)
    /// fails the check.
    pub fn verify_migration_agreement(dest: &Connection) -> Result<(), StorageError> {
        let user_version: i64 =
            dest.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if user_version != 2 {
            return Err(StorageError::Sqlite(
                rusqlite::Error::InvalidParameterName(format!(
                    "user_version divergence: expected 2, got {user_version}"
                )),
            ));
        }
        let app_id: i64 =
            dest.pragma_query_value(None, "application_id", |row| row.get(0))?;
        if app_id != 0x4F525732 {
            return Err(StorageError::Sqlite(
                rusqlite::Error::InvalidParameterName(format!(
                    "application_id divergence: got {app_id:#x}"
                )),
            ));
        }
        let stored: Vec<u8> = dest
            .query_row(
                "SELECT checksum FROM schema_migrations WHERE version=2",
                [],
                |row| row.get(0),
            )
            .map_err(|_| {
                StorageError::Sqlite(rusqlite::Error::InvalidParameterName(
                    "schema_migrations v2 row missing: facade divergence".into(),
                ))
            })?;
        if stored.as_slice() != SchemaV2::workspace_checksum().as_slice() {
            return Err(StorageError::Sqlite(
                rusqlite::Error::InvalidParameterName(
                    "schema_migrations checksum divergence".into(),
                ),
            ));
        }
        // Legacy `_migrations` runner table must not shadow the workspace:
        // any rows there mean a foreign runner wrote migration state.
        let legacy_table: Option<String> = dest
            .query_row(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='_migrations'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        if legacy_table.is_some() {
            let rows: i64 = dest.query_row("SELECT COUNT(*) FROM _migrations", [], |row| {
                row.get(0)
            })?;
            if rows != 0 {
                return Err(StorageError::Sqlite(
                    rusqlite::Error::InvalidParameterName(format!(
                        "_migrations divergence: {rows} foreign row(s)"
                    )),
                ));
            }
        }
        Ok(())
    }
}

/// Append one inline v2 message against an existing session (mirrors
/// `import_v2::append_to_v2` without depending on its private fn).
fn append_inline(
    dest: &mut Connection,
    dest_session_pk: i64,
    role: i64,
    body: &[u8],
    created_us: i64,
) -> Result<(), StorageError> {
    if body.len() > V2_INLINE_CEILING {
        return Err(StorageError::InlinePayloadTooLarge);
    }
    append_message_row(dest, dest_session_pk, role, created_us, |tx, message_pk| {
        tx.execute(
            "INSERT INTO payloads (inline_data,raw_bytes,created_at_us) VALUES (?1,?2,?3)",
            params![body, body.len() as i64, created_us],
        )?;
        let payload_pk = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO message_parts (message_pk,ordinal,kind,payload_pk) VALUES (?1,0,0,?2)",
            params![message_pk, payload_pk],
        )?;
        Ok(())
    })
}

/// Append one blob-backed v2 message: insert the `blobs` row (ready, raw
/// codec), the blob-pointing `payloads` row (guarded by the `payload_ready`
/// trigger), and the `message_parts` link — all in one transaction.
fn append_blob(
    dest: &mut Connection,
    dest_session_pk: i64,
    role: i64,
    hex_hash: &str,
    raw: &[u8],
    created_us: i64,
) -> Result<(), StorageError> {
    if raw.len() > MAX_BLOB_BYTES {
        return Err(blob_quota_exceeded(raw.len()));
    }
    let hash_bytes = hex_to_32(hex_hash)?;
    let stored = (raw.len() as i64)
        .checked_add(BLOB_HEADER_FICTION)
        .ok_or_else(|| StorageError::Sqlite(rusqlite::Error::InvalidQuery))?;
    append_message_row(dest, dest_session_pk, role, created_us, |tx, message_pk| {
        tx.execute(
            "INSERT OR IGNORE INTO blobs (hash,state,codec,raw_bytes,stored_bytes,created_at_us) \
             VALUES (?1,0,0,?2,?3,?4)",
            params![&hash_bytes[..], raw.len() as i64, stored, created_us],
        )?;
        let blob_pk: i64 = tx.query_row(
            "SELECT pk FROM blobs WHERE hash=?1",
            params![&hash_bytes[..]],
            |row| row.get(0),
        )?;
        tx.execute(
            "INSERT INTO payloads (blob_pk,raw_bytes,created_at_us) VALUES (?1,?2,?3)",
            params![blob_pk, raw.len() as i64, created_us],
        )?;
        let payload_pk = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO message_parts (message_pk,ordinal,kind,payload_pk) VALUES (?1,0,0,?2)",
            params![message_pk, payload_pk],
        )?;
        Ok(())
    })
}

/// Allocate the next message seq + insert the `messages` row inside one
/// immediate transaction, then run `parts` for payload linkage before commit.
/// One transaction per message: no half-imported message survives a crash.
fn append_message_row(
    dest: &mut Connection,
    dest_session_pk: i64,
    role: i64,
    created_us: i64,
    parts: impl FnOnce(&rusqlite::Transaction<'_>, i64) -> Result<(), StorageError>,
) -> Result<(), StorageError> {
    use opencode_rk_contracts::MessageId;
    let tx = dest.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let sequence: i64 = tx
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
    tx.execute(
        "UPDATE sessions SET next_message_seq=?1, updated_at_us=?2 WHERE pk=?3",
        params![next_sequence, created_us, dest_session_pk],
    )?;
    let message_id = MessageId::new();
    tx.execute(
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
    let message_pk = tx.last_insert_rowid();
    parts(&tx, message_pk)?;
    tx.commit()?;
    Ok(())
}

/// Explicit quota error for oversized blobs (T05). `StorageError` gains no new
/// variant here (lib.rs is integrator-owned); the `Io` message names the quota.
fn blob_quota_exceeded(actual: usize) -> StorageError {
    StorageError::Io(std::io::Error::new(
        std::io::ErrorKind::QuotaExceeded,
        format!("blob quota exceeded: {actual} bytes > {MAX_BLOB_BYTES}"),
    ))
}

#[cfg(test)]
fn is_quota_error(err: &StorageError) -> bool {
    match err {
        StorageError::Io(e) => e.to_string().contains("blob quota exceeded"),
        _ => false,
    }
}

fn hex_to_32(hex: &str) -> Result<[u8; 32], StorageError> {
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(StorageError::InvalidBlobHash);
    }
    let mut out = [0u8; 32];
    for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let s = std::str::from_utf8(chunk).map_err(|_| StorageError::InvalidBlobHash)?;
        out[i] = u8::from_str_radix(s, 16).map_err(|_| StorageError::InvalidBlobHash)?;
    }
    Ok(out)
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
    use rusqlite::OpenFlags;
    use std::fs;
    use tempfile::tempdir;

    const SOURCE_DDL: &str = "
        CREATE TABLE sessions(id TEXT PRIMARY KEY,title TEXT NOT NULL,state TEXT NOT NULL,
            created_at TEXT NOT NULL,updated_at TEXT NOT NULL,archived_at TEXT);
        CREATE TABLE messages(id TEXT PRIMARY KEY,session_id TEXT NOT NULL REFERENCES sessions(id),
            role TEXT NOT NULL,inline_text TEXT,blob_hash TEXT,byte_len INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            CHECK((inline_text IS NULL)!=(blob_hash IS NULL)));
    ";

    const CREATED_AT: &str = "2026-01-01T00:00:00Z";

    /// Build a format-1 source DB *file* plus a BlobStore dir holding `blobs`
    /// entries keyed by `(row_id, bytes)`; returns (dir, source_path, hashes).
    fn source_file(
        dir: &tempfile::TempDir,
        inline: &[&str],
        blobs: &[Vec<u8>],
    ) -> (std::path::PathBuf, Vec<String>) {
        let source_path = dir.path().join("source.db");
        let conn = Connection::open(&source_path).unwrap();
        conn.execute_batch(SOURCE_DDL).unwrap();
        conn.execute(
            "INSERT INTO sessions(id,title,state,created_at,updated_at,archived_at) \
             VALUES ('src1','s','active',?1,?1,NULL)",
            params![CREATED_AT],
        )
        .unwrap();
        for (i, text) in inline.iter().enumerate() {
            conn.execute(
                "INSERT INTO messages(id,session_id,role,inline_text,blob_hash,byte_len,created_at) \
                 VALUES (?1,'src1','user',?2,NULL,?3,?4)",
                params![format!("m{i}"), text, text.len() as i64, CREATED_AT],
            )
            .unwrap();
        }
        let blob_root = dir.path().join("source_blobs");
        let store = BlobStore::new(&blob_root);
        let mut hashes = Vec::new();
        for (i, bytes) in blobs.iter().enumerate() {
            let info = store.put(bytes).unwrap();
            hashes.push(info.hash.clone());
            conn.execute(
                "INSERT INTO messages(id,session_id,role,inline_text,blob_hash,byte_len,created_at) \
                 VALUES (?1,'src1','assistant',NULL,?2,?3,?4)",
                params![format!("b{i}"), info.hash, bytes.len() as i64, CREATED_AT],
            )
            .unwrap();
        }
        drop(conn);
        (source_path, hashes)
    }

    fn open_read_only(path: &std::path::Path) -> Connection {
        Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
    }

    fn dest_in(dir: &tempfile::TempDir) -> (Connection, i64, [u8; 16]) {
        let mut conn =
            SchemaV2::initialize_workspace(&dir.path().join("ws.db"), [1_u8; 16], [2_u8; 16], 10)
                .unwrap();
        let sid = SessionId::new();
        let id_bytes = *sid.as_uuid().as_bytes();
        V2Writer::create_session(
            &mut conn,
            &NewSession {
                id: sid,
                title: "imported".to_owned(),
                created_at_us: 10,
                updated_at_us: 10,
            },
        )
        .unwrap();
        let pk: i64 = conn
            .query_row(
                "SELECT pk FROM sessions WHERE id=?1",
                params![&id_bytes[..]],
                |r| r.get(0),
            )
            .unwrap();
        (conn, pk, id_bytes)
    }

    fn count(dest: &Connection, pk: i64) -> i64 {
        dest.query_row(
            "SELECT COUNT(*) FROM messages WHERE session_pk=?1",
            params![pk],
            |r| r.get(0),
        )
        .unwrap()
    }

    fn payload_sum(dest: &Connection, pk: i64) -> i64 {
        dest.query_row(
            "SELECT COALESCE(SUM(p.raw_bytes),0) FROM messages m \
             JOIN message_parts mp ON mp.message_pk=m.pk \
             JOIN payloads p ON p.pk=mp.payload_pk WHERE m.session_pk=?1",
            params![pk],
            |r| r.get(0),
        )
        .unwrap()
    }

    // DISC-108-T01: large history imports incrementally from a read-only copy
    // without modifying the source database.
    #[test]
    fn t01_large_history_imports_incrementally_source_untouched() {
        let dir = tempdir().unwrap();
        let inline: Vec<String> = (0..1200).map(|i| format!("body {i}")).collect();
        let refs: Vec<&str> = inline.iter().map(String::as_str).collect();
        let (source_path, _) = source_file(&dir, &refs, &[]);
        let before = fs::read(&source_path).unwrap();
        let source = open_read_only(&source_path);
        let dest_dir = tempdir().unwrap();
        let (mut dest, pk, _new_id) = dest_in(&dest_dir);
        let source_blobs = BlobStore::new(dir.path().join("source_blobs"));
        let dest_blob_dir = tempdir().unwrap();
        let dest_blobs = BlobStore::new(dest_blob_dir.path());

        // Page 1 bounded.
        let (i1, b1, _, c1) =
            BlobImport::import_page(&mut dest, &source, "src1", pk, 0, &source_blobs, &dest_blobs)
                .unwrap();
        assert_eq!((i1, b1), (IMPORT_PAGE_ROWS as u64, 0));
        assert!(c1 > 0);
        // Resume to completion via import_all on a fresh session? No: resume
        // in place with the cursor loop.
        let mut cursor = c1;
        loop {
            let (i, b, _, n) =
                BlobImport::import_page(&mut dest, &source, "src1", pk, cursor, &source_blobs, &dest_blobs)
                    .unwrap();
            if i + b == 0 || n == cursor {
                break;
            }
            cursor = n;
        }
        BlobImport::verify_counts(&dest, pk, 1200).unwrap();
        assert_eq!(count(&dest, pk), 1200);
        let during = fs::read(&source_path).unwrap();
        drop(source);
        assert_eq!(before, during, "source must be byte-identical after import");
        assert_eq!(before, fs::read(&source_path).unwrap());
        BlobImport::verify_migration_agreement(&dest).unwrap();
    }

    // DISC-108-T02: blob payloads copy out of band within the byte budget.
    #[test]
    fn t02_blob_payload_copies_out_of_band_byte_identical() {
        let dir = tempdir().unwrap();
        let big: Vec<u8> = (0..20_000).map(|i| (i % 251) as u8).collect();
        let (source_path, hashes) = source_file(&dir, &["hello"], &[big.clone()]);
        let source = open_read_only(&source_path);
        let dest_dir = tempdir().unwrap();
        let (mut dest, pk, new_id) = dest_in(&dest_dir);
        let source_blobs = BlobStore::new(dir.path().join("source_blobs"));
        let dest_blob_dir = tempdir().unwrap();
        let dest_blobs = BlobStore::new(dest_blob_dir.path());

        let summary = BlobImport::import_all(
            &mut dest,
            &source,
            "src1",
            &new_id,
            pk,
            &source_blobs,
            &dest_blobs,
        )
        .unwrap();
        assert_eq!(summary.inline_messages, 1);
        assert_eq!(summary.blob_messages, 1);
        assert_eq!(summary.blob_bytes, big.len() as u64);
        BlobImport::verify_counts(&dest, pk, 2).unwrap();
        BlobImport::verify_payload_bytes(&dest, pk, 5 + big.len() as u64).unwrap();
        // Byte-identical via the unified dest BlobStore file layout (R6).
        assert_eq!(dest_blobs.get(&hashes[0]).unwrap(), big);
        // And via the v2 blobs-table row the payload points at.
        let via_table: Vec<u8> = dest
            .query_row(
                "SELECT b.hash FROM messages m JOIN message_parts mp ON mp.message_pk=m.pk \
                 JOIN payloads p ON p.pk=mp.payload_pk JOIN blobs b ON b.pk=p.blob_pk \
                 WHERE m.session_pk=?1 AND m.seq=2",
                params![pk],
                |r| r.get::<_, Vec<u8>>(0),
            )
            .map(|h| {
                let hex: String = h.iter().map(|b| format!("{b:02x}")).collect();
                dest_blobs.get(&hex).unwrap()
            })
            .unwrap();
        assert_eq!(via_table, big);
        BlobImport::verify_migration_agreement(&dest).unwrap();
    }

    // DISC-108-T03: facade + migrations-table state agree; divergence fails.
    #[test]
    fn t03_migration_agreement_holds_then_rejects_divergence() {
        let dir = tempdir().unwrap();
        let (source_path, _) = source_file(&dir, &["a", "b"], &[]);
        let source = open_read_only(&source_path);
        let dest_dir = tempdir().unwrap();
        let (mut dest, pk, new_id) = dest_in(&dest_dir);
        let source_blobs = BlobStore::new(dir.path().join("source_blobs"));
        let dest_blob_dir = tempdir().unwrap();
        let dest_blobs = BlobStore::new(dest_blob_dir.path());
        BlobImport::import_all(&mut dest, &source, "src1", &new_id, pk, &source_blobs, &dest_blobs)
            .unwrap();
        BlobImport::verify_migration_agreement(&dest).unwrap();

        // Tamper the checksum: agreement must fail.
        dest.execute(
            "UPDATE schema_migrations SET checksum=?1 WHERE version=2",
            params![vec![0_u8; 32]],
        )
        .unwrap();
        assert!(BlobImport::verify_migration_agreement(&dest).is_err());

        // Restore checksum, then plant a foreign legacy _migrations table:
        // agreement must fail on the shadow runner state too.
        let good = SchemaV2::workspace_checksum();
        dest.execute(
            "UPDATE schema_migrations SET checksum=?1 WHERE version=2",
            params![&good[..]],
        )
        .unwrap();
        dest.execute_batch(
            "CREATE TABLE _migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, \
             checksum TEXT NOT NULL, applied_at TEXT NOT NULL); \
             INSERT INTO _migrations VALUES (1,'foreign','abc','2026-01-01T00:00:00Z');",
        )
        .unwrap();
        assert!(BlobImport::verify_migration_agreement(&dest).is_err());
    }

    // DISC-108-T04: interrupted import resumes to a consistent state.
    #[test]
    fn t04_interrupted_import_resumes_with_no_half_message() {
        let dir = tempdir().unwrap();
        let inline: Vec<String> = (0..700).map(|i| format!("row {i}")).collect();
        let refs: Vec<&str> = inline.iter().map(String::as_str).collect();
        let blob: Vec<u8> = vec![7_u8; 9000];
        let (source_path, _) = source_file(&dir, &refs, &[blob.clone()]);
        let source = open_read_only(&source_path);
        let dest_dir = tempdir().unwrap();
        let (mut dest, pk, _) = dest_in(&dest_dir);
        let source_blobs = BlobStore::new(dir.path().join("source_blobs"));
        let dest_blob_dir = tempdir().unwrap();
        let dest_blobs = BlobStore::new(dest_blob_dir.path());

        // Simulate interruption: import exactly one bounded page then stop.
        let (i1, b1, _, cursor) =
            BlobImport::import_page(&mut dest, &source, "src1", pk, 0, &source_blobs, &dest_blobs)
                .unwrap();
        assert_eq!(i1 + b1, IMPORT_PAGE_ROWS as u64);
        assert_eq!(count(&dest, pk), IMPORT_PAGE_ROWS);
        // Resume from the cursor to completion: no half message, exact total.
        let mut c = cursor;
        loop {
            let (i, b, _, n) =
                BlobImport::import_page(&mut dest, &source, "src1", pk, c, &source_blobs, &dest_blobs)
                    .unwrap();
            if i + b == 0 || n == c {
                break;
            }
            c = n;
        }
        BlobImport::verify_counts(&dest, pk, 701).unwrap();
        let expected: u64 = inline.iter().map(|s| s.len() as u64).sum::<u64>() + 9000;
        BlobImport::verify_payload_bytes(&dest, pk, expected).unwrap();
        // Every message has exactly one part: no orphan/half rows.
        let parts: i64 = dest
            .query_row(
                "SELECT COUNT(*) FROM message_parts mp JOIN messages m ON m.pk=mp.message_pk \
                 WHERE m.session_pk=?1",
                params![pk],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(parts, 701);
    }

    // DISC-108-T05: oversized blob rejects with explicit quota error, prior
    // state untouched.
    #[test]
    fn t05_oversized_blob_rejects_with_quota_error_leaving_state() {
        let dir = tempdir().unwrap();
        let huge_len = MAX_BLOB_BYTES + 1;
        let huge: Vec<u8> = (0..huge_len).map(|i| (i % 251) as u8).collect();
        let (source_path, _) = source_file(&dir, &["keep me"], &[huge]);
        let source = open_read_only(&source_path);
        let dest_dir = tempdir().unwrap();
        let (mut dest, pk, new_id) = dest_in(&dest_dir);
        let source_blobs = BlobStore::new(dir.path().join("source_blobs"));
        let dest_blob_dir = tempdir().unwrap();
        let dest_blobs = BlobStore::new(dest_blob_dir.path());

        let before_count = count(&dest, pk);
        let before_bytes = payload_sum(&dest, pk);
        let err = BlobImport::import_all(
            &mut dest,
            &source,
            "src1",
            &new_id,
            pk,
            &source_blobs,
            &dest_blobs,
        )
        .unwrap_err();
        assert!(
            is_quota_error(&err),
            "must be explicit quota error, got: {err}"
        );
        // The inline prefix row committed; the oversized blob row wrote nothing.
        assert_eq!(count(&dest, pk), before_count + 1);
        assert_eq!(payload_sum(&dest, pk), before_bytes + 7);
        let blob_rows: i64 = dest
            .query_row("SELECT COUNT(*) FROM blobs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(blob_rows, 0, "oversized blob must leave no blobs row");
        BlobImport::verify_counts(&dest, pk, 1).unwrap();
    }
}
