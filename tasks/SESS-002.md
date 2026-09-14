# SESS-002 - Session manager for CRUD operations

Status: COMPLETE. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-006.
Dependencies: none.

## User-observable outcome

In-memory session manager that supports create, get, list, delete, and
update_title operations on sessions. Each session holds a title, timestamps,
and a message list. A typed error is returned when a session is not found.

## Source evidence

- `crates/contracts/src/lib.rs` (SessionId, MessageRecord, Timestamp,
  SessionState, SessionSummary, uuid_id! macro).
- `crates/sessions/Cargo.toml` (chrono, thiserror, rusqlite, uuid workspace deps).

## Observable contract

- `SessionManager` struct with internal `HashMap<SessionId, SessionRecord>`
  and `next_id: u64` counter.
- `SessionRecord`: `id`, `title`, `created_at`, `updated_at`, `messages`.
- `new()` - empty manager.
- `create(title: &str) -> SessionId` - inserts and returns ID.
- `get(id) -> Option<&SessionRecord>` - lookup by ID.
- `delete(id) -> bool` - removes; false if missing.
- `list() -> Vec<&SessionRecord>` - all sessions.
- `update_title(id, title) -> bool` - renames; false if missing.
- `SessionManagerError::NotFound(SessionId)` typed error variant.

## Failure states

- `delete` / `update_title` on a missing session returns `false`.
- No unbounded queue or retained output; manager owns a single HashMap.

## Acceptance criteria

- create_and_get green.
- delete_removes green.
- list_returns_all green.
- update_title_works green.
- delete_missing_returns_false green.
- `cargo test -p opencode-rk-sessions` green (8 tests); `cargo check --workspace` clean.

## Test-first execution

1. RED: stub `manager.rs` compiled; tests written against missing types.
2. GREEN: full implementation; 8 passed (5 new + 3 pre-existing).
3. Evidence: tests pass after implementation.
