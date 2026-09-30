//! DB-021 registered test-author RED: format-1 additive typed tool-history
//! migration observed through the existing public `Storage::open` API plus raw
//! `rusqlite` reads.
//!
//! Scope: migration presence, atomic failure against an incompatible
//! pre-existing object, idempotent reopen, the format-1 `schema_version` marker
//! (stays `1`), the DDL byte-accounting contract, and legacy-row regression.
//! The real per-pair writer and multi-call replay ordering are NOT claimed here;
//! they belong to the DB-022 installed-binary blackbox lane. No typed row is
//! inserted to prove the writer.
//!
//! BOUNDARY: this file binds only the identifiers the approved contract fixes
//! (`APP012-INTEGRATION-CONTRACT.md` SHA-256
//! `9cb402c2f3ca3fc27990231d3c74f7db3318c974d64814691b3a14d8159d9b7a`, lines
//! 97-103: tables `tool_rounds`/`typed_tool_records`, keys
//! `(turn_message_id, round_ordinal)` and `(round_id, pair_index, kind)`). The
//! payload column shape and the feature-ledger columns are explicitly
//! "proposal terms to freeze" in that contract, so no invented column shape is
//! bound: byte accounting is asserted structurally against the landed DDL.
//! `worklog/DB-021.md` records the exact freeze decision for the integrator.
//!
//! Every assertion is expected to PASS once DB-019 lands a contract-compliant
//! migration; today it fails at runtime because the typed schema is missing
//! (compiling RED, never a compile error).
use std::path::Path;

use opencode_rk_contracts::{
    MessageId, MessageRecord, MessageRole, PayloadRef, SessionId, SessionState, SessionSummary,
    Timestamp,
};
use opencode_rk_storage::{Storage, StoragePaths};
use rusqlite::{params, Connection};
use tempfile::tempdir;

/// Table names fixed by the approved contract (APP012-INTEGRATION-CONTRACT.md
/// lines 97-99).
const TOOL_ROUNDS: &str = "tool_rounds";
const TYPED_RECORDS: &str = "typed_tool_records";
/// Feature-version migration marker table. The contract requires "a
/// feature-version migration marker" without fixing its name; every reviewed
/// storage proposal agrees on `feature_migrations`. The exact feature key and
/// version are an integrator freeze decision (see worklog/DB-021.md).
const LEDGER: &str = "feature_migrations";
/// Names that exist ONLY in the format-2 scripts (`schema/v2/*.sql`). They must
/// never be created by the format-1 bootstrap (docs/STORAGE.md:17-20).
const V2_ONLY_TABLES: &[&str] = &[
    "payloads",
    "message_parts",
    "session_inputs",
    "executions",
    "provider_attempts",
    "event_outbox",
    "workspace_state",
    "catalog_state",
];

fn paths_under(dir: &Path) -> StoragePaths {
    StoragePaths::under(dir.join("data"))
}

fn table_exists(connection: &Connection, name: &str) -> bool {
    connection
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
            params![name],
            |row| row.get::<_, i64>(0),
        )
        .map(|count| count > 0)
        .unwrap_or(false)
}

fn table_sql(connection: &Connection, name: &str) -> Option<String> {
    connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name=?1",
            params![name],
            |row| row.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten()
}

fn schema_version(connection: &Connection) -> String {
    connection
        .query_row(
            "SELECT value FROM schema_meta WHERE key='schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .expect("format-1 schema_meta marker must exist after Storage::open")
}

fn ledger_row_count(connection: &Connection) -> i64 {
    connection
        .query_row(&format!("SELECT count(*) FROM {LEDGER}"), [], |row| {
            row.get::<_, i64>(0)
        })
        .expect("feature-migration ledger must exist after Storage::open")
}

/// Unique index column sets for a table, including the implicit primary-key
/// index. Order-insensitive; each set is returned sorted.
fn unique_index_columns(connection: &Connection, table: &str) -> Vec<Vec<String>> {
    let mut statement = connection
        .prepare(&format!("PRAGMA index_list({table})"))
        .expect("index_list");
    let indexes: Vec<(String, i64)> = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
        })
        .expect("index_list rows")
        .filter_map(Result::ok)
        .filter(|(_, unique)| *unique != 0)
        .collect();
    drop(statement);

    let mut sets = Vec::new();
    for (name, _) in indexes {
        let mut info = connection
            .prepare(&format!("PRAGMA index_info('{name}')"))
            .expect("index_info");
        let mut columns: Vec<String> = info
            .query_map([], |row| row.get::<_, String>(2))
            .expect("index_info rows")
            .filter_map(Result::ok)
            .collect();
        drop(info);
        columns.sort();
        sets.push(columns);
    }
    sets
}

