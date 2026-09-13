//! Storage v2 module perf probe (DEBUG BUILD numbers, NOT production claims).
//!
//! All timings come from an unoptimized `cargo test` build on one machine;
//! release builds and cold caches will differ. SQLite 3.53.2 (rusqlite 0.40,
//! bundled; also printed at runtime via rusqlite::version()).
//!
//! Measured 2026-09-13. No new deps, std::time only. Each test prints total +
//! per-op micros via eprintln (use --nocapture) and asserts a generous <10s
//! batch bound so CI stays green on slow machines.
//!
//! gc note: crate::gc_v2 is still a stub lane, so the gc claim probe exercises
//! the schema-level claim CAS directly (`UPDATE blobs SET state=1 ... WHERE
//! state=0`, guarded by the blob_gc_claim trigger).
//! ponytail: upgrade path: re-route through GcV2::claim once that lane lands.

use std::time::{Duration, Instant};

use opencode_rk_contracts::{MessageId, MessageRole, PayloadRef, SessionId};
use opencode_rk_storage::{
    admission_v2::AdmissionV2,
    approvals_v2::ApprovalsV2,
    execution_v2::ExecV2,
    schema_v2::SchemaV2,
    snapshot_v2::SnapshotV2,
    writer_v2::{NewMessage, NewSession, V2Writer},
};
use rusqlite::{params, Connection};
use tempfile::tempdir;

const BATCH: usize = 100;
const BOUND_SECS: u64 = 10;

fn init_db(dir: &tempfile::TempDir) -> Connection {
    SchemaV2::initialize_workspace(&dir.path().join("workspace.db"), [1_u8; 16], [2_u8; 16], 10)
        .unwrap()
}

fn report(label: &str, elapsed: Duration, ops: usize) {
    let per_op_us = elapsed.as_micros() as f64 / ops as f64;
    eprintln!(
        "[perf_modules_v2] {label}: total={elapsed:?} ops={ops} per_op={per_op_us:.1}us"
    );
}

fn check(label: &str, elapsed: Duration) {
    assert!(
        elapsed.as_secs() < BOUND_SECS,
        "{label} batch took {elapsed:?}"
    );
}

fn create_session(conn: &mut Connection) -> (SessionId, i64) {
    let id = SessionId::new();
    V2Writer::create_session(
        conn,
        &NewSession {
            id,
            title: "perf-mod".into(),
            created_at_us: 10,
            updated_at_us: 10,
        },
    )
    .unwrap();
    let session_pk: i64 = conn
        .query_row(
            "SELECT pk FROM sessions WHERE id=?1",
            params![id.as_uuid().as_bytes().as_slice()],
            |row| row.get(0),
        )
        .unwrap();
    (id, session_pk)
}

fn insert_payload(conn: &Connection, data: &[u8], now_us: i64) -> i64 {
    conn.execute(
        "INSERT INTO payloads (inline_data, raw_bytes, created_at_us) VALUES (?1, ?2, ?3)",
        params![data, data.len() as i64, now_us],
    )
    .unwrap();
    conn.last_insert_rowid()
}

#[test]
fn sqlite_version_is_printed() {
    eprintln!(
        "[perf_modules_v2] sqlite_version={}",
        rusqlite::version()
    );
}

#[test]
fn perf_100_admission_submit_promote() {
    let dir = tempdir().unwrap();
    let mut conn = init_db(&dir);
    let (session_id, _pk) = create_session(&mut conn);
    let session_bytes = *session_id.as_uuid().as_bytes();
    let parts = vec![(0_u8, b"hello perf".to_vec())];

    let mut submit_t = Duration::ZERO;
    let mut promote_t = Duration::ZERO;
    let mut input_pks = Vec::with_capacity(BATCH);
    for i in 0..BATCH {
        let start = Instant::now();
        let (input_pk, _seq) = AdmissionV2::submit_input(
            &mut conn,
            &session_bytes,
            0,
            &[i as u8; 32],
            &parts,
        )
        .unwrap();
        submit_t += start.elapsed();
        input_pks.push(input_pk);
        let start = Instant::now();
        AdmissionV2::promote_input(&mut conn, input_pk).unwrap();
        promote_t += start.elapsed();
    }
    report("admission_submit_x100", submit_t, BATCH);
    check("admission_submit", submit_t);
    report("admission_promote_x100", promote_t, BATCH);
    check("admission_promote", promote_t);
}

