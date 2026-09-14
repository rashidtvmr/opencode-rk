//! Integration contracts for ImportV2: format-1 file source into a format-2 workspace.
//!
//! The format-1 source is a real file database built from the exact DDL used by
//! `Storage::migrate` (crates/storage/src/lib.rs), opened read-only for the
//! import. The source bytes must be identical before and after the import.
use std::fs;
use std::path::Path;

use opencode_rk_contracts::SessionId;
use opencode_rk_storage::{ImportV2, NewSession, SchemaV2, V2Writer};
use rusqlite::{params, Connection, OpenFlags};
use tempfile::{tempdir, TempDir};

/// Format-1 DDL, CREATE statements copied verbatim from `Storage::migrate`
/// (crates/storage/src/lib.rs, migrate()).
const FORMAT1_DDL: &str = "
    CREATE TABLE IF NOT EXISTS schema_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY,title TEXT NOT NULL,state TEXT NOT NULL CHECK(state IN ('active','archived')),created_at TEXT NOT NULL,updated_at TEXT NOT NULL,archived_at TEXT);
    CREATE INDEX IF NOT EXISTS sessions_updated_idx ON sessions(updated_at DESC);
    CREATE TABLE IF NOT EXISTS messages(id TEXT PRIMARY KEY,session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,role TEXT NOT NULL,inline_text TEXT,blob_hash TEXT,byte_len INTEGER NOT NULL CHECK(byte_len>=0),created_at TEXT NOT NULL,CHECK((inline_text IS NULL)!=(blob_hash IS NULL)));
    CREATE INDEX IF NOT EXISTS messages_session_idx ON messages(session_id,created_at,id);
    CREATE TABLE IF NOT EXISTS recent_events(seq INTEGER PRIMARY KEY AUTOINCREMENT,session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,kind TEXT NOT NULL,payload_json TEXT NOT NULL,created_at TEXT NOT NULL);
    CREATE INDEX IF NOT EXISTS recent_events_session_idx ON recent_events(session_id,seq);
";

const CREATED_AT: &str = "2026-01-01T00:00:00Z";
/// Format-1 role strings cycled across inserted messages, exercising every
/// role mapping the importer encodes into v2 integers.
const ROLES: [&str; 4] = ["system", "user", "assistant", "tool"];

/// Build a format-1 source database file: `sessions` rows plus `messages`
/// rows given as (session_id, inline_text) pairs.
fn build_format1_source(path: &Path, sessions: &[&str], messages: &[(String, String)]) {
    let connection = Connection::open(path).unwrap();
    connection.execute_batch(FORMAT1_DDL).unwrap();
    for (index, id) in sessions.iter().enumerate() {
        connection
            .execute(
                "INSERT INTO sessions(id,title,state,created_at,updated_at,archived_at) \
                 VALUES (?1,?2,'active',?3,?3,NULL)",
                params![id, format!("session {index}"), CREATED_AT],
            )
            .unwrap();
    }
    for (index, (session, text)) in messages.iter().enumerate() {
        connection
            .execute(
                "INSERT INTO messages(id,session_id,role,inline_text,blob_hash,byte_len,created_at) \
                 VALUES (?1,?2,?3,?4,NULL,?5,?6)",
                params![
                    format!("m{index}"),
                    session,
                    ROLES[index % ROLES.len()],
                    text,
                    text.len() as i64,
                    CREATED_AT
                ],
            )
            .unwrap();
    }
}

/// Open an existing database strictly read-only; the importer must never need
/// more than read access to the source.
fn open_read_only(path: &Path) -> Connection {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
}

/// Fresh format-2 workspace via SchemaV2::initialize_workspace on a tempdir.
fn initialized_dest() -> (TempDir, Connection) {
    let directory = tempdir().unwrap();
    let path = directory.path().join("workspace.db");
    let connection = SchemaV2::initialize_workspace(&path, [1_u8; 16], [2_u8; 16], 10).unwrap();
    (directory, connection)
}

/// Create one v2 session; returns (pk, session id bytes) for the import call.
fn dest_session(connection: &mut Connection) -> (i64, [u8; 16]) {
    let id = SessionId::new();
    let id_bytes = *id.as_uuid().as_bytes();
    V2Writer::create_session(
        connection,
        &NewSession {
            id,
            title: "imported".to_owned(),
            created_at_us: 10,
            updated_at_us: 10,
        },
    )
    .unwrap();
    let pk: i64 = connection
        .query_row(
            "SELECT pk FROM sessions WHERE id=?1",
            params![&id_bytes[..]],
            |row| row.get(0),
        )
        .unwrap();
    (pk, id_bytes)
}

fn source_three_by_three(path: &Path) {
    build_format1_source(
        path,
        &["src1", "src2", "src3"],
        &[
            ("src1".to_owned(), "alpha".to_owned()),
            ("src1".to_owned(), "bravo".to_owned()),
            ("src1".to_owned(), "charlie".to_owned()),
        ],
    );
}

