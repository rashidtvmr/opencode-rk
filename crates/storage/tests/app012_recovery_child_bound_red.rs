//! APP-012 recovery residual R1: bounded root recovery must not strand children.
//!
//! A single prior-generation running execution owns 501 open provider attempts.
//! `StorageFacade::open` may inspect at most 500 child rows per startup, but once
//! it converts the root to uncertain, later opens must still reconcile the child
//! row left outside that first child batch. The terminal attempt is a rewrite
//! guard and must remain unchanged throughout.
#![forbid(unsafe_code)]

use opencode_rk_storage::{execution_v2::ExecV2, facade::StorageFacade, schema_v2::SchemaV2};
use rusqlite::{params, Connection};
use tempfile::tempdir;

const OPEN_ATTEMPTS: usize = 501;

fn insert_session(connection: &Connection) -> i64 {
    connection
        .execute(
            "INSERT INTO sessions (id, title, created_at_us, updated_at_us)
             VALUES (randomblob(16), 'recovery-bound', 1, 1)",
            [],
        )
        .unwrap();
    connection.last_insert_rowid()
}

fn insert_payload(connection: &Connection, data: &[u8]) -> i64 {
    connection
        .execute(
            "INSERT INTO payloads (inline_data, raw_bytes, created_at_us)
             VALUES (?1, ?2, 1)",
            params![data, data.len() as i64],
        )
        .unwrap();
    connection.last_insert_rowid()
}

fn attempt_counts(connection: &Connection) -> (i64, i64, i64) {
    connection
        .query_row(
            "SELECT
                 COALESCE(SUM(state IN (0, 1)), 0),
                 COALESCE(SUM(state = 4), 0),
                 COUNT(*)
             FROM provider_attempts",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap()
}

fn execution_state(connection: &Connection, execution_pk: i64) -> (i64, Option<i64>) {
    connection
        .query_row(
            "SELECT state, finished_at_us FROM executions WHERE pk=?1",
            params![execution_pk],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
}

fn attempt_state(connection: &Connection, attempt_pk: i64) -> (i64, Option<i64>) {
    connection
        .query_row(
            "SELECT state, finished_at_us FROM provider_attempts WHERE pk=?1",
            params![attempt_pk],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
}

#[test]
fn recovery_cleans_children_for_each_selected_root_atomically() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("workspace.db");
    let connection = SchemaV2::initialize_workspace(&path, [3_u8; 16], [4_u8; 16], 1).unwrap();
    let config_pk = insert_payload(&connection, b"config");

    for ordinal in 0..OPEN_ATTEMPTS {
        let session_pk = insert_session(&connection);
        let execution_pk = ExecV2::start_execution(
            &connection,
            session_pk,
            0,
            0,
            config_pk,
            "provider",
            "model",
            10 + ordinal as i64,
        )
        .unwrap();
        ExecV2::transition_execution(&connection, execution_pk, 0, 1, None).unwrap();
        ExecV2::record_attempt(
            &connection,
            execution_pk,
            0,
            &[9_u8; 32],
            1,
            1000 + ordinal as i64,
        )
        .unwrap();
    }
    drop(connection);

    let first = StorageFacade::open(&path).unwrap();
    first
        .with_connection(|connection| {
            let uncertain_roots: i64 = connection
                .query_row("SELECT COUNT(*) FROM executions WHERE state=4", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert!(uncertain_roots <= 500, "startup root bound exceeded");
            assert_eq!(uncertain_roots, 500);
            assert_eq!(attempt_counts(connection), (1, 500, OPEN_ATTEMPTS as i64));
            let open_children_on_uncertain_roots: i64 = connection
                .query_row(
                    "SELECT COUNT(*)
                     FROM provider_attempts AS a
                     JOIN executions AS e ON e.pk=a.execution_pk
                     WHERE e.state=4 AND a.state IN (0,1)",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(
                open_children_on_uncertain_roots, 0,
                "selected roots must not retain open children"
            );
            Ok(())
        })
        .unwrap();
    drop(first);

    let second = StorageFacade::open(&path).unwrap();
    second
        .with_connection(|connection| {
            let (open, uncertain, total) = attempt_counts(connection);
            assert_eq!(
                (open, uncertain, total),
                (0, OPEN_ATTEMPTS as i64, OPEN_ATTEMPTS as i64)
            );
            let uncertain_roots: i64 = connection
                .query_row("SELECT COUNT(*) FROM executions WHERE state=4", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(uncertain_roots, OPEN_ATTEMPTS as i64);
            Ok(())
        })
        .unwrap();
}

#[test]
fn recovery_does_not_strand_children_after_bounded_root_scan() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("workspace.db");
    let connection = SchemaV2::initialize_workspace(&path, [1_u8; 16], [2_u8; 16], 1).unwrap();
    let session_pk = insert_session(&connection);
    let config_pk = insert_payload(&connection, b"config");
    let execution_pk = ExecV2::start_execution(
        &connection,
        session_pk,
        0,
        0,
        config_pk,
        "provider",
        "model",
        10,
    )
    .unwrap();
    ExecV2::transition_execution(&connection, execution_pk, 0, 1, None).unwrap();

    for ordinal in 0..OPEN_ATTEMPTS {
        ExecV2::record_attempt(
            &connection,
            execution_pk,
            ordinal as i64,
            &[7_u8; 32],
            1,
            20 + ordinal as i64,
        )
        .unwrap();
    }
    let terminal_attempt = ExecV2::record_attempt(
        &connection,
        execution_pk,
        OPEN_ATTEMPTS as i64,
        &[8_u8; 32],
        1,
        1000,
    )
    .unwrap();
    ExecV2::finish_attempt(&connection, terminal_attempt, 2, 1001).unwrap();
    assert_eq!(attempt_counts(&connection), (OPEN_ATTEMPTS as i64, 0, 502));
    drop(connection);

    // The first startup pass is bounded. It must atomically leave the selected
    // children and their running root in the same uncertain recovery state.
    let first = StorageFacade::open(&path).unwrap();
    first
        .with_connection(|connection| {
            assert_eq!(execution_state(connection, execution_pk), (4, None));
            assert_eq!(attempt_counts(connection), (1, 500, 502));
            assert_eq!(attempt_state(connection, terminal_attempt), (2, Some(1001)));
            Ok(())
        })
        .unwrap();
    drop(first);

    // The remaining child belongs to a root already made uncertain. A second
    // startup must still find it through the real facade, not replay or rewrite
    // the terminal attempt.
    let second = StorageFacade::open(&path).unwrap();
    second
        .with_connection(|connection| {
            assert_eq!(execution_state(connection, execution_pk), (4, None));
            assert_eq!(
                attempt_counts(connection),
                (0, OPEN_ATTEMPTS as i64, (OPEN_ATTEMPTS + 1) as i64),
                "all ambiguous children must eventually become uncertain"
            );
            assert_eq!(attempt_state(connection, terminal_attempt), (2, Some(1001)));
            Ok(())
        })
        .unwrap();
}
