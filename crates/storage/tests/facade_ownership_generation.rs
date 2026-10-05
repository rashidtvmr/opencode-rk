#![forbid(unsafe_code)]

use opencode_rk_storage::{ExecV2, NewSession, StorageError, facade::StorageFacade};
use rusqlite::{Connection, params};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tempfile::tempdir;

const CHILD_DB_ENV: &str = "OPENCODE_RK_FACADE_OWNER_TEST_DB";

fn state(facade: &StorageFacade) -> (i64, i64) {
    facade
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT owner_generation, clean_shutdown FROM workspace_state WHERE id=1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(StorageError::from)
        })
        .unwrap()
}

fn on_disk_state(path: &Path) -> (i64, i64) {
    let connection = Connection::open(path).unwrap();
    connection
        .query_row(
            "SELECT owner_generation, clean_shutdown FROM workspace_state WHERE id=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
}

fn database_artifacts(path: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut artifacts = BTreeMap::new();
    for suffix in ["", "-wal", "-shm"] {
        let mut name = OsString::from(path.as_os_str());
        name.push(suffix);
        let artifact = PathBuf::from(name);
        if artifact.exists() {
            artifacts.insert(artifact.clone(), fs::read(artifact).unwrap());
        }
    }
    artifacts
}

fn child_owner_denial(path: &Path) -> std::process::Output {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "child_facade_open_is_rejected_by_live_owner",
            "--nocapture",
        ])
        .env_clear()
        .env(CHILD_DB_ENV, path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let output = child.wait_with_output().unwrap();
            panic!("owned lock-probe child exceeded 10s: {output:?}");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn child_facade_open_is_rejected_by_live_owner() {
    let Ok(path) = std::env::var(CHILD_DB_ENV) else {
        return;
    };
    let error = match StorageFacade::open(Path::new(&path)) {
        Ok(facade) => {
            drop(facade);
            panic!("second process opened a database owned by another facade");
        }
        Err(error) => error,
    };
    assert!(
        matches!(error, StorageError::Io(ref error) if error.kind() == std::io::ErrorKind::WouldBlock),
        "expected immediate OS ownership-lock denial, got {error:?}"
    );
}

#[test]
fn same_process_second_owner_is_denied_without_database_mutation() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("workspace.db");
    let first = StorageFacade::open(&path).unwrap();
    assert_eq!(state(&first), (1, 0));
    let before = database_artifacts(&path);

    let error = match StorageFacade::open(&path) {
        Ok(facade) => {
            drop(facade);
            panic!("second facade opened the live database");
        }
        Err(error) => error,
    };
    assert!(
        matches!(error, StorageError::Io(ref error) if error.kind() == std::io::ErrorKind::WouldBlock),
        "expected immediate OS ownership-lock denial, got {error:?}"
    );
    assert_eq!(
        database_artifacts(&path),
        before,
        "rejected open touched DB bytes"
    );
    assert_eq!(
        state(&first),
        (1, 0),
        "rejected open changed startup markers"
    );

    drop(first);
    let second = StorageFacade::open(&path).unwrap();
    assert_eq!(state(&second), (2, 0));
}

#[test]
fn separate_process_second_owner_is_denied_without_database_mutation() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("workspace.db");
    let first = StorageFacade::open(&path).unwrap();
    let before = database_artifacts(&path);

    let output = child_owner_denial(&path);
    assert!(
        output.status.success(),
        "child lock probe failed: {output:?}"
    );
    assert_eq!(
        database_artifacts(&path),
        before,
        "rejected child open touched DB bytes"
    );
    assert_eq!(state(&first), (1, 0));
}

#[test]
fn fresh_and_clean_reopen_issue_monotonic_generation_and_shutdown_markers() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("workspace.db");

    let first = StorageFacade::open(&path).unwrap();
    assert_eq!(
        state(&first),
        (1, 0),
        "fresh owner must start dirty at generation 1"
    );
    let synchronous: i64 = first
        .with_connection(|connection| {
            connection
                .pragma_query_value(None, "synchronous", |row| row.get(0))
                .map_err(StorageError::from)
        })
        .unwrap();
    assert_eq!(
        synchronous, 2,
        "owner marker requires SQLite FULL synchronous"
    );
    first.close().unwrap();
    assert_eq!(on_disk_state(&path), (1, 1));

    let second = StorageFacade::open(&path).unwrap();
    assert_eq!(
        state(&second),
        (2, 0),
        "clean reopen must advance generation"
    );
    second.close().unwrap();
    assert_eq!(on_disk_state(&path), (2, 1));
}

