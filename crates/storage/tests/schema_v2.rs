//! Contracts for new format-2 files; the format-1 runtime is not switched here.
use rusqlite::{params, Connection};

const WORKSPACE_SQL: &str = include_str!("../schema/v2/workspace.sql");
const CATALOG_SQL: &str = include_str!("../schema/v2/catalog.sql");

fn workspace() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF;")
        .unwrap();
    db.execute_batch(WORKSPACE_SQL).unwrap();
    db.execute(
        "INSERT INTO workspace_state(id,workspace_id,cursor_epoch,format_version,created_at_us) \
         VALUES(1,?1,?2,2,0)",
        params![vec![1_u8; 16], vec![2_u8; 16]],
    )
    .unwrap();
    db
}

#[test]
fn both_new_file_schemas_load_without_foreign_key_errors() {
    let db = workspace();
    let errors: i64 = db
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(errors, 0);
    let catalog = Connection::open_in_memory().unwrap();
    catalog
        .execute_batch("PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF;")
        .unwrap();
    catalog.execute_batch(CATALOG_SQL).unwrap();
}

#[test]
fn payload_bytes_are_bounded_and_lengths_are_exact() {
    let db = workspace();
    let insert = "INSERT INTO payloads(inline_data,raw_bytes,created_at_us) VALUES(?1,?2,0)";
    db.execute(insert, params![vec![0_u8; 8192], 8192_i64])
        .unwrap();
    assert!(db
        .execute(insert, params![vec![0_u8; 8193], 8193_i64])
        .is_err());
    assert!(db.execute(insert, params![vec![0_u8; 2], 1_i64]).is_err());
}

#[test]
fn deleting_blob_cannot_gain_references_or_be_resurrected() {
    let db = workspace();
    db.execute(
        "INSERT INTO blobs(hash,state,codec,raw_bytes,stored_bytes,created_at_us) \
         VALUES(?1,0,0,9000,9052,0)",
        params![vec![7_u8; 32]],
    )
    .unwrap();
    let blob_pk = db.last_insert_rowid();
    db.execute("UPDATE blobs SET state=1 WHERE pk=?1", [blob_pk])
        .unwrap();
    assert!(db
        .execute(
            "INSERT INTO payloads(blob_pk,raw_bytes,created_at_us) VALUES(?1,9000,0)",
            [blob_pk],
        )
        .is_err());
    assert!(db
        .execute("UPDATE blobs SET state=0 WHERE pk=?1", [blob_pk])
        .is_err());
}

#[test]
fn referenced_blob_cannot_be_claimed_by_gc() {
    let db = workspace();
    db.execute(
        "INSERT INTO blobs(hash,state,codec,raw_bytes,stored_bytes,created_at_us) \
         VALUES(?1,0,0,9000,9052,0)",
        params![vec![8_u8; 32]],
    )
    .unwrap();
    let blob_pk = db.last_insert_rowid();
    db.execute(
        "INSERT INTO payloads(blob_pk,raw_bytes,created_at_us) VALUES(?1,9000,0)",
        [blob_pk],
    )
    .unwrap();
    assert!(db
        .execute("UPDATE blobs SET state=1 WHERE pk=?1", [blob_pk])
        .is_err());
    assert!(db.execute("DELETE FROM blobs WHERE pk=?1", [blob_pk]).is_err());
}

#[test]
fn immutable_payloads_cannot_change_after_publication() {
    let db = workspace();
    db.execute(
        "INSERT INTO payloads(inline_data,raw_bytes,created_at_us) VALUES(?1,2,0)",
        params![b"{}".to_vec()],
    )
    .unwrap();
    assert!(db
        .execute("UPDATE payloads SET inline_data=?1", params![b"[]".to_vec()])
        .is_err());
}

#[test]
fn state_outbox_and_cursor_roll_back_together() {
    let mut db = workspace();
    {
        let tx = db.transaction().unwrap();
        tx.execute(
            "INSERT INTO sessions(id,title,created_at_us,updated_at_us) VALUES(?1,'a',0,0)",
            params![vec![3_u8; 16]],
        )
        .unwrap();
        tx.execute("UPDATE workspace_state SET event_head_seq=1", [])
            .unwrap();
        tx.execute(
            "INSERT INTO event_outbox(seq,kind,payload_json,created_at_us) VALUES(1,'created','{}',0)",
            [],
        )
        .unwrap();
        tx.rollback().unwrap();
    }
    let counts: (i64, i64, i64) = db
        .query_row(
            "SELECT (SELECT count(*) FROM sessions), \
                    (SELECT count(*) FROM event_outbox),event_head_seq FROM workspace_state",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(counts, (0, 0, 0));
}