#[test]
fn import_session_copies_all_messages_preserving_role_and_order() {
    let directory = tempdir().unwrap();
    let source_path = directory.path().join("source.db");
    source_three_by_three(&source_path);
    let source = open_read_only(&source_path);

    let (_dest_dir, mut dest) = initialized_dest();
    let (pk, new_id) = dest_session(&mut dest);

    let imported = ImportV2::import_session(&mut dest, &source, "src1", &new_id, pk).unwrap();

    assert_eq!(imported, 3, "every source message must import");
    let count: i64 = dest
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE session_pk=?1",
            params![pk],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 3);
    ImportV2::verify_counts(&dest, pk, 3).unwrap();

    // Sequences allocate from 1, roles map system/user/assistant -> 0/1/2,
    // bodies survive inline in source rowid order.
    let rows: Vec<(i64, i64, Vec<u8>)> = dest
        .prepare(
            "SELECT m.seq,m.role,p.inline_data FROM messages m \
             JOIN message_parts mp ON mp.message_pk=m.pk \
             JOIN payloads p ON p.pk=mp.payload_pk \
             WHERE m.session_pk=?1 ORDER BY m.seq",
        )
        .unwrap()
        .query_map(params![pk], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0], (1, 0, b"alpha".to_vec()));
    assert_eq!(rows[1], (2, 1, b"bravo".to_vec()));
    assert_eq!(rows[2], (3, 2, b"charlie".to_vec()));

    let next_seq: i64 = dest
        .query_row(
            "SELECT next_message_seq FROM sessions WHERE pk=?1",
            params![pk],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(next_seq, 4, "session sequence must advance past imports");
}

#[test]
fn source_file_is_byte_identical_after_import() {
    let directory = tempdir().unwrap();
    let source_path = directory.path().join("source.db");
    source_three_by_three(&source_path);
    let before = fs::read(&source_path).unwrap();

    let source = open_read_only(&source_path);
    let (_dest_dir, mut dest) = initialized_dest();
    let (pk, new_id) = dest_session(&mut dest);

    ImportV2::import_session(&mut dest, &source, "src1", &new_id, pk).unwrap();
    ImportV2::verify_counts(&dest, pk, 3).unwrap();

    let during = fs::read(&source_path).unwrap();
    drop(source);
    let after = fs::read(&source_path).unwrap();
    assert_eq!(before, during, "import must not touch the source mid-run");
    assert_eq!(
        before, after,
        "import must not touch the source after close"
    );
}

#[test]
fn import_is_resumable_drains_a_large_session_in_two_pages() {
    let directory = tempdir().unwrap();
    let source_path = directory.path().join("source.db");
    let messages: Vec<(String, String)> = (0..700)
        .map(|index| ("src1".to_owned(), format!("body {index}")))
        .collect();
    build_format1_source(&source_path, &["src1"], &messages);
    let source = open_read_only(&source_path);

    let (_dest_dir, mut dest) = initialized_dest();
    let (pk, _new_id) = dest_session(&mut dest);

    // Page 1 is bounded by the documented PAGE_BUDGET of 500.
    let (imported_first, cursor_first) =
        ImportV2::import_is_resumable(&mut dest, &source, pk, 0).unwrap();
    assert_eq!(imported_first, 500);
    assert!(cursor_first > 0, "page 1 must advance the cursor");

    // Page 2 resumes at the returned cursor and drains the remaining 200.
    let (imported_second, cursor_second) =
        ImportV2::import_is_resumable(&mut dest, &source, pk, cursor_first).unwrap();
    assert_eq!(imported_second, 200);
    assert!(cursor_second > cursor_first, "page 2 must advance further");

    // A third page is exhausted: nothing imported, cursor does not move.
    let (imported_third, cursor_third) =
        ImportV2::import_is_resumable(&mut dest, &source, pk, cursor_second).unwrap();
    assert_eq!(imported_third, 0);
    assert_eq!(cursor_third, cursor_second);

    assert_eq!(imported_first + imported_second, 700);
    ImportV2::verify_counts(&dest, pk, 700).unwrap();
}

#[test]
fn verify_counts_rejects_deliberate_mismatches() {
    let directory = tempdir().unwrap();
    let source_path = directory.path().join("source.db");
    source_three_by_three(&source_path);
    let source = open_read_only(&source_path);

    let (_dest_dir, mut dest) = initialized_dest();
    let (pk, new_id) = dest_session(&mut dest);
    ImportV2::import_session(&mut dest, &source, "src1", &new_id, pk).unwrap();

    assert!(
        ImportV2::verify_counts(&dest, pk, 2).is_err(),
        "undercount must fail"
    );
    assert!(
        ImportV2::verify_counts(&dest, pk, 4).is_err(),
        "overcount must fail"
    );
    ImportV2::verify_counts(&dest, pk, 3).unwrap();

    // An unknown destination session holds zero messages: nonzero
    // expectations fail, an exact zero expectation succeeds.
    assert!(ImportV2::verify_counts(&dest, pk + 100, 3).is_err());
    ImportV2::verify_counts(&dest, pk + 100, 0).unwrap();
}
