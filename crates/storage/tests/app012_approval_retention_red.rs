//! APP012-APPROVAL-RETENTION-RED: resolved approval state 3 (expired)
//! participates in the bounded retention backlog/sweep per the existing
//! resolved-age policy; pending state 0 and too-new rows stay untouched.
//!
//! Gap: `RetentionV2::sweep_resolved_approvals` / `retention_backlog`
//! (`retention_v2.rs:97,142`) match `state IN (1, 2, 4)` only, while
//! `ApprovalsV2::expire_sweep` (`approvals_v2.rs:137`) produces `state = 3`.
//! Expired approvals therefore grow unbounded outside retention.
#![forbid(unsafe_code)]

use opencode_rk_storage::{ApprovalsV2, RetentionV2, SchemaV2};
use rusqlite::{params, Connection};

fn workspace() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let conn =
        SchemaV2::initialize_workspace(&dir.path().join("workspace.db"), [1_u8; 16], [2_u8; 16], 0)
            .unwrap();
    (dir, conn)
}

fn session_pk(conn: &Connection) -> i64 {
    conn.execute(
        "INSERT INTO sessions (id, title, created_at_us, updated_at_us)
         VALUES (randomblob(16), 't', 0, 1)",
        [],
    )
    .unwrap();
    conn.last_insert_rowid()
}

/// Real pending → expired (state 3) transition, then stamp the resolved age
/// the existing retention policy keys on (bounded: 2 params).
fn expired_with_resolved_age(conn: &Connection, session: i64, resolved_us: i64) -> i64 {
    let pk = ApprovalsV2::request(
        conn,
        session,
        None,
        &[7_u8; 32],
        1,
        "delete",
        false,
        None,
        5,
        100,
    )
    .unwrap();
    assert_eq!(ApprovalsV2::expire_sweep(conn, 100, 500).unwrap(), 1);
    conn.execute(
        "UPDATE approvals SET resolved_at_us = ?1 WHERE pk = ?2",
        params![resolved_us, pk],
    )
    .unwrap();
    pk
}

fn state_of(conn: &Connection, pk: i64) -> i64 {
    conn.query_row("SELECT state FROM approvals WHERE pk = ?1", [pk], |r| {
        r.get(0)
    })
    .unwrap()
}

#[test]
fn expired_state3_old_resolved_appears_in_backlog() {
    let (_dir, conn) = workspace();
    let session = session_pk(&conn);
    let pk = expired_with_resolved_age(&conn, session, 5);
    assert_eq!(state_of(&conn, pk), 3);
    assert_eq!(
        RetentionV2::retention_backlog(&conn, 50).unwrap(),
        (0, 1, 0),
        "old resolved expired approval must join the retention backlog"
    );
}

#[test]
fn expired_state3_old_resolved_swept_with_cascade() {
    let (_dir, conn) = workspace();
    let session = session_pk(&conn);
    let expired = expired_with_resolved_age(&conn, session, 5);
    ApprovalsV2::add_resource(&conn, expired, 0, "file:///tmp/x").unwrap();
    let pending = ApprovalsV2::request(
        &conn,
        session,
        None,
        &[7_u8; 32],
        1,
        "delete",
        false,
        None,
        0,
        10_000,
    )
    .unwrap();
    assert_eq!(
        RetentionV2::sweep_resolved_approvals(&conn, 50, 500).unwrap(),
        1,
        "old resolved expired approval must be swept like other terminal states"
    );
    assert_eq!(state_of(&conn, pending), 0);
    let resources: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM approval_resources WHERE approval_pk = ?1",
            [expired],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(resources, 0);
}

#[test]
fn pending_and_too_new_expired_rows_untouched() {
    let (_dir, conn) = workspace();
    let session = session_pk(&conn);
    let pending = ApprovalsV2::request(
        &conn,
        session,
        None,
        &[7_u8; 32],
        1,
        "delete",
        false,
        None,
        0,
        10_000,
    )
    .unwrap();
    let fresh = expired_with_resolved_age(&conn, session, 1000);
    assert_eq!(
        RetentionV2::sweep_resolved_approvals(&conn, 50, 500).unwrap(),
        0,
        "pending and too-new rows must survive the sweep"
    );
    assert_eq!(state_of(&conn, pending), 0);
    assert_eq!(state_of(&conn, fresh), 3);
    assert_eq!(
        RetentionV2::retention_backlog(&conn, 50).unwrap(),
        (0, 0, 0)
    );
}
