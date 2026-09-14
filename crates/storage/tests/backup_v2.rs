//! Backup/restore validation: snapshot-pinned backups capture a restorable
//! image while GC is excluded.
//!
//! Scope and validity boundary: these tests copy the bytes of a file-backed
//! SQLite DB (WAL) to a separate backup path. A file copy of a WAL database is
//! only crash-consistent if the WAL has been checkpointed and the main DB file
//! fully flushed first. Each test therefore runs `PRAGMA wal_checkpoint(TRUNCATE)`
//! before copying, making the copy independent of the original -wal/-shm.
//! Power-loss durability is NOT claimed here; that requires the SQLite backup
//! API under an exclusive retention gate (see docs/storage/CRASH_CONSISTENCY.md,
//! "Backups, restore and cross-store boundaries").
use std::fs;

use opencode_rk_contracts::{MessageId, MessageRole, PayloadRef, SessionId};
use opencode_rk_storage::{
    schema_v2::SchemaV2,
    writer_v2::{NewMessage, NewSession, V2Writer},
};
use rusqlite::{params, Connection};
use tempfile::{tempdir, TempDir};

fn session(id: SessionId, title: impl Into<String>, updated_at_us: i64) -> NewSession {
    NewSession {
        id,
        title: title.into(),
        created_at_us: updated_at_us,
        updated_at_us,
    }
}

fn message(session_id: SessionId, id: MessageId, text: &str) -> NewMessage {
    NewMessage {
        id,
        session_id,
        role: MessageRole::User,
        body: PayloadRef::Inline {
            text: text.to_owned(),
        },
        created_at_us: 10,
    }
}

/// Fresh file-backed workspace (NOT :memory:) plus its directory.
fn initialized() -> (TempDir, Connection) {
    let directory = tempdir().unwrap();
    let path = directory.path().join("workspace.db");
    let connection = SchemaV2::initialize_workspace(&path, [1_u8; 16], [2_u8; 16], 10).unwrap();
    (directory, connection)
}

/// Flush all WAL frames into the main DB file so a byte copy is self-contained.
// wal_checkpoint returns (busy, log_frames, checkpointed_frames); success is busy==0.
fn checkpoint(connection: &Connection) {
    let (busy, _log, _ckpt): (i64, i64, i64) = connection
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .unwrap();
    assert_eq!(busy, 0, "wal_checkpoint must succeed before copying");
}

/// Copy the main DB file to a fresh backup path in the same tempdir.
fn copy_backup(directory: &TempDir, connection: &Connection) -> std::path::PathBuf {
    checkpoint(connection);
    let source = directory.path().join("workspace.db");
    let backup = directory.path().join("backup.db");
    fs::copy(&source, &backup).unwrap();
    backup
}

fn foreign_key_violations(connection: &Connection) -> i64 {
    // foreign_key_check yields one row per violation; count via a wrapper query.
    let mut stmt = connection
        .prepare("SELECT count(*) FROM pragma_foreign_key_check")
        .unwrap();
    stmt.query_row([], |row| row.get(0)).unwrap()
}

