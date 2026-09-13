//! RED contracts for the format-2 initializer and bounded writer.
use std::fs;

use opencode_rk_contracts::{MessageId, MessageRole, PayloadRef, SessionId};
use opencode_rk_storage::{
    schema_v2::SchemaV2,
    writer_v2::{NewMessage, NewSession, V2Writer},
};
use rusqlite::{params, Connection};
use tempfile::{tempdir, TempDir};

const WORKSPACE_SQL: &str = include_str!("../schema/v2/workspace.sql");
const WORKSPACE_APPLICATION_ID: i64 = 0x4F525732;

fn pragma_i64(connection: &Connection, name: &str) -> i64 {
    connection
        .query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))
        .unwrap()
}

fn initialized() -> (TempDir, Connection) {
    let directory = tempdir().unwrap();
    let path = directory.path().join("workspace.db");
    let connection = SchemaV2::initialize_workspace(&path, [1_u8; 16], [2_u8; 16], 10).unwrap();
    (directory, connection)
}

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

#[test]
fn initialize_new_workspace_applies_the_format_2_connection_contract() {
    let (_directory, connection) = initialized();

    assert_eq!(pragma_i64(&connection, "page_size"), 4096);
    assert_eq!(pragma_i64(&connection, "auto_vacuum"), 2);
    assert_eq!(
        pragma_i64(&connection, "application_id"),
        WORKSPACE_APPLICATION_ID
    );
    assert_eq!(pragma_i64(&connection, "user_version"), 2);
    assert_eq!(
        connection
            .query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))
            .unwrap()
            .to_ascii_lowercase(),
        "wal"
    );
    assert_eq!(pragma_i64(&connection, "foreign_keys"), 1);
    assert_eq!(pragma_i64(&connection, "synchronous"), 2);
    assert_eq!(pragma_i64(&connection, "trusted_schema"), 0);
    assert_eq!(pragma_i64(&connection, "temp_store"), 1);
    assert_eq!(pragma_i64(&connection, "busy_timeout"), 5000);

    let (version, checksum, applied_at_us): (i64, Vec<u8>, i64) = connection
        .query_row(
            "SELECT version,checksum,applied_at_us FROM schema_migrations",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(version, 2);
    assert_eq!(checksum.len(), 32);
    assert_eq!(
        checksum.as_slice(),
        SchemaV2::workspace_checksum().as_slice()
    );
    assert_eq!(applied_at_us, 10);

    let workspace: (Vec<u8>, Vec<u8>, i64, i64) = connection
        .query_row(
            "SELECT workspace_id,cursor_epoch,format_version,created_at_us \
             FROM workspace_state WHERE id=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(workspace, (vec![1; 16], vec![2; 16], 2, 10));

    assert!(WORKSPACE_SQL.contains("CREATE TABLE schema_migrations"));
    assert!(WORKSPACE_SQL.contains("CREATE TABLE event_outbox"));
}

#[test]
fn initialize_rejects_a_nonempty_file_without_modifying_it() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("not-a-workspace.db");
    let original = b"pre-existing bytes";
    fs::write(&path, original).unwrap();

    assert!(
        SchemaV2::initialize_workspace(&path, [1_u8; 16], [2_u8; 16], 10).is_err(),
        "non-empty files must fail closed"
    );
    assert_eq!(fs::read(path).unwrap(), original);
}

#[test]
fn initialize_is_not_idempotent_and_open_existing_is_the_reopen_path() {
    let (directory, connection) = initialized();
    let path = directory.path().join("workspace.db");
    drop(connection);

    assert!(SchemaV2::initialize_workspace(&path, [1_u8; 16], [2_u8; 16], 10).is_err());
    let reopened = SchemaV2::open_existing(&path).unwrap();
    assert_eq!(pragma_i64(&reopened, "user_version"), 2);
}

#[test]
fn open_existing_rejects_a_tampered_migration_checksum() {
    let (directory, mut connection) = initialized();
    let path = directory.path().join("workspace.db");
    connection
        .execute(
            "UPDATE schema_migrations SET checksum=?1 WHERE version=2",
            params![vec![0_u8; 32]],
        )
        .unwrap();
    drop(connection);

    assert!(
        SchemaV2::open_existing(&path).is_err(),
        "checksum verification is a readiness gate"
    );
}