#[test]
fn perf_100_execution_start_transition() {
    let dir = tempdir().unwrap();
    let mut conn = init_db(&dir);
    let (_id, session_pk) = create_session(&mut conn);
    let config_pk = insert_payload(&conn, b"cfg", 10);

    let mut start_t = Duration::ZERO;
    let mut trans_t = Duration::ZERO;
    let mut ops = 0_usize;
    for i in 0..BATCH {
        // start queue(0) -> running(1) -> complete(2, terminal) so the
        // single-owner index is released before the next start.
        let start = Instant::now();
        let exec_pk =
            ExecV2::start_execution(&conn, session_pk, 0, i as i64, config_pk, "prov", "model", 1000 + i as i64)
                .unwrap();
        start_t += start.elapsed();
        let start = Instant::now();
        ExecV2::transition_execution(&conn, exec_pk, 0, 1, None).unwrap();
        ExecV2::transition_execution(&conn, exec_pk, 1, 2, Some(2000 + i as i64)).unwrap();
        trans_t += start.elapsed();
        ops += 2;
    }
    report("execution_start_x100", start_t, BATCH);
    check("execution_start", start_t);
    report("execution_transition_x200", trans_t, ops);
    check("execution_transition", trans_t);
}

#[test]
fn perf_100_approval_request_resolve() {
    let dir = tempdir().unwrap();
    let mut conn = init_db(&dir);
    let (_id, session_pk) = create_session(&mut conn);
    let human = [9_u8; 16];

    let mut request_t = Duration::ZERO;
    let mut resolve_t = Duration::ZERO;
    for i in 0..BATCH {
        let start = Instant::now();
        let pk = ApprovalsV2::request(
            &conn,
            session_pk,
            None,
            &[i as u8; 32],
            1,
            "write",
            true,
            Some(&human),
            1000 + i as i64,
            10_000 + i as i64,
        )
        .unwrap();
        request_t += start.elapsed();
        let start = Instant::now();
        ApprovalsV2::resolve(&conn, pk, 1, Some(&human), 2000 + i as i64).unwrap();
        resolve_t += start.elapsed();
    }
    report("approval_request_x100", request_t, BATCH);
    check("approval_request", request_t);
    report("approval_resolve_x100", resolve_t, BATCH);
    check("approval_resolve", resolve_t);
}

#[test]
fn perf_100_gc_claim() {
    let dir = tempdir().unwrap();
    let conn = init_db(&dir);
    // Seed 100 unreferenced ready blobs; claim = CAS state 0 -> 1 exactly as
    // GcV2 (stub lane) will do, guarded by the blob_gc_claim trigger.
    let mut pks = Vec::with_capacity(BATCH);
    for i in 0..BATCH {
        conn.execute(
            "INSERT INTO blobs (hash, state, codec, raw_bytes, stored_bytes, created_at_us)
             VALUES (?1, 0, 0, 100, 60, ?2)",
            params![&[i as u8; 32][..], 1000 + i as i64],
        )
        .unwrap();
        pks.push(conn.last_insert_rowid());
    }
    let start = Instant::now();
    for pk in &pks {
        let changed = conn
            .execute(
                "UPDATE blobs SET state=1 WHERE pk=?1 AND state=0",
                params![pk],
            )
            .unwrap();
        assert_eq!(changed, 1);
    }
    let elapsed = start.elapsed();
    report("gc_claim_blobs_x100", elapsed, BATCH);
    check("gc_claim", elapsed);
}

#[test]
fn perf_100_snapshot_export_page() {
    let dir = tempdir().unwrap();
    let mut conn = init_db(&dir);
    let (session_id, session_pk) = create_session(&mut conn);
    // Seed 1000 messages so each page has real rows to walk.
    for i in 0..1000 {
        V2Writer::append_message(
            &mut conn,
            &NewMessage {
                id: MessageId::new(),
                session_id,
                role: MessageRole::User,
                body: PayloadRef::Inline {
                    text: format!("m{i}"),
                },
                created_at_us: 100 + i,
            },
        )
        .unwrap();
    }
    let start = Instant::now();
    let mut total_rows = 0_usize;
    for _ in 0..BATCH {
        let (rows, watermark) =
            SnapshotV2::export_page(&conn, session_pk, 0, 100).unwrap();
        assert_eq!(rows.len(), 100);
        assert!(watermark > 0);
        total_rows += rows.len();
    }
    let elapsed = start.elapsed();
    assert_eq!(total_rows, BATCH * 100);
    report("snapshot_export_page_x100_over_1000msgs", elapsed, BATCH);
    check("snapshot_export_page", elapsed);
}
