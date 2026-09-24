//! APP-012 restart/resume durable-storage behavior lock (partial RED boundary).
//!
//! The accepted 18-row crash/recovery matrix is defined in
//! `worklog/APP012-RESTART-RESUME-CONTRACT.md` (+ CORRECTION). This file covers
//! the matrix rows that are observable through EXISTING public storage APIs at
//! this revision: disposable SQLite workspace, process-equivalent crash (drop
//! connection), reopen with `SchemaV2::open_existing`, then assert the durable
//! fence/CAS/terminal/owner-slot invariants that a correct recovery owner must
//! preserve and must never blindly replay.
//!
//! Two test groups live here:
//!
//! * GREEN behavior locks (existing fences, expected to pass on the current
//!   tree): the durable CAS/terminal/owner-slot invariants across a reopen.
//! * TRUE RED (`startup_*`): the accepted contract requires the startup path to
//!   read prior `clean_shutdown`, write `clean_shutdown=0`, advance
//!   `workspace_state.owner_generation`, and run a bounded conservative recovery
//!   scan (running execution -> uncertain, prepared/dispatched attempt/tool ->
//!   uncertain, terminal rows untouched, never a blind replay). The startup
//!   seam is the existing production API `StorageFacade::open`; the current
//!   implementation (`SchemaV2::open_existing`) does none of this, so the
//!   `startup_*` tests compile and fail behaviorally. They reference only
//!   existing public APIs; no `RecoveryV2` symbol exists or is referenced.
//!
//! No user database is touched: every test builds its own `tempfile::tempdir()`
//! workspace. No network, no wall-clock dependence, no secrets.
#![forbid(unsafe_code)]

use opencode_rk_storage::{
    admission_v2::AdmissionV2, approvals_v2::ApprovalsV2, execution_v2::ExecV2,
    facade::StorageFacade, schema_v2::SchemaV2,
};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn path_of(dir: &Path) -> PathBuf {
    dir.join("w.db")
}

fn init(dir: &Path) -> Connection {
    SchemaV2::initialize_workspace(&path_of(dir), [1_u8; 16], [2_u8; 16], 10).unwrap()
}

fn insert_session(connection: &Connection, id: &[u8; 16]) -> i64 {
    connection
        .execute(
            "INSERT INTO sessions (id, title, created_at_us, updated_at_us)
             VALUES (?1, 't', 0, 1)",
            params![&id[..]],
        )
        .unwrap();
    connection.last_insert_rowid()
}

fn insert_assistant(connection: &Connection, session_pk: i64, seq: i64, now_us: i64) -> i64 {
    connection
        .execute(
            "INSERT INTO messages
             (id, session_pk, seq, role, status, created_at_us, completed_at_us)
             VALUES (randomblob(16), ?1, ?2, 2, 1, ?3, ?3)",
            params![session_pk, seq, now_us],
        )
        .unwrap();
    connection.last_insert_rowid()
}

fn insert_payload(connection: &Connection, data: &[u8], now_us: i64) -> i64 {
    connection
        .execute(
            "INSERT INTO payloads (inline_data, raw_bytes, created_at_us) VALUES (?1, ?2, ?3)",
            params![data, data.len() as i64, now_us],
        )
        .unwrap();
    connection.last_insert_rowid()
}