#[test]
fn create_session_rejects_titles_over_1024_bytes() {
    let (_directory, mut connection) = initialized();
    let invalid = session(
        SessionId::new(),
        String::from_utf8(vec![b'x'; 1025]).unwrap(),
        10,
    );

    assert!(V2Writer::create_session(&mut connection, &invalid).is_err());
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM sessions", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn append_message_allocates_monotonic_session_sequences_atomically() {
    let (_directory, mut connection) = initialized();
    let session_id = SessionId::new();
    V2Writer::create_session(&mut connection, &session(session_id, "messages", 10)).unwrap();

    V2Writer::append_message(
        &mut connection,
        &message(session_id, MessageId::new(), "one"),
    )
    .unwrap();
    V2Writer::append_message(
        &mut connection,
        &message(session_id, MessageId::new(), "two"),
    )
    .unwrap();

    let sequences: Vec<i64> = connection
        .prepare("SELECT seq FROM messages ORDER BY seq")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(sequences, vec![1, 2]);
    assert_eq!(
        connection
            .query_row(
                "SELECT next_message_seq FROM sessions WHERE id=?1",
                params![session_id.as_uuid().as_bytes().as_slice()],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        3
    );
}

#[test]
fn append_outbox_event_allocates_a_monotonic_workspace_sequence() {
    let (_directory, mut connection) = initialized();
    let session_id = SessionId::new();

    V2Writer::append_outbox_event(&mut connection, session_id, "created", "{}").unwrap();
    V2Writer::append_outbox_event(&mut connection, session_id, "changed", "{\"ok\":true}").unwrap();

    let sequences: Vec<i64> = connection
        .prepare("SELECT seq FROM event_outbox ORDER BY seq")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(sequences, vec![1, 2]);
    assert_eq!(
        connection
            .query_row(
                "SELECT event_head_seq FROM workspace_state WHERE id=1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        2
    );
}

#[test]
fn rejected_outbox_event_rolls_back_head_and_rows_together() {
    let (_directory, mut connection) = initialized();

    assert!(
        V2Writer::append_outbox_event(&mut connection, SessionId::new(), "bad", "not-json")
            .is_err()
    );

    let counts: (i64, i64, i64) = connection
        .query_row(
            "SELECT (SELECT count(*) FROM sessions), \
                    (SELECT count(*) FROM event_outbox), \
                    event_head_seq FROM workspace_state WHERE id=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(counts, (0, 0, 0));
}

#[test]
fn outbox_payloads_over_4096_bytes_are_rejected_without_advancing_head() {
    let (_directory, mut connection) = initialized();
    let payload = format!("\"{}\"", "x".repeat(4096));
    assert!(payload.len() > 4096);

    assert!(
        V2Writer::append_outbox_event(&mut connection, SessionId::new(), "large", &payload)
            .is_err()
    );
    assert_eq!(pragma_i64(&connection, "user_version"), 2);
    assert_eq!(
        connection
            .query_row("SELECT event_head_seq FROM workspace_state", [], |row| row
                .get::<_, i64>(
                0
            ))
            .unwrap(),
        0
    );
}

#[test]
fn recent_sessions_are_state_filtered_keyset_paginated_and_limit_clamped() {
    let (_directory, mut connection) = initialized();
    let active_ids = [SessionId::new(), SessionId::new(), SessionId::new()];
    for (id, updated_at_us) in active_ids.into_iter().zip([30, 20, 10]) {
        V2Writer::create_session(&mut connection, &session(id, "active", updated_at_us)).unwrap();
    }
    let archived_id = SessionId::new();
    V2Writer::create_session(&mut connection, &session(archived_id, "archived", 40)).unwrap();
    connection
        .execute(
            "UPDATE sessions SET state=1,archived_at_us=updated_at_us WHERE id=?1",
            params![archived_id.as_uuid().as_bytes().as_slice()],
        )
        .unwrap();

    let active_cursor: Vec<(i64, i64)> = connection
        .prepare("SELECT updated_at_us,pk FROM sessions WHERE state=0 ORDER BY updated_at_us DESC,pk DESC")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(active_cursor.len(), 3);

    assert_eq!(
        V2Writer::list_recent_sessions(&connection, 0, None, 0)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        V2Writer::list_recent_sessions(&connection, 0, None, usize::MAX)
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        V2Writer::list_recent_sessions(&connection, 1, None, 100)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        V2Writer::list_recent_sessions(&connection, 0, Some(active_cursor[1]), 100)
            .unwrap()
            .len(),
        1
    );
}