fn foreign_key_targets(connection: &Connection, table: &str) -> Vec<String> {
    let mut statement = connection
        .prepare(&format!("PRAGMA foreign_key_list({table})"))
        .expect("foreign_key_list");
    let targets: Vec<String> = statement
        .query_map([], |row| row.get::<_, String>(2))
        .expect("foreign_key_list rows")
        .filter_map(Result::ok)
        .collect();
    drop(statement);
    targets
}

fn create_session(storage: &Storage) -> SessionId {
    let id = SessionId::new();
    let now = Timestamp::now();
    storage
        .create_session(&SessionSummary {
            id,
            title: "DB-021 fixture".to_owned(),
            state: SessionState::Active,
            created_at: now,
            updated_at: now,
            archived_at: None,
        })
        .expect("create_session");
    id
}

fn append_message(
    storage: &Storage,
    session_id: SessionId,
    role: MessageRole,
    text: &str,
) -> MessageId {
    let id = MessageId::new();
    storage
        .append_message(&MessageRecord {
            id,
            session_id,
            role,
            body: PayloadRef::inline(text).expect("inline payload"),
            created_at: Timestamp::now(),
        })
        .expect("append_message");
    id
}

#[test]
fn fresh_open_creates_typed_schema_and_feature_ledger_and_keeps_format1() {
    // Contract: a feature-version ledger row plus the typed-history DDL land
    // through the existing format-1 bootstrap; schema_version stays 1 and v2
    // scripts are never applied (docs/STORAGE.md:17-20).
    let dir = tempdir().unwrap();
    let storage = Storage::open(paths_under(dir.path())).expect("Storage::open");
    let connection = Connection::open(paths_under(dir.path()).database).unwrap();

    assert_eq!(
        schema_version(&connection),
        "1",
        "format-1 marker preserved"
    );
    assert!(
        table_exists(&connection, TOOL_ROUNDS),
        "migration must create {TOOL_ROUNDS}: absent today (missing typed migration)"
    );
    assert!(
        table_exists(&connection, TYPED_RECORDS),
        "migration must create {TYPED_RECORDS}: absent today (missing typed migration)"
    );
    assert!(
        table_exists(&connection, LEDGER),
        "migration must create the feature-version ledger {LEDGER}"
    );
    assert_eq!(
        ledger_row_count(&connection),
        1,
        "exactly one additive feature migration must be recorded"
    );
    for name in V2_ONLY_TABLES {
        assert!(
            !table_exists(&connection, name),
            "format-2 table {name} must never be created over format 1"
        );
    }
    drop(connection);
    drop(storage);
}

#[test]
fn conflicting_preexisting_typed_records_object_fails_closed_atomically() {
    // Contract: an incompatible pre-existing object must abort the whole
    // feature migration; a plain CREATE TABLE IF NOT EXISTS accepting the wrong
    // shape is a violation.
    let dir = tempdir().unwrap();
    let paths = paths_under(dir.path());
    std::fs::create_dir_all(&paths.root).unwrap();
    {
        let raw = Connection::open(&paths.database).unwrap();
        raw.execute_batch("CREATE TABLE typed_tool_records(x TEXT);")
            .unwrap();
    }

    let opened = Storage::open(paths.clone());
    assert!(
        opened.is_err(),
        "Storage::open must fail against a wrong-shaped {TYPED_RECORDS}"
    );
    drop(opened);

    let connection = Connection::open(&paths.database).unwrap();
    assert!(
        !table_exists(&connection, TOOL_ROUNDS),
        "atomic rollback: {TOOL_ROUNDS} must not survive a failed feature migration"
    );
    assert!(
        !table_exists(&connection, LEDGER),
        "atomic rollback: {LEDGER} must not survive a failed feature migration"
    );
    drop(connection);
}

