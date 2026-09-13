//! E2E integration tests for the format-2 workspace modules.
//! Exercises writer + admission + execution + approvals + snapshot + quota.

use opencode_rk_storage::{
    admission_v2::AdmissionV2,
    approvals_v2::ApprovalsV2,
    execution_v2::ExecV2,
    quota_v2::QuotaV2,
    schema_v2::SchemaV2,
    snapshot_v2::SnapshotV2,
    writer_v2::{NewSession, V2Writer},
};
use opencode_rk_contracts::SessionId;
use rusqlite::Connection;
use tempfile::{tempdir, TempDir};

fn initialized() -> (TempDir, Connection) {
    let directory = tempdir().unwrap();
    let path = directory.path().join("workspace.db");
    let connection = SchemaV2::initialize_workspace(&path, [1_u8; 16], [2_u8; 16], 10)
        .expect("initialization");
    (directory, connection)
}

fn session_pk(connection: &Connection, session_id: SessionId) -> i64 {
    connection
        .query_row(
            "SELECT pk FROM sessions WHERE id=?1",
            [session_id.as_uuid().as_bytes()],
            |row| row.get(0),
        )
        .expect("session pk")
}

#[test]
fn full_session_lifecycle() {
    let (_directory, mut connection) = initialized();

    // Step 1: init workspace.
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        2
    );

    // Step 2: create session.
    let session_id = SessionId::new();
    V2Writer::create_session(
        &mut connection,
        &NewSession {
            id: session_id,
            title: "e2e-session".into(),
            created_at_us: 100,
            updated_at_us: 100,
        },
    )
    .expect("create session");
    let spk = session_pk(&connection, session_id);

    // Step 3: submit + promote admitted input.
    let (input_pk, seq) = AdmissionV2::submit_input(
        &mut connection,
        session_id.as_uuid().as_bytes(),
        0,
        &[7_u8; 32],
        &[(0, b"hello".to_vec())],
    )
    .expect("submit input");
    assert_eq!(seq, 1);
    let message_pk = AdmissionV2::promote_input(&mut connection, input_pk).expect("promote");
    assert!(message_pk > 0);

    // Step 4: start execution, transition running -> complete.
    let payload_pk: i64 = connection
        .query_row(
            "SELECT pk FROM payloads ORDER BY pk DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let exec_pk = ExecV2::start_execution(
        &connection,
        spk,
        0,
        0,
        payload_pk,
        "test-provider",
        "test-model",
        200,
    )
    .expect("start execution");
    ExecV2::transition_execution(&connection, exec_pk, 0, 1, None).expect("running");
    ExecV2::transition_execution(&connection, exec_pk, 1, 2, Some(300)).expect("complete");

    // Step 5: request + resolve approval.
    let approval_pk = ApprovalsV2::request(
        &connection,
        spk,
        None,
        &[9_u8; 32],
        1,
        "tool:write",
        false,
        None,
        400,
        500,
    )
    .expect("request approval");
    ApprovalsV2::resolve(&connection, approval_pk, 1, None, 450).expect("resolve approval");

    // Step 6: open epoch + export page.
    let epoch = SnapshotV2::open_epoch(&connection, spk, payload_pk, payload_pk, 600)
        .expect("open epoch");
    assert_eq!(epoch, 1);
    let (rows, watermark) = SnapshotV2::export_page(&connection, spk, 0, 100).expect("export");
    assert_eq!(rows.len(), 1);
    assert_eq!(watermark, 1);
    SnapshotV2::close_epoch(&connection, spk, 700).expect("close epoch");

    // Step 7: outbox events.
    V2Writer::append_outbox_event(&mut connection, session_id, "execution_started", "{}")
        .expect("outbox 1");
    V2Writer::append_outbox_event(&mut connection, session_id, "execution_completed", "{}")
        .expect("outbox 2");

    // Step 8: quota measure + reclaim.
    let snapshot = QuotaV2::measure(&connection).expect("measure");
    assert!(snapshot.page_count > 0);
    QuotaV2::reclaim(&connection, 3).expect("reclaim");

    // Final invariants.
    let final_messages: i64 = connection
        .query_row("SELECT count(*) FROM messages", [], |row| row.get(0))
        .expect("messages");
    assert_eq!(final_messages, 1);
    let outbox_count: i64 = connection
        .query_row("SELECT count(*) FROM event_outbox", [], |row| row.get(0))
        .expect("outbox");
    assert_eq!(outbox_count, 2);
    let fk_violations: i64 = connection
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .expect("fk check");
    assert_eq!(fk_violations, 0);
}

#[test]
fn illegal_sequences_rejected() {
    let (_directory, mut connection) = initialized();
    let session_id = SessionId::new();
    V2Writer::create_session(
        &mut connection,
        &NewSession {
            id: session_id,
            title: "test".into(),
            created_at_us: 10,
            updated_at_us: 10,
        },
    )
    .unwrap();
    let spk = session_pk(&connection, session_id);

    // Illegal: promote a nonexistent input.
    assert!(AdmissionV2::promote_input(&mut connection, 9999).is_err());

    // Illegal: resolve a non-pending approval.
    assert!(ApprovalsV2::resolve(&connection, 9999, 1, None, 20).is_err());

    // Illegal: second running execution for the same session owner slot.
    let payload_pk: i64 = connection
        .query_row(
            "INSERT INTO payloads (inline_data, raw_bytes, created_at_us) VALUES (x'', 0, 1) RETURNING pk",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let first = ExecV2::start_execution(&connection, spk, 0, 0, payload_pk, "p", "m", 30).unwrap();
    ExecV2::transition_execution(&connection, first, 0, 1, None).unwrap();
    assert!(ExecV2::start_execution(&connection, spk, 0, 0, payload_pk, "p", "m", 31).is_err());

    // Illegal: transition from the wrong state.
    assert!(ExecV2::transition_execution(&connection, first, 0, 1, None).is_err());

    // Integrity preserved after rejected paths.
    let fk_violations: i64 = connection
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .expect("fk check");
    assert_eq!(fk_violations, 0);
}
