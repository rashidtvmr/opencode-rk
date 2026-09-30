//! APP012: expired approvals are terminal rows in the existing bounded sweep.
#![forbid(unsafe_code)]

use opencode_rk_storage::{ApprovalsV2, RetentionV2, SchemaV2};
use rusqlite::Connection;

fn workspace() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let conn =
        SchemaV2::initialize_workspace(&dir.path().join("workspace.db"), [1; 16], [2; 16], 0)
            .unwrap();
    (dir, conn)
}

fn session_pk(conn: &Connection) -> i64 {
    conn.execute("INSERT INTO sessions (id, title, created_at_us, updated_at_us) VALUES (randomblob(16), 't', 0, 1)", []).unwrap();
    conn.last_insert_rowid()
}

fn expired(conn: &Connection, session: i64, expiry_us: i64) -> i64 {
    let pk = ApprovalsV2::request(
        conn, session, None, &[7; 32], 1, "delete", false, None, 5, expiry_us,
    )
    .unwrap();
    assert_eq!(ApprovalsV2::expire_sweep(conn, expiry_us, 500).unwrap(), 1);
    assert_eq!(
        conn.query_row(
            "SELECT state, resolved_at_us FROM approvals WHERE pk = ?1",
            [pk],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?))
        )
        .unwrap(),
        (3, Some(expiry_us))
    );
    pk
}

fn state(conn: &Connection, pk: i64) -> i64 {
    conn.query_row("SELECT state FROM approvals WHERE pk = ?1", [pk], |r| {
        r.get(0)
    })
    .unwrap()
}

#[test]
fn expired_transition_is_retained_until_existing_cutoff_then_enters_backlog() {
    let (_dir, conn) = workspace();
    let session = session_pk(&conn);
    let pk = expired(&conn, session, 100);
    assert_eq!(RetentionV2::retention_backlog(&conn, 99).unwrap().1, 0);
    assert_eq!(
        RetentionV2::retention_backlog(&conn, 100).unwrap(),
        (0, 1, 0)
    );
    assert_eq!(state(&conn, pk), 3);
}

#[test]
fn expired_terminal_rows_are_bounded_swept_with_resource_cascade() {
    let (_dir, conn) = workspace();
    let session = session_pk(&conn);
    let first = expired(&conn, session, 50);
    let second = expired(&conn, session, 50);
    ApprovalsV2::add_resource(&conn, first, 0, "file:///tmp/x").unwrap();
    ApprovalsV2::add_resource(&conn, second, 0, "file:///tmp/y").unwrap();
    let pending = ApprovalsV2::request(
        &conn, session, None, &[7; 32], 1, "delete", false, None, 0, 10_000,
    )
    .unwrap();
    assert_eq!(
        RetentionV2::sweep_resolved_approvals(&conn, 50, 1).unwrap(),
        1
    );
    assert_eq!(state(&conn, pending), 0);
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM approvals WHERE state = 3", [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM approval_resources", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn pending_and_newer_expired_rows_are_protected() {
    let (_dir, conn) = workspace();
    let session = session_pk(&conn);
    let pending = ApprovalsV2::request(
        &conn, session, None, &[7; 32], 1, "delete", false, None, 0, 10_000,
    )
    .unwrap();
    let fresh = expired(&conn, session, 1000);
    assert_eq!(
        RetentionV2::sweep_resolved_approvals(&conn, 100, 500).unwrap(),
        0
    );
    assert_eq!(state(&conn, pending), 0);
    assert_eq!(state(&conn, fresh), 3);
    assert_eq!(
        RetentionV2::retention_backlog(&conn, 100).unwrap(),
        (0, 0, 0)
    );
}