#[test]
fn conflicting_preexisting_tool_rounds_object_fails_closed_atomically() {
    let dir = tempdir().unwrap();
    let paths = paths_under(dir.path());
    std::fs::create_dir_all(&paths.root).unwrap();
    {
        let raw = Connection::open(&paths.database).unwrap();
        raw.execute_batch("CREATE TABLE tool_rounds(x TEXT);")
            .unwrap();
    }

    let opened = Storage::open(paths.clone());
    assert!(
        opened.is_err(),
        "Storage::open must fail against a wrong-shaped {TOOL_ROUNDS}"
    );
    drop(opened);

    let connection = Connection::open(&paths.database).unwrap();
    assert!(
        !table_exists(&connection, TYPED_RECORDS),
        "atomic rollback: {TYPED_RECORDS} must not survive a failed feature migration"
    );
    assert!(
        !table_exists(&connection, LEDGER),
        "atomic rollback: {LEDGER} must not survive a failed feature migration"
    );
    drop(connection);
}

#[test]
fn reopen_is_idempotent_and_preserves_marker_and_schema() {
    let dir = tempdir().unwrap();
    let paths = paths_under(dir.path());

    let first_version;
    let first_rounds_sql;
    let first_records_sql;
    let first_ledger;
    {
        let storage = Storage::open(paths.clone()).expect("first Storage::open");
        let connection = Connection::open(&paths.database).unwrap();
        assert!(table_exists(&connection, TOOL_ROUNDS));
        assert!(table_exists(&connection, TYPED_RECORDS));
        first_version = schema_version(&connection);
        first_rounds_sql = table_sql(&connection, TOOL_ROUNDS).expect("tool_rounds sql");
        first_records_sql = table_sql(&connection, TYPED_RECORDS).expect("typed sql");
        first_ledger = ledger_row_count(&connection);
        drop(connection);
        drop(storage);
    }

    let storage = Storage::open(paths.clone()).expect("idempotent second Storage::open");
    let connection = Connection::open(&paths.database).unwrap();
    assert_eq!(schema_version(&connection), "1");
    assert_eq!(schema_version(&connection), first_version);
    assert_eq!(
        table_sql(&connection, TOOL_ROUNDS).as_deref(),
        Some(first_rounds_sql.as_str())
    );
    assert_eq!(
        table_sql(&connection, TYPED_RECORDS).as_deref(),
        Some(first_records_sql.as_str())
    );
    assert_eq!(
        ledger_row_count(&connection),
        first_ledger,
        "reopen must not duplicate the feature ledger row"
    );
    for name in V2_ONLY_TABLES {
        assert!(!table_exists(&connection, name), "v2 {name} never applied");
    }
    drop(connection);
    drop(storage);
}

#[test]
fn fixed_keying_and_foreign_keys_are_present() {
    let dir = tempdir().unwrap();
    let storage = Storage::open(paths_under(dir.path())).expect("Storage::open");
    let connection = Connection::open(paths_under(dir.path()).database).unwrap();

    assert!(
        table_exists(&connection, TOOL_ROUNDS),
        "missing typed migration"
    );
    assert!(
        table_exists(&connection, TYPED_RECORDS),
        "missing typed migration"
    );

    let mut round_keys: Vec<String> = vec![
        "round_id".to_owned(),
        "turn_message_id".to_owned(),
        "round_ordinal".to_owned(),
    ];
    round_keys.sort();
    assert!(
        unique_index_columns(&connection, TOOL_ROUNDS).contains(&round_keys),
        "tool_rounds must be unique on (turn_message_id, round_ordinal), got {:?}",
        unique_index_columns(&connection, TOOL_ROUNDS)
    );

    let mut record_key: Vec<String> = vec![
        "round_id".to_owned(),
        "pair_index".to_owned(),
        "kind".to_owned(),
    ];
    record_key.sort();
    assert!(
        unique_index_columns(&connection, TYPED_RECORDS).contains(&record_key),
        "typed_tool_records keyed by (round_id, pair_index, kind), got {:?}",
        unique_index_columns(&connection, TYPED_RECORDS)
    );

    let round_targets = foreign_key_targets(&connection, TOOL_ROUNDS);
    assert!(
        round_targets.iter().any(|t| t == "sessions"),
        "tool_rounds -> sessions FK"
    );
    assert!(
        round_targets.iter().any(|t| t == "messages"),
        "tool_rounds -> messages FK"
    );

    let record_targets = foreign_key_targets(&connection, TYPED_RECORDS);
    assert!(
        record_targets.iter().any(|t| t == "tool_rounds"),
        "typed -> tool_rounds FK"
    );
    assert!(
        record_targets.iter().any(|t| t == "messages"),
        "typed -> messages FK"
    );

    drop(connection);
    drop(storage);
}