fn exec_state(connection: &Connection, pk: i64) -> (i64, Option<i64>) {
    connection
        .query_row(
            "SELECT state, finished_at_us FROM executions WHERE pk=?1",
            params![pk],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
}

fn tool_state(connection: &Connection, pk: i64) -> (i64, Option<i64>) {
    connection
        .query_row(
            "SELECT state, finished_at_us FROM tool_calls WHERE pk=?1",
            params![pk],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
}

fn attempt_state(connection: &Connection, pk: i64) -> (i64, Option<i64>) {
    connection
        .query_row(
            "SELECT state, finished_at_us FROM provider_attempts WHERE pk=?1",
            params![pk],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
}

fn approval_state(connection: &Connection, pk: i64) -> (i64, Option<i64>) {
    connection
        .query_row(
            "SELECT state, resolved_at_us FROM approvals WHERE pk=?1",
            params![pk],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
}

fn markers(connection: &Connection) -> (i64, i64) {
    connection
        .query_row(
            "SELECT owner_generation, clean_shutdown FROM workspace_state WHERE id=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
}

// ---------------------------------------------------------------------------
// GREEN behavior locks: durable fences that already hold on this tree. These
// are expected to PASS today and must stay green after the recovery owner lands.
// ---------------------------------------------------------------------------

// Matrix row 1: a pending human decision is not history and survives a restart.
#[test]
fn pending_approval_survives_reopen_and_resolves_once() {
    let dir = tempdir().unwrap();
    let path = path_of(dir.path());
    let conn = init(dir.path());
    let session = insert_session(&conn, &[3_u8; 16]);
    let pk = ApprovalsV2::request(
        &conn,
        session,
        None,
        &[7_u8; 32],
        1,
        "delete",
        false,
        None,
        0,
        100,
    )
    .unwrap();
    assert_eq!(approval_state(&conn, pk), (0, None));
    drop(conn);

    let reopened = SchemaV2::open_existing(&path).unwrap();
    assert_eq!(
        approval_state(&reopened, pk),
        (0, None),
        "row 1: pending approval must survive restart undecided"
    );
    ApprovalsV2::resolve(&reopened, pk, 1, None, 150).unwrap();
    assert_eq!(approval_state(&reopened, pk), (1, Some(150)));
    // Single-winner CAS: the grant cannot be handed out twice.
    assert!(ApprovalsV2::resolve(&reopened, pk, 2, None, 151).is_err());
    assert_eq!(approval_state(&reopened, pk), (1, Some(150)));
}

// Matrix row 12: consumed (4) plus the pending-only resolve CAS is the durable
// cross-restart replay fence. Process-local `GrantLedger` does not persist.
#[test]
fn consumed_approval_state4_blocks_cross_restart_replay() {
    let dir = tempdir().unwrap();
    let path = path_of(dir.path());
    let conn = init(dir.path());
    let session = insert_session(&conn, &[4_u8; 16]);
    let pk = ApprovalsV2::request(
        &conn,
        session,
        None,
        &[9_u8; 32],
        1,
        "delete",
        false,
        None,
        0,
        100,
    )
    .unwrap();
    ApprovalsV2::resolve(&conn, pk, 4, Some(&[5_u8; 16]), 20).unwrap();
    drop(conn);

    let reopened = SchemaV2::open_existing(&path).unwrap();
    assert_eq!(
        approval_state(&reopened, pk),
        (4, Some(20)),
        "row 12: consumed state must persist"
    );
    assert!(ApprovalsV2::resolve(&reopened, pk, 1, None, 30).is_err());
    assert!(ApprovalsV2::resolve(&reopened, pk, 4, None, 31).is_err());
    assert_eq!(approval_state(&reopened, pk), (4, Some(20)));
}

// Same-session mandatory-human evidence survives restart and still demands an
// authenticated client identity (approvals_v2 mandatory gate).
#[test]
fn mandatory_human_approval_requires_client_identity_after_restart() {
    let dir = tempdir().unwrap();
    let path = path_of(dir.path());
    let conn = init(dir.path());
    let session = insert_session(&conn, &[6_u8; 16]);
    let pk = ApprovalsV2::request(
        &conn,
        session,
        None,
        &[1_u8; 32],
        1,
        "delete",
        true,
        None,
        0,
        100,
    )
    .unwrap();
    drop(conn);

    let reopened = SchemaV2::open_existing(&path).unwrap();
    assert!(ApprovalsV2::resolve(&reopened, pk, 1, None, 10).is_err());
    ApprovalsV2::resolve(&reopened, pk, 1, Some(&[3_u8; 16]), 10).unwrap();
    let stored: Vec<u8> = reopened
        .query_row(
            "SELECT human_client_id FROM approvals WHERE pk=?1",
            params![pk],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored, vec![3_u8; 16]);
}

// Matrix rows 7/8/9: success(2)/failure(3)/cancelled(4) tools are terminal,
// survive restart, and reject a rewrite (no fabricated flip after recovery).
#[test]
fn terminal_tool_states_survive_reopen_and_reject_rewrite() {
    let dir = tempdir().unwrap();
    let path = path_of(dir.path());
    let conn = init(dir.path());
    let session = insert_session(&conn, &[7_u8; 16]);
    let config = insert_payload(&conn, b"cfg", 100);
    let input = insert_payload(&conn, b"input", 100);
    let msg = insert_assistant(&conn, session, 1, 100);
    let exec = ExecV2::start_execution(&conn, session, 0, 0, config, "p", "m", 100).unwrap();

    let ok = ExecV2::plan_tool(
        &conn,
        session,
        exec,
        msg,
        0,
        "t-ok",
        &[1_u8; 32],
        input,
        110,
    )
    .unwrap();
    let out = insert_payload(&conn, b"ok", 120);
    ExecV2::finish_tool(&conn, ok, 2, Some(out), None, 120).unwrap();

    let failed = ExecV2::plan_tool(
        &conn,
        session,
        exec,
        msg,
        1,
        "t-fail",
        &[2_u8; 32],
        input,
        110,
    )
    .unwrap();
    let err = insert_payload(&conn, b"boom", 120);
    ExecV2::finish_tool(&conn, failed, 3, None, Some(err), 120).unwrap();

    let cancelled = ExecV2::plan_tool(
        &conn,
        session,
        exec,
        msg,
        2,
        "t-cancel",
        &[3_u8; 32],
        input,
        110,
    )
    .unwrap();
    ExecV2::finish_tool(&conn, cancelled, 4, None, None, 130).unwrap();
    drop(conn);

    let reopened = SchemaV2::open_existing(&path).unwrap();
    assert_eq!(tool_state(&reopened, ok), (2, Some(120)));
    assert_eq!(tool_state(&reopened, failed), (3, Some(120)));
    assert_eq!(tool_state(&reopened, cancelled), (4, Some(130)));
    assert!(ExecV2::finish_tool(&reopened, ok, 3, None, Some(out), 200).is_err());
    assert!(ExecV2::finish_tool(&reopened, failed, 2, Some(out), None, 200).is_err());
    assert!(ExecV2::finish_tool(&reopened, cancelled, 5, None, None, 200).is_err());
    assert_eq!(tool_state(&reopened, ok), (2, Some(120)));
}

// Matrix rows 4/5/11/17: an uncertain execution(4) and a dispatched tool(1)
// keep the single-owner slot across restart; a second executor is rejected.
#[test]
fn uncertain_execution_and_dispatched_tool_hold_owner_slot() {
    let dir = tempdir().unwrap();
    let path = path_of(dir.path());
    let conn = init(dir.path());
    let session = insert_session(&conn, &[8_u8; 16]);
    let config = insert_payload(&conn, b"cfg", 100);
    let input = insert_payload(&conn, b"input", 100);
    let msg = insert_assistant(&conn, session, 1, 100);
    let exec = ExecV2::start_execution(&conn, session, 0, 0, config, "p", "m", 100).unwrap();
    ExecV2::transition_execution(&conn, exec, 0, 1, None).unwrap();
    ExecV2::transition_execution(&conn, exec, 1, 4, None).unwrap();
    let tool =
        ExecV2::plan_tool(&conn, session, exec, msg, 0, "t", &[4_u8; 32], input, 110).unwrap();
    // No API emits the dispatched state; plant it the way a crash mid-dispatch
    // leaves it (the row is what recovery must observe).
    conn.execute(
        "UPDATE tool_calls SET state=1, dispatched_at_us=?1 WHERE pk=?2",
        params![110, tool],
    )
    .unwrap();
    drop(conn);

    let reopened = SchemaV2::open_existing(&path).unwrap();
    assert_eq!(
        exec_state(&reopened, exec),
        (4, None),
        "row 5/11: uncertain execution stays open (finished NULL)"
    );
    assert_eq!(
        tool_state(&reopened, tool),
        (1, None),
        "row 4/15/16: dispatched tool with no result stays open"
    );
    assert!(
        ExecV2::start_execution(&reopened, session, 0, 1, config, "p", "m", 9000).is_err(),
        "row 11/17: uncertain execution retains the single-owner slot"
    );
}

// Matrix row 11 resolution path: only an explicit 4->2/3/5 CAS releases the
// owner slot; nothing here auto-replays the ambiguous effect.
#[test]
fn uncertain_execution_resolves_to_terminal_and_releases_slot() {
    let dir = tempdir().unwrap();
    let path = path_of(dir.path());
    let conn = init(dir.path());
    let session = insert_session(&conn, &[9_u8; 16]);
    let config = insert_payload(&conn, b"cfg", 100);
    let exec = ExecV2::start_execution(&conn, session, 0, 0, config, "p", "m", 100).unwrap();
    ExecV2::transition_execution(&conn, exec, 0, 1, None).unwrap();
    ExecV2::transition_execution(&conn, exec, 1, 4, None).unwrap();
    drop(conn);

    let reopened = SchemaV2::open_existing(&path).unwrap();
    ExecV2::transition_execution(&reopened, exec, 4, 2, Some(2000)).unwrap();
    assert_eq!(exec_state(&reopened, exec), (2, Some(2000)));
    assert!(ExecV2::start_execution(&reopened, session, 0, 1, config, "p", "m", 2010).is_ok());
}

// Matrix row 17: a second client opens the same durable rows read-only and its
// duplicate execution is rejected by the single-owner partial UNIQUE index.
#[test]
fn second_client_observes_same_durable_rows_and_second_execution_rejected() {
    let dir = tempdir().unwrap();
    let path = path_of(dir.path());
    let conn = init(dir.path());
    let session = insert_session(&conn, &[10_u8; 16]);
    let config = insert_payload(&conn, b"cfg", 100);
    let exec = ExecV2::start_execution(&conn, session, 0, 0, config, "p", "m", 100).unwrap();
    ExecV2::transition_execution(&conn, exec, 0, 1, None).unwrap();
    ExecV2::transition_execution(&conn, exec, 1, 4, None).unwrap();
    drop(conn);

    let client_one = SchemaV2::open_existing(&path).unwrap();
    let client_two = SchemaV2::open_existing(&path).unwrap();
    assert_eq!(exec_state(&client_one, exec), exec_state(&client_two, exec));
    assert_eq!(exec_state(&client_two, exec), (4, None));
    assert!(ExecV2::start_execution(&client_one, session, 0, 1, config, "p", "m", 500).is_err());
    assert!(ExecV2::start_execution(&client_two, session, 0, 1, config, "p", "m", 501).is_err());
}

// Matrix row 18: UNIQUE(assistant_message_pk, ordinal) rejects a duplicate tool
// invocation submitted by a second client; the first copy wins.
#[test]
fn duplicate_tool_ordinal_rejected_by_unique_index() {
    let dir = tempdir().unwrap();
    let path = path_of(dir.path());
    let conn = init(dir.path());
    let session = insert_session(&conn, &[11_u8; 16]);
    let config = insert_payload(&conn, b"cfg", 100);
    let input = insert_payload(&conn, b"input", 100);
    let msg = insert_assistant(&conn, session, 1, 100);
    let exec = ExecV2::start_execution(&conn, session, 0, 0, config, "p", "m", 100).unwrap();
    drop(conn);

    let client_one = SchemaV2::open_existing(&path).unwrap();
    let client_two = SchemaV2::open_existing(&path).unwrap();
    let first = ExecV2::plan_tool(
        &client_one,
        session,
        exec,
        msg,
        0,
        "t",
        &[6_u8; 32],
        input,
        110,
    )
    .unwrap();
    assert!(
        ExecV2::plan_tool(
            &client_two,
            session,
            exec,
            msg,
            0,
            "t",
            &[6_u8; 32],
            input,
            111
        )
        .is_err(),
        "row 18: duplicate (assistant_message_pk, ordinal) must be rejected"
    );
    assert_eq!(tool_state(&client_two, first).0, 0);
}

// Matrix rows 13/14: a pending input is not history and is not auto-promoted on
// restart; promotion happens once and is idempotent by state.
#[test]
fn pending_input_survives_reopen_and_promotes_once() {
    let dir = tempdir().unwrap();
    let path = path_of(dir.path());
    let mut conn = init(dir.path());
    insert_session(&conn, &[12_u8; 16]);
    let session_id = [12_u8; 16];
    let (input_pk, seq) = AdmissionV2::submit_input(
        &mut conn,
        &session_id,
        0,
        &[2_u8; 32],
        &[(0, b"hi".to_vec())],
    )
    .unwrap();
    assert_eq!(seq, 1);
    drop(conn);

    let mut reopened = SchemaV2::open_existing(&path).unwrap();
    let state: i64 = reopened
        .query_row(
            "SELECT state FROM session_inputs WHERE pk=?1",
            params![input_pk],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        state, 0,
        "row 13: pending input stays pending across restart"
    );
    let message_pk = AdmissionV2::promote_input(&mut reopened, input_pk).unwrap();
    let (state, promoted): (i64, Option<i64>) = reopened
        .query_row(
            "SELECT state, promoted_message_pk FROM session_inputs WHERE pk=?1",
            params![input_pk],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((state, promoted), (1, Some(message_pk)));
    assert!(
        AdmissionV2::promote_input(&mut reopened, input_pk).is_err(),
        "row 14: promotion is idempotent by state"
    );
}

// Matrix rows 10/15/16: prepared attempts survive restart; a terminal attempt is
// immutable. Marking uncertainty is an explicit, non-replaying transition.
#[test]
fn provider_attempt_survives_reopen_and_terminal_attempt_immutable() {
    let dir = tempdir().unwrap();
    let path = path_of(dir.path());
    let conn = init(dir.path());
    let session = insert_session(&conn, &[13_u8; 16]);
    let config = insert_payload(&conn, b"cfg", 100);
    let exec = ExecV2::start_execution(&conn, session, 0, 0, config, "p", "m", 100).unwrap();
    let prepared = ExecV2::record_attempt(&conn, exec, 0, &[7_u8; 32], 1, 110).unwrap();
    let done = ExecV2::record_attempt(&conn, exec, 1, &[8_u8; 32], 1, 111).unwrap();
    ExecV2::finish_attempt(&conn, done, 2, 120).unwrap();
    drop(conn);

    let reopened = SchemaV2::open_existing(&path).unwrap();
    assert_eq!(
        attempt_state(&reopened, prepared),
        (0, None),
        "row 15/16: prepared attempt survives restart unresolved"
    );
    ExecV2::finish_attempt(&reopened, prepared, 4, 130).unwrap();
    assert_eq!(attempt_state(&reopened, prepared), (4, None));
    assert!(
        ExecV2::finish_attempt(&reopened, done, 3, 140).is_err(),
        "row 10: a terminal attempt must not be rewritten"
    );
    assert_eq!(attempt_state(&reopened, done), (2, Some(120)));
}

// Matrix row 2: expiry is owned solely by the bounded sweep and never by a
// resolve; expired rows cannot later be granted.
#[test]
fn expired_approval_sweep_is_explicit_and_bounded() {
    let dir = tempdir().unwrap();
    let conn = init(dir.path());
    let session = insert_session(&conn, &[14_u8; 16]);
    let due_a = ApprovalsV2::request(
        &conn,
        session,
        None,
        &[1_u8; 32],
        1,
        "delete",
        false,
        None,
        0,
        100,
    )
    .unwrap();
    let due_b = ApprovalsV2::request(
        &conn,
        session,
        None,
        &[2_u8; 32],
        1,
        "delete",
        false,
        None,
        0,
        200,
    )
    .unwrap();
    let fresh = ApprovalsV2::request(
        &conn,
        session,
        None,
        &[3_u8; 32],
        1,
        "delete",
        false,
        None,
        0,
        10_000,
    )
    .unwrap();
    assert_eq!(ApprovalsV2::expire_sweep(&conn, 500, 500).unwrap(), 2);
    assert_eq!(approval_state(&conn, due_a).0, 3);
    assert_eq!(approval_state(&conn, due_b).0, 3);
    assert_eq!(approval_state(&conn, fresh).0, 0);
    assert!(ApprovalsV2::resolve(&conn, due_a, 1, None, 501).is_err());
    // Sweep is idempotent and bounded.
    assert_eq!(ApprovalsV2::expire_sweep(&conn, 500, 500).unwrap(), 0);
}

// Clean/unclean shutdown marker: `StorageFacade::close` writes the durable
// clean-shutdown bit.
#[test]
fn clean_shutdown_marker_written_on_facade_close() {
    let dir = tempdir().unwrap();
    let path = path_of(dir.path());
    {
        let facade = StorageFacade::open(&path).unwrap();
        facade.close().unwrap();
    }
    let reopened = SchemaV2::open_existing(&path).unwrap();
    let (_generation, clean) = markers(&reopened);
    assert_eq!(clean, 1, "facade close records a clean shutdown");
}

// ---------------------------------------------------------------------------
// TRUE RED: startup recovery through the existing production seam
// `StorageFacade::open`. Current `SchemaV2::open_existing` reads no
// `clean_shutdown`, advances no `owner_generation`, and runs no recovery scan,
// so every `startup_*` test below compiles and FAILS behaviorally. They must
// turn GREEN when the startup fencing + recovery owner lands, with zero edits
// to this file.
// ---------------------------------------------------------------------------

/// One running (1) execution owned by its own session, with a dispatched (1)
/// tool and a prepared (0) + dispatched (1) provider attempt. This is the
/// crash shape the startup scan must convert to conservative uncertainty.
fn seed_running_crash(connection: &Connection, session_id: [u8; 16], base: i64) -> (i64, i64) {
    let session = insert_session(connection, &session_id);
    let config = insert_payload(connection, b"cfg", base);
    let input = insert_payload(connection, b"input", base);
    let msg = insert_assistant(connection, session, 1, base);
    let exec = ExecV2::start_execution(connection, session, 0, 0, config, "p", "m", base).unwrap();
    ExecV2::transition_execution(connection, exec, 0, 1, None).unwrap();
    ExecV2::record_attempt(connection, exec, 0, &[7_u8; 32], 1, base + 1).unwrap();
    let dispatched = ExecV2::record_attempt(connection, exec, 1, &[8_u8; 32], 1, base + 2).unwrap();
    connection
        .execute(
            "UPDATE provider_attempts SET state=1, dispatched_at_us=?1 WHERE pk=?2",
            params![base + 2, dispatched],
        )
        .unwrap();
    let tool = ExecV2::plan_tool(
        connection,
        session,
        exec,
        msg,
        0,
        "t",
        &[4_u8; 32],
        input,
        base + 3,
    )
    .unwrap();
    connection
        .execute(
            "UPDATE tool_calls SET state=1, dispatched_at_us=?1 WHERE pk=?2",
            params![base + 3, tool],
        )
        .unwrap();
    (exec, tool)
}

fn read_markers_after_startup(path: &Path) -> (i64, i64) {
    let reopened = SchemaV2::open_existing(path).unwrap();
    markers(&reopened)
}

fn read_exec_state_after_startup(path: &Path, pk: i64) -> (i64, Option<i64>) {
    let reopened = SchemaV2::open_existing(path).unwrap();
    exec_state(&reopened, pk)
}

// RED: startup must advance `workspace_state.owner_generation` exactly once,
// under FULL, BEFORE accepting work (contract "Recovery ownership" + startup
// caller). Current open leaves it at 0.
#[test]
fn startup_advances_owner_generation() {
    let dir = tempdir().unwrap();
    let path = path_of(dir.path());
    let conn = init(dir.path());
    drop(conn);

    let facade = StorageFacade::open(&path).unwrap();
    drop(facade);

    let (generation, _clean) = read_markers_after_startup(&path);
    assert_eq!(
        generation, 1,
        "startup must advance owner_generation by exactly one generation"
    );
}

// RED: a clean close writes clean_shutdown=1; the next startup must clear it to
// 0 as part of taking ownership, so a later crash is distinguishable. Current
// open never reads or writes the marker.
#[test]
fn startup_clears_clean_shutdown_before_accepting_work() {
    let dir = tempdir().unwrap();
    let path = path_of(dir.path());
    {
        let facade = StorageFacade::open(&path).unwrap();
        facade.close().unwrap();
    }
    let (_, after_clean_close) = read_markers_after_startup(&path);
    assert_eq!(after_clean_close, 1, "close leaves the clean marker set");

    let facade = StorageFacade::open(&path).unwrap();
    drop(facade);
    let (_, after_startup) = read_markers_after_startup(&path);
    assert_eq!(
        after_startup, 0,
        "startup must clear clean_shutdown to 0 before accepting work"
    );
}

// RED: matrix row 5/11/15/16: an execution left running by a crash becomes
// uncertain(4) on startup and keeps the owner slot; it is never silently
// retried or fabricated terminal.
#[test]
fn startup_converts_running_execution_to_uncertain() {
    let dir = tempdir().unwrap();
    let path = path_of(dir.path());
    let conn = init(dir.path());
    let (exec, _tool) = seed_running_crash(&conn, [20_u8; 16], 1000);
    assert_eq!(exec_state(&conn, exec), (1, None));
    drop(conn);

    let facade = StorageFacade::open(&path).unwrap();
    drop(facade);

    assert_eq!(
        read_exec_state_after_startup(&path, exec),
        (4, None),
        "row 5: running execution must become uncertain(4) with finished NULL"
    );
}

// RED: matrix rows 4/15/16: prepared(0)/dispatched(1) attempts and a
// dispatched tool become uncertain on startup, with finished_at_us NULL
// (ambiguous effects, manual review, never auto-replayed).
#[test]
fn startup_converts_open_attempts_and_tool_to_uncertain() {
    let dir = tempdir().unwrap();
    let path = path_of(dir.path());
    let conn = init(dir.path());
    let (_exec, tool) = seed_running_crash(&conn, [21_u8; 16], 2000);
    drop(conn);

    let facade = StorageFacade::open(&path).unwrap();
    drop(facade);

    let reopened = SchemaV2::open_existing(&path).unwrap();
    let attempts: Vec<(i64, i64, Option<i64>)> = {
        let mut stmt = reopened
            .prepare("SELECT pk, state, finished_at_us FROM provider_attempts ORDER BY pk")
            .unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    assert!(!attempts.is_empty());
    for (pk, state, finished) in &attempts {
        assert_eq!(
            (*state, *finished),
            (4, None),
            "row 4/15/16: attempt {pk} must become uncertain(4), got state {state}"
        );
    }
    assert_eq!(
        tool_state(&reopened, tool),
        (5, None),
        "row 4: dispatched tool must become uncertain(5) with finished NULL"
    );
}

// INVARIANT (must hold before AND after recovery lands): terminal rows are
// never rewritten by startup. Currently a no-op; the recovery owner must keep
// it a no-op.
#[test]
fn startup_does_not_rewrite_terminal_states() {
    let dir = tempdir().unwrap();
    let path = path_of(dir.path());
    let conn = init(dir.path());
    let session = insert_session(&conn, &[22_u8; 16]);
    let config = insert_payload(&conn, b"cfg", 100);
    let input = insert_payload(&conn, b"input", 100);
    let msg = insert_assistant(&conn, session, 1, 100);
    let exec = ExecV2::start_execution(&conn, session, 0, 0, config, "p", "m", 100).unwrap();
    ExecV2::transition_execution(&conn, exec, 0, 1, None).unwrap();
    let tool =
        ExecV2::plan_tool(&conn, session, exec, msg, 0, "t", &[1_u8; 32], input, 110).unwrap();
    let out = insert_payload(&conn, b"ok", 120);
    ExecV2::finish_tool(&conn, tool, 2, Some(out), None, 120).unwrap();
    let attempt = ExecV2::record_attempt(&conn, exec, 0, &[2_u8; 32], 1, 130).unwrap();
    ExecV2::finish_attempt(&conn, attempt, 2, 140).unwrap();
    // Terminal execution only after its attempt/tool rows exist.
    ExecV2::transition_execution(&conn, exec, 1, 2, Some(200)).unwrap();
    drop(conn);

    let facade = StorageFacade::open(&path).unwrap();
    drop(facade);
    let reopened = SchemaV2::open_existing(&path).unwrap();

    assert_eq!(exec_state(&reopened, exec), (2, Some(200)));
    assert_eq!(tool_state(&reopened, tool), (2, Some(120)));
    assert_eq!(attempt_state(&reopened, attempt), (2, Some(140)));
}

// RED: the startup recovery scan is bounded (contract MAX_SWEEP_ROWS=500) and
// drains across passes. 501 crashed running executions must leave at least one
// for the next startup pass, then drain on the following pass. Unbounded
// recovery (or none at all) fails this.
#[test]
fn startup_recovery_is_bounded_and_drains_across_passes() {
    let dir = tempdir().unwrap();
    let path = path_of(dir.path());
    let conn = init(dir.path());
    for i in 0..501_i64 {
        let mut id = [0_u8; 16];
        id[..8].copy_from_slice(&i.to_le_bytes());
        let session = insert_session(&conn, &id);
        let config = insert_payload(&conn, b"cfg", 100 + i);
        let exec =
            ExecV2::start_execution(&conn, session, 0, 0, config, "p", "m", 100 + i).unwrap();
        ExecV2::transition_execution(&conn, exec, 0, 1, None).unwrap();
    }
    drop(conn);

    let facade = StorageFacade::open(&path).unwrap();
    drop(facade);
    let after_first = SchemaV2::open_existing(&path).unwrap();
    let running_after_first: i64 = after_first
        .query_row("SELECT COUNT(*) FROM executions WHERE state=1", [], |row| {
            row.get(0)
        })
        .unwrap();
    drop(after_first);
    // Contract: each startup recovery scan is bounded by MAX_SWEEP_ROWS=500.
    // One open must transition at least one row (progress) and at most 500
    // (bounded), leaving the rest for the next pass. `running` counts what the
    // first pass did NOT take: 1..=500 proves a per-open cap of <=500 exists and
    // that no single open is unbounded.
    assert!(
        (1..=500).contains(&running_after_first),
        "bounded scan must leave 1..=500 rows for the next pass, got {running_after_first} running"
    );

    let facade = StorageFacade::open(&path).unwrap();
    drop(facade);
    let after_second = SchemaV2::open_existing(&path).unwrap();
    let running_after_second: i64 = after_second
        .query_row("SELECT COUNT(*) FROM executions WHERE state=1", [], |row| {
            row.get(0)
        })
        .unwrap();
    let uncertain: i64 = after_second
        .query_row("SELECT COUNT(*) FROM executions WHERE state=4", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(
        running_after_second, 0,
        "second startup pass must drain the remaining crashed rows"
    );
    assert_eq!(uncertain, 501, "every crashed row must end uncertain(4)");
}
