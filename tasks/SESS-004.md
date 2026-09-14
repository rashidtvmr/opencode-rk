# SESS-004

Status: COMPLETED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: None.
Dependencies: crates/contracts provides SessionSummary type, crates/storage provides schema.

## User-observable outcome

`PersistentSessionStore` in `crates/sessions/src/store.rs` provides SQLite-backed persistence for sessions with full CRUD operations.

## Source evidence

- `SessionRecord` is type alias for `SessionSummary` (lib.rs:13)
- `SessionSummary` defined in contracts/src/lib.rs:146-154 with fields: id, title, state, created_at, updated_at, archived_at
- `SessionState` enum in contracts with Active/Archived variants
- Sessions table schema in storage/schema/v2/workspace.sql:58-79
- V2Writer pattern used in storage/src/writer_v2.rs

## Observable contract

- `PersistentSessionStore` wraps rusqlite::Connection
- `new(conn)` - constructor from Connection
- `create(session)` - inserts SessionRecord, returns Result<(), SessionError>
- `fetch(id)` - gets Option<SessionRecord>
- `update(session)` - updates record, returns Result<(), SessionError>
- `delete(id)` - removes record, returns Result<bool, SessionError>
- `all_sessions()` - returns Vec<SessionRecord>
- `create_session_table(conn)` - migration creates sessions table if not exists
- `ensure_migrations(conn)` - ensures all migration tables exist

## Test obligations

- SESS-004-T01: create_and_fetch - create then fetch returns same session
- SESS-004-T02: update_persists - update changes are persisted
- SESS-004-T03: delete_removes - delete removes the session
- SESS-004-T04: all_sessions_returns_all - all_sessions returns all sessions
- SESS-004-T05: fetch_missing_returns_none - fetch of non-existent returns None

## Verification

```bash
cargo test -p opencode-rk-sessions
cargo check --workspace
```

Both commands pass with no errors.

## Notes

SessionRecord = SessionSummary (alias in lib.rs:13)
SessionSummary has: id (SessionId), title (String), state (SessionState), created_at (Timestamp), updated_at (Timestamp), archived_at (Option<Timestamp>)

## Completion

Implementation complete in `crates/sessions/src/store.rs`.

- `PersistentSessionStore` wraps rusqlite::Connection via `new(conn)`.
- `create(&session)` inserts using session's `created_at`/`updated_at`.
- `fetch(id)` returns `Option<SessionRecord>`.
- `update(&session)` updates record, returns `Err(NotFound)` if missing.
- `delete(id)` returns `Result<bool>` (true if deleted).
- `all_sessions()` returns `Vec<SessionRecord>`.
- `create_session_table(conn)` and `ensure_migrations(conn)` in `crates/sessions/src/migration.rs`.

Tests:
- SESS-004-T01: create_and_fetch - PASS
- SESS-004-T02: update_persists - PASS
- SESS-004-T03: delete_removes - PASS
- SESS-004-T04: all_sessions_returns_all - PASS
- SESS-004-T05: fetch_missing_returns_none - PASS

Verification:
```
cargo test -p opencode-rk-sessions  -- 25 tests passed
cargo check --workspace -- no errors
```