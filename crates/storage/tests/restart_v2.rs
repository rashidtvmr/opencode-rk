//! Process-restart durability contracts for the format-2 workspace schema.
//!
//! ENGINE BOUNDARY: bundled SQLite is 3.50.2, below the 3.50.7 WAL-reset gate
//! (docs/storage/ENGINE_GATE.md). These tests prove committed data survives a
//! connection close plus reopen (process-equivalent) on the SAME host. They do
//! NOT claim power-loss or filesystem-crash durability, and the bundled engine
//! is not production-qualified. journal_mode=WAL persistence across reopen is
//! the observably tested claim here.
use opencode_rk_contracts::{MessageId, SessionId};
use opencode_rk_storage::{
    schema_v2::SchemaV2,
    writer_v2::{NewMessage, NewSession, V2Writer},
};
use rusqlite::{params, Connection, TransactionBehavior};
use tempfile::tempdir;

const WORKSPACE_SQL: &str = include_str!("../schema/v2/workspace.sql");

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
        role: opencode_rk_contracts::MessageRole::User,
        body: opencode_rk_contracts::PayloadRef::Inline {
            text: text.to_owned(),
        },
        created_at_us: 10,
    }
}

fn init(path: &std::path::Path) -> Connection {
    SchemaV2::initialize_workspace(path, [1_u8; 16], [2_u8; 16], 10).unwrap()
}

fn pragma_journal(connection: &Connection) -> String {
    connection
        .query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))
        .unwrap()
        .to_ascii_lowercase()
}

fn pragma_i64(connection: &Connection, name: &str) -> i64 {
    connection
        .query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))
        .unwrap()
}

fn workspace_row(connection: &Connection) -> (Vec<u8>, Vec<u8>, i64, i64, i64) {
    connection
        .query_row(
            "SELECT workspace_id,cursor_epoch,format_version,event_head_seq,created_at_us \
             FROM workspace_state WHERE id=1",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .unwrap()
}

#[test]
fn reopen_initialized_file_stays_wal() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("workspace.db");
    let conn = init(&path);
    let before = workspace_row(&conn);
    drop(conn);

    let reopened = SchemaV2::open_existing(&path).unwrap();
    assert_eq!(pragma_journal(&reopened), "wal");
    assert_eq!(pragma_i64(&reopened, "user_version"), 2);
    assert_eq!(workspace_row(&reopened), before);
    assert!(WORKSPACE_SQL.contains("CREATE TABLE sessions"));
}

#[test]
fn committed_session_survives_reopen() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("workspace.db");
    let mut conn = init(&path);
    let session_id = SessionId::new();
    V2Writer::create_session(&mut conn, &session(session_id, "durable", 50)).unwrap();
    drop(conn);

    let reopened = SchemaV2::open_existing(&path).unwrap();
    let sessions = V2Writer::list_recent_sessions(&reopened, 0, None, 100).unwrap();
    assert_eq!(sessions.len(), 1);
    let count: i64 = reopened
        .query_row("SELECT count(*) FROM sessions", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn committed_messages_and_outbox_survive_reopen() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("workspace.db");
    let mut conn = init(&path);
    let session_id = SessionId::new();
    V2Writer::create_session(&mut conn, &session(session_id, "conversation", 10)).unwrap();
    V2Writer::append_message(&mut conn, &message(session_id, MessageId::new(), "one")).unwrap();
    V2Writer::append_message(&mut conn, &message(session_id, MessageId::new(), "two")).unwrap();
    V2Writer::append_outbox_event(&mut conn, session_id, "created", "{}").unwrap();
    V2Writer::append_outbox_event(
        &mut conn,
        session_id,
        "changed",
        "{\"ok\":true}",
    )
    .unwrap();
    drop(conn);

    let reopened = SchemaV2::open_existing(&path).unwrap();
    let sequences: Vec<i64> = reopened
        .prepare("SELECT seq FROM messages ORDER BY seq")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(sequences, vec![1, 2]);
    assert_eq!(
        reopened
            .query_row(
                "SELECT next_message_seq FROM sessions WHERE id=?1",
                params![session_id.as_uuid().as_bytes().as_slice()],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        3
    );
    let outbox_seqs: Vec<i64> = reopened
        .prepare("SELECT seq FROM event_outbox ORDER BY seq")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(outbox_seqs, vec![1, 2]);
    assert_eq!(
        reopened
            .query_row("SELECT event_head_seq FROM workspace_state WHERE id=1", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        2
    );
}

#[test]
fn uncommitted_tx_does_not_survive() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("workspace.db");
    let mut conn = init(&path);
    let session_id = SessionId::new();
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    tx.execute(
        "INSERT INTO sessions (id, title, created_at_us, updated_at_us)
         VALUES (?1, ?2, 10, 10)",
        params![session_id.as_uuid().as_bytes().as_slice(), &"transient".to_string()],
    )
    .unwrap();
    // session visible to the in-flight connection, not yet durable
    let in_flight: i64 = tx
        .query_row("SELECT count(*) FROM sessions", [], |row| row.get(0))
        .unwrap();
    assert_eq!(in_flight, 1);
    tx.rollback().unwrap();
    drop(conn);

    let reopened = SchemaV2::open_existing(&path).unwrap();
    let after: i64 = reopened
        .query_row("SELECT count(*) FROM sessions", [], |row| row.get(0))
        .unwrap();
    assert_eq!(after, 0);
}

#[test]
fn checkpoint_does_not_lose_rows() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("workspace.db");
    let mut conn = init(&path);
    let session_id = SessionId::new();
    V2Writer::create_session(&mut conn, &session(session_id, "checkpointed", 10)).unwrap();
    V2Writer::append_message(&mut conn, &message(session_id, MessageId::new(), "one")).unwrap();
    V2Writer::append_message(&mut conn, &message(session_id, MessageId::new(), "two")).unwrap();
    V2Writer::append_outbox_event(&mut conn, session_id, "created", "{}").unwrap();
    V2Writer::append_outbox_event(
        &mut conn,
        session_id,
        "changed",
        "{\"ok\":true}",
    )
    .unwrap();

    let checkpoint: (i64, i64) = conn
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!(checkpoint.0, 0, "checkpoint must report busy==0");
    let head_before: i64 = conn
        .query_row("SELECT event_head_seq FROM workspace_state WHERE id=1", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap();
    assert_eq!(head_before, 2);
    drop(conn);

    let reopened = SchemaV2::open_existing(&path).unwrap();
    let session_count: i64 = reopened
        .query_row("SELECT count(*) FROM sessions", [], |row| row.get(0))
        .unwrap();
    assert_eq!(session_count, 1);
    let seqs: Vec<i64> = reopened
        .prepare("SELECT seq FROM messages ORDER BY seq")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(seqs, vec![1, 2]);
    assert_eq!(
        reopened
            .query_row("SELECT event_head_seq FROM workspace_state WHERE id=1", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        2
    );
    assert_eq!(
        reopened
            .query_row(
                "SELECT next_message_seq FROM sessions WHERE id=?1",
                params![session_id.as_uuid().as_bytes().as_slice()],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        3
    );
}