#[test]
fn typed_ddl_uses_blob_byte_accounting_and_has_no_512_call_id_cap() {
    // Contract: SQLite checks use `length(CAST(x AS BLOB))` and no arbitrary
    // 512-byte call-ID cap is introduced (the storage-contract-final DDL's
    // `BETWEEN 1 AND 512` conflicts with the approved contract and the card).
    let dir = tempdir().unwrap();
    let storage = Storage::open(paths_under(dir.path())).expect("Storage::open");
    let connection = Connection::open(paths_under(dir.path()).database).unwrap();

    let sql = table_sql(&connection, TYPED_RECORDS)
        .expect("typed_tool_records DDL must exist (missing typed migration)");
    let normalized: String = sql
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        normalized.contains("cast(") && normalized.contains("as blob"),
        "byte accounting must use length(CAST(x AS BLOB)); got {sql}"
    );
    assert!(
        !normalized.contains("512"),
        "no arbitrary 512-byte call-ID cap is permitted; got {sql}"
    );

    drop(connection);
    drop(storage);
}

#[test]
fn legacy_rows_survive_migrated_reopen_byte_identical() {
    let dir = tempdir().unwrap();
    let paths = paths_under(dir.path());
    let tool_text = "[read] legacy tool output";
    let reasoning = "legacy reasoning summary";
    let attachment_bytes: &[u8] = b"legacy attachment bytes";
    let (session_id, tool_id, assistant_id, attachment_id, tool_len);

    {
        let storage = Storage::open(paths.clone()).expect("Storage::open");
        session_id = create_session(&storage);
        tool_id = append_message(&storage, session_id, MessageRole::Tool, tool_text);
        assistant_id = MessageId::new();
        storage
            .append_message_with_reasoning(
                &MessageRecord {
                    id: assistant_id,
                    session_id,
                    role: MessageRole::Assistant,
                    body: PayloadRef::inline("legacy assistant").unwrap(),
                    created_at: Timestamp::now(),
                },
                Some(reasoning),
            )
            .expect("append_message_with_reasoning");
        let attachment = storage
            .create_draft_attachment(session_id, "legacy.txt", "text/plain", attachment_bytes)
            .expect("create_draft_attachment");
        attachment_id = attachment.id;
        drop(storage);
        let raw = Connection::open(&paths.database).unwrap();
        let len: i64 = raw
            .query_row(
                "SELECT byte_len FROM messages WHERE id=?1",
                params![tool_id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        tool_len = len;
        drop(raw);
    }

    let storage = Storage::open(paths.clone()).expect("migrated reopen");
    let raw = Connection::open(&paths.database).unwrap();
    let (role, inline, byte_len, created): (String, Option<String>, i64, String) = raw
        .query_row(
            "SELECT role,inline_text,byte_len,created_at FROM messages WHERE id=?1",
            params![tool_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(role, "tool");
    assert_eq!(inline.as_deref(), Some(tool_text));
    assert_eq!(byte_len as u64, tool_text.len() as u64);

    let len: i64 = raw
        .query_row(
            "SELECT byte_len FROM messages WHERE id=?1",
            params![tool_id.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(len, tool_len);

    let (att_name, att_mime, att_len): (String, String, i64) = raw
        .query_row(
            "SELECT name,mime,byte_len FROM draft_attachments WHERE id=?1",
            params![attachment_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(att_name, "legacy.txt");
    assert_eq!(att_mime, "text/plain");
    assert_eq!(att_len as usize, attachment_bytes.len());

    let summary: String = raw
        .query_row(
            "SELECT reasoning_summary FROM message_activity WHERE message_id=?1",
            params![assistant_id.to_string()],
            |row| row.get(0),
        )
        .expect("reasoning summary row preserved");
    assert_eq!(summary, reasoning);
    drop(raw);

    assert_eq!(
        storage.get_message(session_id, tool_id).unwrap().body,
        PayloadRef::Inline {
            text: tool_text.to_owned()
        }
    );
    assert_eq!(storage.list_draft_attachments(session_id).unwrap().len(), 1);
    assert_eq!(
        storage
            .list_assistant_activity(session_id, 10)
            .unwrap()
            .len(),
        1
    );
    drop(storage);
}