#[test]
fn backup_captures_pinned_roots() {
    let (directory, mut connection) = initialized();
    let session_id = SessionId::new();
    V2Writer::create_session(&mut connection, &session(session_id, "pinned", 10)).unwrap();
    V2Writer::append_message(
        &mut connection,
        &message(session_id, MessageId::new(), "hello"),
    )
    .unwrap();
    V2Writer::append_message(
        &mut connection,
        &message(session_id, MessageId::new(), "world"),
    )
    .unwrap();

    // Pin the message payloads as backup roots: purpose=2 is the backup pin.
    let payload_pks: Vec<i64> = connection
        .prepare("SELECT payload_pk FROM message_parts ORDER BY ordinal")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(payload_pks.len(), 2);
    for payload_pk in &payload_pks {
        connection
            .execute(
                "INSERT INTO retained_payloads (owner_id, purpose, payload_pk, created_at_us)
                 VALUES (?1, 2, ?2, 10)",
                params![vec![3_u8; 16], payload_pk],
            )
            .unwrap();
    }

    let backup = copy_backup(&directory, &connection);

    // Open the backup independently and verify the pinned image is present.
    let backup_conn = SchemaV2::open_existing(&backup).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM sessions", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        backup_conn
            .query_row("SELECT count(*) FROM sessions", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        backup_conn
            .query_row("SELECT count(*) FROM messages", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
    // Both pinned roots survive the copy.
    let pinned: i64 = backup_conn
        .query_row(
            "SELECT count(*) FROM retained_payloads WHERE purpose=2",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(pinned, 2);
    // The message bodies are reachable from the backup image.
    let bodies: Vec<String> = backup_conn
        .prepare(
            "SELECT CAST(p.inline_data AS TEXT) FROM message_parts mp
             JOIN payloads p ON p.pk = mp.payload_pk ORDER BY mp.ordinal",
        )
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(bodies, vec!["hello".to_owned(), "world".to_owned()]);
}

#[test]
fn backup_excludes_gc_midflight() {
    let (directory, mut connection) = initialized();
    let session_id = SessionId::new();
    V2Writer::create_session(&mut connection, &session(session_id, "live", 10)).unwrap();
    V2Writer::append_message(
        &mut connection,
        &message(session_id, MessageId::new(), "keep me"),
    )
    .unwrap();

    // Simulate a GC that committed its DELETING tombstone but has NOT yet
    // unlinked the file / deleted the tombstone (mid-flight GC). This blob is
    // deliberately unreferenced by payloads, so deleting=1 is collectable.
    connection
        .execute(
            "INSERT INTO blobs (hash, state, codec, raw_bytes, stored_bytes, created_at_us)
             VALUES (?1, 1, 0, 10, 62, 10)",
            params![vec![9_u8; 32]],
        )
        .unwrap();

    let backup = copy_backup(&directory, &connection);
    let backup_conn = SchemaV2::open_existing(&backup).unwrap();

    // Live rows are intact in the backup.
    assert_eq!(
        backup_conn
            .query_row("SELECT count(*) FROM messages", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    // The mid-flight GC tombstone is preserved as an intent, not lost.
    assert_eq!(
        backup_conn
            .query_row(
                "SELECT state FROM blobs WHERE hash=?1",
                params![vec![9_u8; 32]],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        1
    );
}

#[test]
fn restore_reopens_clean() {
    let (directory, mut connection) = initialized();
    let session_id = SessionId::new();
    V2Writer::create_session(&mut connection, &session(session_id, "clean", 10)).unwrap();
    V2Writer::append_message(
        &mut connection,
        &message(session_id, MessageId::new(), "payload one"),
    )
    .unwrap();
    V2Writer::append_message(
        &mut connection,
        &message(session_id, MessageId::new(), "payload two"),
    )
    .unwrap();

    let backup = copy_backup(&directory, &connection);

    // The backup passes the schema readiness gate and has no FK violations.
    let backup_conn = SchemaV2::open_existing(&backup).unwrap();
    assert_eq!(foreign_key_violations(&backup_conn), 0);
}

#[test]
fn outbox_floor_preserved() {
    let (directory, mut connection) = initialized();
    V2Writer::append_outbox_event(&mut connection, SessionId::new(), "changed", "{}").unwrap();
    V2Writer::append_outbox_event(&mut connection, SessionId::new(), "created", "{\"a\":1}")
        .unwrap();
    // Advance the floor to the head by pruning the contiguous prefix.
    connection
        .execute(
            "UPDATE workspace_state SET event_floor_seq=event_head_seq WHERE id=1",
            [],
        )
        .unwrap();

    let (head, floor): (i64, i64) = connection
        .query_row(
            "SELECT event_head_seq, event_floor_seq FROM workspace_state WHERE id=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(head, 2);
    assert_eq!(head, floor);

    let backup = copy_backup(&directory, &connection);
    let backup_conn = SchemaV2::open_existing(&backup).unwrap();
    let (b_head, b_floor): (i64, i64) = backup_conn
        .query_row(
            "SELECT event_head_seq, event_floor_seq FROM workspace_state WHERE id=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    // Cursor head/floor are preserved exactly by the copy.
    assert_eq!((b_head, b_floor), (head, floor));
}

#[test]
fn pin_blocks_gc_claim() {
    let (directory, mut connection) = initialized();
    // A ready blob referenced by a payload that is pinned as a backup root.
    connection
        .execute(
            "INSERT INTO blobs (hash, state, codec, raw_bytes, stored_bytes, created_at_us)
             VALUES (?1, 0, 0, 10, 62, 10)",
            params![vec![7_u8; 32]],
        )
        .unwrap();
    let blob_pk = connection.last_insert_rowid();
    connection
        .execute(
            "INSERT INTO payloads (blob_pk, raw_bytes, created_at_us) VALUES (?1, 10, 10)",
            params![blob_pk],
        )
        .unwrap();
    let payload_pk = connection.last_insert_rowid();
    connection
        .execute(
            "INSERT INTO retained_payloads (owner_id, purpose, payload_pk, created_at_us)
             VALUES (?1, 2, ?2, 10)",
            params![vec![1_u8; 16], payload_pk],
        )
        .unwrap();

    // GC tries to claim the pinned blob by setting DELETING without finishing.
    // The blob_gc_claim trigger rejects the collection because a payload still
    // references this blob.
    let result = connection.execute("UPDATE blobs SET state=1 WHERE pk=?1", params![blob_pk]);
    assert!(
        result.is_err(),
        "a pinned/referenced blob must not be collectable by GC"
    );
    // The blob is still READY and its pin survives.
    assert_eq!(
        connection
            .query_row(
                "SELECT state FROM blobs WHERE pk=?1",
                params![blob_pk],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        0
    );
}
