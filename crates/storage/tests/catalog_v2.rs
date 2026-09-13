//! RED contracts for the format-2 installation catalog initializer.
use std::fs;

use opencode_rk_storage::catalog_v2::{CatalogV2, WorkspaceRegistration};
use rusqlite::{params, Connection};
use tempfile::{tempdir, TempDir};

const CATALOG_APPLICATION_ID: i64 = 1330791218; // 0x4F524332 = "ORC2"

fn pragma_i64(connection: &Connection, name: &str) -> i64 {
    connection
        .query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))
        .unwrap()
}

fn initialized() -> (TempDir, Connection) {
    let directory = tempdir().unwrap();
    let path = directory.path().join("catalog.db");
    let connection = CatalogV2::initialize_catalog(&path, [7_u8; 16], 10).unwrap();
    (directory, connection)
}

fn registration(id: [u8; 16], label: impl Into<String>, status: u8) -> WorkspaceRegistration {
    WorkspaceRegistration {
        id,
        label: label.into(),
        project_root: Some("/tmp/proj".to_owned()),
        status,
        created_at_us: 12,
    }
}

#[test]
fn initialize_new_catalog_applies_the_format_2_connection_contract() {
    let (_directory, connection) = initialized();

    assert_eq!(pragma_i64(&connection, "page_size"), 4096);
    assert_eq!(pragma_i64(&connection, "auto_vacuum"), 2);
    assert_eq!(
        pragma_i64(&connection, "application_id"),
        CATALOG_APPLICATION_ID
    );
    assert_eq!(pragma_i64(&connection, "user_version"), 2);
    assert_eq!(
        connection
            .query_row("PRAGMA journal_mode", [], |row| row
                .get::<_, String>(0))
            .unwrap()
            .to_ascii_lowercase(),
        "wal"
    );
    assert_eq!(pragma_i64(&connection, "foreign_keys"), 1);
    assert_eq!(pragma_i64(&connection, "synchronous"), 2);
    assert_eq!(pragma_i64(&connection, "trusted_schema"), 0);
    assert_eq!(pragma_i64(&connection, "temp_store"), 1);
    assert_eq!(pragma_i64(&connection, "busy_timeout"), 5000);

    let (id, installation_id, format_version): (i64, Vec<u8>, i64) = connection
        .query_row(
            "SELECT id, installation_id, format_version FROM catalog_state WHERE id=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!((id, installation_id, format_version), (1, vec![7; 16], 2));

    let (version, checksum, applied_at_us): (i64, Vec<u8>, i64) = connection
        .query_row(
            "SELECT version, checksum, applied_at_us FROM schema_migrations",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(version, 2);
    assert_eq!(checksum.len(), 32);
    assert_eq!(
        checksum.as_slice(),
        CatalogV2::catalog_checksum().as_slice()
    );
    assert_eq!(applied_at_us, 10);
}

#[test]
fn initialize_rejects_a_nonempty_file_without_modifying_it() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("not-a-catalog.db");
    let original = b"pre-existing bytes";
    fs::write(&path, original).unwrap();

    assert!(
        CatalogV2::initialize_catalog(&path, [7_u8; 16], 10).is_err(),
        "non-empty files must fail closed"
    );
    assert_eq!(fs::read(path).unwrap(), original);
}

#[test]
fn initialize_is_not_idempotent_and_open_existing_is_the_reopen_path() {
    let (directory, connection) = initialized();
    let path = directory.path().join("catalog.db");
    drop(connection);

    assert!(CatalogV2::initialize_catalog(&path, [7_u8; 16], 10).is_err());
    let reopened = CatalogV2::open_existing(&path).unwrap();
    assert_eq!(pragma_i64(&reopened, "user_version"), 2);
    assert_eq!(
        pragma_i64(&reopened, "application_id"),
        CATALOG_APPLICATION_ID
    );
}

#[test]
fn open_existing_rejects_a_tampered_migration_checksum() {
    let (directory, mut connection) = initialized();
    let path = directory.path().join("catalog.db");
    connection
        .execute(
            "UPDATE schema_migrations SET checksum=?1 WHERE version=2",
            params![vec![0_u8; 32]],
        )
        .unwrap();
    drop(connection);

    assert!(
        CatalogV2::open_existing(&path).is_err(),
        "checksum verification is a readiness gate"
    );
}

#[test]
fn register_workspace_rejects_an_overlong_label_without_inserting() {
    let (_directory, mut connection) = initialized();
    let invalid = registration(
        [1_u8; 16],
        String::from_utf8(vec![b'x'; 1025]).unwrap(),
        1,
    );

    assert!(CatalogV2::register_workspace(&mut connection, &invalid).is_err());
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM workspaces", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn register_workspace_rejects_an_invalid_status_without_inserting() {
    let (_directory, mut connection) = initialized();
    let invalid = registration([2_u8; 16], "bad-status", 4);

    assert!(CatalogV2::register_workspace(&mut connection, &invalid).is_err());
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM workspaces", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn register_workspace_rejects_an_overlong_project_root_without_inserting() {
    let (_directory, mut connection) = initialized();
    let mut invalid = registration([3_u8; 16], "big-root", 1);
    invalid.project_root = Some(String::from_utf8(vec![b'p'; 4097]).unwrap());

    assert!(CatalogV2::register_workspace(&mut connection, &invalid).is_err());
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM workspaces", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn register_and_list_round_trip_ordered_by_pk_with_limit_clamp() {
    let (_directory, mut connection) = initialized();
    let a = registration([1_u8; 16], "first", 1);
    let b = registration([2_u8; 16], "second", 1);
    let c = registration([3_u8; 16], "third", 1);

    CatalogV2::register_workspace(&mut connection, &a).unwrap();
    CatalogV2::register_workspace(&mut connection, &b).unwrap();
    CatalogV2::register_workspace(&mut connection, &c).unwrap();

    assert_eq!(
        CatalogV2::list_workspaces(&connection, 0).unwrap(),
        vec![a.clone()],
        "limit 0 clamps up to at least one row"
    );
    assert_eq!(
        CatalogV2::list_workspaces(&connection, usize::MAX).unwrap(),
        vec![a, b, c],
        "large limit returns all rows ordered by pk"
    );
}

#[test]
fn put_and_get_setting_round_trip_bumps_revision_from_one_to_two() {
    let (_directory, mut connection) = initialized();

    CatalogV2::put_setting(&mut connection, "theme", r#"{"dark":true}"#, 13).unwrap();
    assert_eq!(
        CatalogV2::get_setting(&connection, "theme").unwrap(),
        Some((r#"{"dark":true}"#.to_owned(), 1))
    );

    CatalogV2::put_setting(&mut connection, "theme", r#"{"dark":false}"#, 14).unwrap();
    assert_eq!(
        CatalogV2::get_setting(&connection, "theme").unwrap(),
        Some((r#"{"dark":false}"#.to_owned(), 2))
    );
}

#[test]
fn put_setting_rejects_bad_key_invalid_or_oversize_json() {
    let (_directory, mut connection) = initialized();

    assert!(CatalogV2::put_setting(&mut connection, "", r#"{}"#, 13).is_err());
    assert!(CatalogV2::put_setting(&mut connection, "a", "not-json", 13).is_err());
    let oversize = format!("\"{}\"", "x".repeat(4096));
    assert!(oversize.len() > 4096);
    assert!(CatalogV2::put_setting(&mut connection, "big", &oversize, 13).is_err());

    assert_eq!(
        CatalogV2::get_setting(&connection, "theme").unwrap(),
        None,
        "no setting was persisted by any rejected write"
    );
}

#[test]
fn get_setting_missing_key_returns_none() {
    let (_directory, connection) = initialized();
    assert_eq!(CatalogV2::get_setting(&connection, "missing").unwrap(), None);
}
