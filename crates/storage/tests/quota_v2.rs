//! Integration suite for QuotaV2 on file-backed initialized workspace.
use opencode_rk_storage::quota_v2::QuotaSnapshot;
use opencode_rk_storage::quota_v2::QuotaV2Error;
use opencode_rk_storage::schema_v2::SchemaV2;
use opencode_rk_storage::{NewSession, QuotaV2, V2Writer};
use rusqlite::Connection;
use tempfile::tempdir;

fn initialized() -> (tempfile::TempDir, Connection) {
    let directory = tempdir().unwrap();
    let path = directory.path().join("workspace.db");
    let connection = SchemaV2::initialize_workspace(&path, [1_u8; 16], [2_u8; 16], 10).unwrap();
    (directory, connection)
}

#[test]
fn measure_returns_non_negative_page_count_gt_zero() {
    let (_dir, connection) = initialized();
    let snap = QuotaV2::measure(&connection).unwrap();
    assert!(
        snap.page_count > 0,
        "page_count must be positive on initialized workspace"
    );
    assert!(snap.page_count >= 0);
    assert!(snap.freelist_count >= 0);
    assert!(snap.wal_pages >= 0);
    assert!(snap.db_bytes >= 0);
}

#[test]
fn admit_ok_under_generous_limits() {
    let (_dir, connection) = initialized();
    let snap = QuotaV2::measure(&connection).unwrap();
    let result = QuotaV2::admit(&connection, &snap, i64::MAX, i64::MAX);
    assert!(result.is_ok(), "admit should succeed under generous limits");
}

#[test]
fn admit_rejects_db_bytes_when_max_is_tiny() {
    let (_dir, connection) = initialized();
    let snap = QuotaV2::measure(&connection).unwrap();
    let result = QuotaV2::admit(&connection, &snap, 0, i64::MAX);
    assert!(matches!(result, Err(QuotaV2Error::DbBytes(_))));
}

#[test]
fn admit_rejects_wal_bytes_synthetic_snapshot() {
    let (_dir, connection) = initialized();
    let snap = QuotaSnapshot {
        page_count: 1,
        freelist_count: 0,
        wal_pages: 4,
        db_bytes: 4096,
    };
    let result = QuotaV2::admit(&connection, &snap, i64::MAX, 0);
    assert!(matches!(result, Err(QuotaV2Error::WalBytes(_))));
}

#[test]
fn reclaim_runs_without_error_message_count_unchanged() {
    let (_dir, mut connection) = initialized();
    let session_id = opencode_rk_contracts::SessionId::new();
    V2Writer::create_session(
        &mut connection,
        &NewSession {
            id: session_id,
            title: "test".to_owned(),
            created_at_us: 10,
            updated_at_us: 10,
        },
    )
    .unwrap();

    let count_before: i64 = connection
        .query_row("SELECT count(*) FROM messages", [], |row| row.get(0))
        .unwrap();

    let result = QuotaV2::reclaim(&connection, 3);
    assert!(result.is_ok(), "reclaim should run without error");

    let count_after: i64 = connection
        .query_row("SELECT count(*) FROM messages", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        count_before, count_after,
        "message row count must be unchanged after reclaim"
    );
}
