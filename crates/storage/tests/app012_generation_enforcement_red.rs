//! OWN-GENERATION-RED: writable execution ownership is fenced by the current
//! workspace generation. A caller from either an older or unissued generation
//! must fail closed and create no execution row.
#![forbid(unsafe_code)]

use opencode_rk_storage::{ExecV2, SchemaV2};
use rusqlite::{params, Connection};

const CURRENT_GENERATION: i64 = 7;

struct Fixture {
    _directory: tempfile::TempDir,
    connection: Connection,
    session_pk: i64,
    config_payload_pk: i64,
}

fn fixture() -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let connection = SchemaV2::initialize_workspace(
        &directory.path().join("workspace.db"),
        [1_u8; 16],
        [2_u8; 16],
        0,
    )
    .unwrap();
    connection
        .execute(
            "UPDATE workspace_state SET owner_generation = ?1 WHERE id = 1",
            [CURRENT_GENERATION],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO sessions (id, title, created_at_us, updated_at_us)
             VALUES (randomblob(16), 'generation-fence', 1, 1)",
            [],
        )
        .unwrap();
    let session_pk = connection.last_insert_rowid();
    connection
        .execute(
            "INSERT INTO payloads (inline_data, raw_bytes, created_at_us)
             VALUES (?1, ?2, ?3)",
            params![b"cfg".as_slice(), 3_i64, 1_i64],
        )
        .unwrap();
    let config_payload_pk = connection.last_insert_rowid();
    Fixture {
        _directory: directory,
        connection,
        session_pk,
        config_payload_pk,
    }
}

fn start(fixture: &Fixture, generation: i64) -> Result<i64, opencode_rk_storage::StorageError> {
    ExecV2::start_execution(
        &fixture.connection,
        fixture.session_pk,
        0,
        generation,
        fixture.config_payload_pk,
        "fixture-provider",
        "fixture-model",
        10,
    )
}

fn execution_count(connection: &Connection) -> i64 {
    connection
        .query_row("SELECT COUNT(*) FROM executions", [], |row| row.get(0))
        .unwrap()
}

#[test]
fn current_workspace_generation_can_start_execution() {
    let fixture = fixture();
    assert!(start(&fixture, CURRENT_GENERATION).is_ok());
    assert_eq!(execution_count(&fixture.connection), 1);
}

#[test]
fn stale_workspace_generation_is_denied_without_execution_row() {
    let fixture = fixture();
    assert!(
        start(&fixture, CURRENT_GENERATION - 1).is_err(),
        "an earlier daemon generation must be fenced before inserting"
    );
    assert_eq!(
        execution_count(&fixture.connection),
        0,
        "generation denial must leave no execution side effect"
    );
}

#[test]
fn unissued_future_generation_is_denied_without_execution_row() {
    let fixture = fixture();
    assert!(
        start(&fixture, CURRENT_GENERATION + 1).is_err(),
        "a caller cannot self-issue future workspace authority"
    );
    assert_eq!(
        execution_count(&fixture.connection),
        0,
        "generation denial must leave no execution side effect"
    );
}