#[test]
fn only_the_issued_generation_can_start_execution() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("workspace.db");
    let facade = StorageFacade::open(&path).unwrap();
    facade
        .create_session(&NewSession {
            id: opencode_rk_contracts::SessionId::new(),
            title: "generation fence".to_owned(),
            created_at_us: 1,
            updated_at_us: 1,
        })
        .unwrap();
    let (session_pk, config_payload_pk) = facade
        .with_connection(|connection| {
            let session_pk =
                connection.query_row("SELECT pk FROM sessions", [], |row| row.get(0))?;
            connection.execute(
                "INSERT INTO payloads (inline_data, raw_bytes, created_at_us) VALUES (?1, 3, 1)",
                params![b"cfg".as_slice()],
            )?;
            Ok((session_pk, connection.last_insert_rowid()))
        })
        .unwrap();
    let generation = state(&facade).0;

    facade
        .with_connection(|connection| {
            assert!(
                ExecV2::start_execution(
                    connection,
                    session_pk,
                    0,
                    generation,
                    config_payload_pk,
                    "fixture-provider",
                    "fixture-model",
                    2,
                )
                .is_ok()
            );
            assert!(
                ExecV2::start_execution(
                    connection,
                    session_pk,
                    0,
                    generation - 1,
                    config_payload_pk,
                    "fixture-provider",
                    "fixture-model",
                    3,
                )
                .is_err()
            );
            assert!(
                ExecV2::start_execution(
                    connection,
                    session_pk,
                    0,
                    generation + 1,
                    config_payload_pk,
                    "fixture-provider",
                    "fixture-model",
                    4,
                )
                .is_err()
            );
            let count: i64 =
                connection.query_row("SELECT COUNT(*) FROM executions", [], |row| row.get(0))?;
            assert_eq!(count, 1, "denied generations must create no execution rows");
            connection.execute("UPDATE executions SET state=5, finished_at_us=5", [])?;
            Ok(())
        })
        .unwrap();
    facade.close().unwrap();
}

#[test]
fn pending_execution_or_attempt_never_receives_clean_shutdown_marker() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("workspace.db");
    let facade = StorageFacade::open(&path).unwrap();
    facade
        .create_session(&NewSession {
            id: opencode_rk_contracts::SessionId::new(),
            title: "pending activity".to_owned(),
            created_at_us: 1,
            updated_at_us: 1,
        })
        .unwrap();
    let (session_pk, config_payload_pk) = facade
        .with_connection(|connection| {
            let session_pk =
                connection.query_row("SELECT pk FROM sessions", [], |row| row.get(0))?;
            connection.execute(
                "INSERT INTO payloads (inline_data, raw_bytes, created_at_us) VALUES (?1, 3, 1)",
                params![b"cfg".as_slice()],
            )?;
            Ok((session_pk, connection.last_insert_rowid()))
        })
        .unwrap();
    let generation = state(&facade).0;
    facade
        .with_connection(|connection| {
            let execution_pk = ExecV2::start_execution(
                connection,
                session_pk,
                0,
                generation,
                config_payload_pk,
                "fixture-provider",
                "fixture-model",
                2,
            )?;
            ExecV2::record_attempt(connection, execution_pk, 0, &[7; 32], 0, 3)?;
            Ok(())
        })
        .unwrap();

    assert!(
        facade.close().is_err(),
        "unresolved work must block clean shutdown"
    );
    assert_eq!(on_disk_state(&path), (1, 0));
}

#[test]
fn unknown_clean_marker_fails_closed_without_generation_or_marker_rewrite() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("workspace.db");
    let facade = StorageFacade::open(&path).unwrap();
    facade
        .with_connection(|connection| {
            connection.execute_batch("PRAGMA ignore_check_constraints=ON")?;
            connection.execute("UPDATE workspace_state SET clean_shutdown=7 WHERE id=1", [])?;
            Ok(())
        })
        .unwrap();
    drop(facade);

    assert!(
        StorageFacade::open(&path).is_err(),
        "unknown marker must fail closed"
    );
    assert_eq!(on_disk_state(&path), (1, 7));
}

#[test]
fn startup_issues_generation_and_dirty_marker_together_under_full() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("workspace.db");
    let facade = StorageFacade::open(&path).unwrap();
    facade.close().unwrap();
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER startup_marker_requires_generation
         BEFORE UPDATE OF clean_shutdown ON workspace_state
         WHEN NEW.clean_shutdown=0 AND NEW.owner_generation=OLD.owner_generation
         BEGIN SELECT RAISE(ABORT, 'dirty marker without issued generation'); END;
         CREATE TRIGGER startup_generation_requires_dirty_marker
         BEFORE UPDATE OF owner_generation ON workspace_state
         WHEN NEW.owner_generation=OLD.owner_generation+1 AND NEW.clean_shutdown<>0
         BEGIN SELECT RAISE(ABORT, 'generation without dirty marker'); END;",
        )
        .unwrap();
    drop(connection);

    let reopened = StorageFacade::open(&path).unwrap();
    assert_eq!(state(&reopened), (2, 0));
    let synchronous: i64 = reopened
        .with_connection(|connection| {
            connection
                .pragma_query_value(None, "synchronous", |row| row.get(0))
                .map_err(StorageError::from)
        })
        .unwrap();
    assert_eq!(synchronous, 2);
    drop(reopened);
    assert_eq!(on_disk_state(&path), (2, 0));
}

