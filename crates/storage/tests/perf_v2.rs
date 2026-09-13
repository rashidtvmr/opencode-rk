//! Storage v2 perf probe (DEBUG BUILD numbers, not production claims).
//!
//! Measured 2026-09-13 on debug build. No new deps, std::time only.
//! Each test prints elapsed + per-op micros via eprintln (use --nocapture)
//! and asserts a generous <10s batch bound so CI stays green on slow machines.

use std::time::Instant;

use opencode_rk_contracts::{MessageId, MessageRole, PayloadRef, SessionId};
use opencode_rk_storage::{
    schema_v2::SchemaV2,
    writer_v2::{NewMessage, NewSession, V2Writer},
};
use tempfile::tempdir;

const BATCH: usize = 100;
const BOUND_SECS: u64 = 10;

fn init_db(dir: &tempfile::TempDir) -> rusqlite::Connection {
    let path = dir.path().join("workspace.db");
    SchemaV2::initialize_workspace(&path, [1_u8; 16], [2_u8; 16], 10).unwrap()
}

fn pragma_str(conn: &rusqlite::Connection, name: &str) -> String {
    conn.query_row(&format!("PRAGMA {name}"), [], |r| r.get::<_, String>(0))
        .unwrap()
}

fn pragma_i64(conn: &rusqlite::Connection, name: &str) -> i64 {
    conn.query_row(&format!("PRAGMA {name}"), [], |r| r.get(0))
        .unwrap()
}

fn report(label: &str, elapsed: std::time::Duration, ops: usize) {
    let per_op_us = elapsed.as_micros() as f64 / ops as f64;
    eprintln!("[perf_v2] {label}: total={elapsed:?} ops={ops} per_op={per_op_us:.1}us");
}

#[test]
fn perf_init_fresh_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("workspace.db");
    let start = Instant::now();
    let conn = SchemaV2::initialize_workspace(&path, [1_u8; 16], [2_u8; 16], 10).unwrap();
    let elapsed = start.elapsed();
    report("init_fresh_file", elapsed, 1);
    assert!(elapsed.as_secs() < BOUND_SECS, "init took {elapsed:?}");
    drop(conn);
}

#[test]
fn perf_100_sequential_create_session() {
    let dir = tempdir().unwrap();
    let mut conn = init_db(&dir);
    let start = Instant::now();
    for i in 0..BATCH {
        V2Writer::create_session(
            &mut conn,
            &NewSession {
                id: SessionId::new(),
                title: format!("s{i}"),
                created_at_us: 10 + i as i64,
                updated_at_us: 10 + i as i64,
            },
        )
        .unwrap();
    }
    let elapsed = start.elapsed();
    report("create_session_x100", elapsed, BATCH);
    assert!(elapsed.as_secs() < BOUND_SECS, "batch took {elapsed:?}");
}

#[test]
fn perf_100_append_message_inline_100b() {
    let dir = tempdir().unwrap();
    let mut conn = init_db(&dir);
    let session_id = SessionId::new();
    V2Writer::create_session(
        &mut conn,
        &NewSession {
            id: session_id,
            title: "perf".into(),
            created_at_us: 10,
            updated_at_us: 10,
        },
    )
    .unwrap();
    let body = "x".repeat(100);
    let start = Instant::now();
    for i in 0..BATCH {
        V2Writer::append_message(
            &mut conn,
            &NewMessage {
                id: MessageId::new(),
                session_id,
                role: MessageRole::User,
                body: PayloadRef::Inline {
                    text: body.clone(),
                },
                created_at_us: 10 + i as i64,
            },
        )
        .unwrap();
    }
    let elapsed = start.elapsed();
    report("append_message_inline100B_x100", elapsed, BATCH);
    assert!(elapsed.as_secs() < BOUND_SECS, "batch took {elapsed:?}");

    // DB size + pragmas after 100 msgs.
    let path = dir.path().join("workspace.db");
    let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    eprintln!(
        "[perf_v2] db_bytes_after_100msg={} page_count={} freelist_count={} journal_mode={}",
        bytes,
        pragma_i64(&conn, "page_count"),
        pragma_i64(&conn, "freelist_count"),
        pragma_str(&conn, "journal_mode"),
    );
}

#[test]
fn perf_100_append_outbox_event() {
    let dir = tempdir().unwrap();
    let mut conn = init_db(&dir);
    let session_id = SessionId::new();
    let start = Instant::now();
    for _ in 0..BATCH {
        V2Writer::append_outbox_event(&mut conn, session_id, "perf", "{}").unwrap();
    }
    let elapsed = start.elapsed();
    report("append_outbox_event_x100", elapsed, BATCH);
    assert!(elapsed.as_secs() < BOUND_SECS, "batch took {elapsed:?}");
}

#[test]
fn perf_keyset_list_recent_over_100_sessions() {
    let dir = tempdir().unwrap();
    let mut conn = init_db(&dir);
    for i in 0..BATCH {
        V2Writer::create_session(
            &mut conn,
            &NewSession {
                id: SessionId::new(),
                title: format!("s{i}"),
                created_at_us: 10 + i as i64,
                updated_at_us: 10 + i as i64,
            },
        )
        .unwrap();
    }
    let start = Instant::now();
    let rows = V2Writer::list_recent_sessions(&conn, 0, None, 100).unwrap();
    let elapsed = start.elapsed();
    assert_eq!(rows.len(), 100);
    report("list_recent_100rows_over_100sessions", elapsed, 1);
    assert!(elapsed.as_secs() < BOUND_SECS, "list took {elapsed:?}");
}