#[cfg(unix)]
fn session_titles(facade: &StorageFacade) -> Vec<String> {
    facade
        .with_connection(|connection| {
            let mut statement = connection.prepare("SELECT title FROM sessions ORDER BY pk")?;
            let mut titles = Vec::new();
            for title in statement.query_map([], |row| row.get::<_, String>(0))? {
                titles.push(title?);
            }
            Ok(titles)
        })
        .unwrap()
}

#[cfg(unix)]
fn assert_alias_rejected_without_mutation(
    facade: &StorageFacade,
    path: &Path,
    alias: &Path,
) {
    let before_state = state(facade);
    let before_titles = session_titles(facade);
    let before_artifacts = (database_artifacts(path), database_artifacts(alias));
    let denial = match StorageFacade::open(alias) {
        Ok(second_owner) => {
            drop(second_owner);
            None
        }
        Err(error) => Some(error),
    };
    let after_artifacts = (database_artifacts(path), database_artifacts(alias));
    let after_state = state(facade);
    let after_titles = session_titles(facade);
    let rejected_before_sqlite = matches!(
        &denial,
        Some(StorageError::Io(error))
            if matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::InvalidInput)
    );

    assert_eq!(after_state, before_state, "alias denial changed owner markers");
    assert_eq!(after_titles, before_titles, "alias denial changed database rows");
    assert_eq!(
        after_artifacts, before_artifacts,
        "alias denial changed database, WAL, or shared-memory artifacts"
    );
    assert!(
        rejected_before_sqlite,
        "alias must be denied before writable SQLite open, not by missing-WAL/SQLite failure: {denial:?}"
    );

    facade
        .create_session(&NewSession {
            id: opencode_rk_contracts::SessionId::new(),
            title: "original owner remains writable".to_owned(),
            created_at_us: 2,
            updated_at_us: 2,
        })
        .unwrap();
    let mut expected_titles = before_titles;
    expected_titles.push("original owner remains writable".to_owned());
    assert_eq!(
        session_titles(facade),
        expected_titles,
        "original owner stopped working after alias denial"
    );
}

#[cfg(unix)]
#[test]
fn hard_link_alias_is_rejected_before_second_writable_database_open() {
    use std::os::unix::fs::MetadataExt;

    let directory = tempdir().unwrap();
    let path = directory.path().join("workspace.db");
    let alias = directory.path().join("workspace-hardlink.db");
    {
        let seeded = StorageFacade::open(&path).unwrap();
        seeded
            .create_session(&NewSession {
                id: opencode_rk_contracts::SessionId::new(),
                title: "original owner data".to_owned(),
                created_at_us: 1,
                updated_at_us: 1,
            })
            .unwrap();
        seeded.close().unwrap();
    }

    fs::hard_link(&path, &alias).unwrap();
    let original = fs::metadata(&path).unwrap();
    let linked = fs::metadata(&alias).unwrap();
    assert_eq!(
        (original.dev(), original.ino()),
        (linked.dev(), linked.ino()),
        "fixture alias must name the same database inode"
    );
    assert!(original.nlink() >= 2);

    let facade = StorageFacade::open(&path).unwrap();
    let generation = state(&facade).0;
    assert_alias_rejected_without_mutation(&facade, &path, &alias);
    assert_eq!(state(&facade).0, generation);
    facade.close().unwrap();
}

#[cfg(unix)]
#[test]
fn symlink_alias_is_rejected_while_original_facade_owns_database() {
    use std::os::unix::fs::symlink;

    let directory = tempdir().unwrap();
    let path = directory.path().join("workspace.db");
    let alias = directory.path().join("workspace-symlink.db");
    let facade = StorageFacade::open(&path).unwrap();
    facade
        .create_session(&NewSession {
            id: opencode_rk_contracts::SessionId::new(),
            title: "original owner data".to_owned(),
            created_at_us: 1,
            updated_at_us: 1,
        })
        .unwrap();
    symlink(&path, &alias).unwrap();

    assert_alias_rejected_without_mutation(&facade, &path, &alias);
    facade.close().unwrap();
}
