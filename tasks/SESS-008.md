# SESS-008

Status: IN PROGRESS. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: None.
Dependencies: crates/contracts provides SessionSummary, SessionId, Timestamp, SessionState.
Test obligations: SESS-008-T01..T05.

## User-observable outcome

An in-memory session store with dirty tracking and compaction, implemented in
`crates/sessions/src/persist.rs`.

## Source evidence

- `crates/contracts/src/lib.rs:146-154` - `SessionSummary` fields: id, title,
  state, created_at, updated_at, archived_at.
- `crates/contracts/src/lib.rs:59,62` - `SessionId` (Copy, Hash, Eq).
- `crates/sessions/src/store.rs` - existing SQLite-backed store used as the
  reference for dirty-tracking semantics.

## Observable contract

`PersistentSessionStore` fields: `sessions: Vec<SessionSummary>`,
`dirty: HashSet<SessionId>`, `flush_interval_ms: u64`.

Methods:

- `save(session)` - insert or replace a session, mark its id dirty.
- `load(id) -> Option<SessionSummary>` - return a clone of the stored session
  or None.
- `flush()` - clear the dirty set (write-through model for in-memory store).
- `compact()` - remove orphaned (still-dirty) sessions from the vector and
  dirty set; retain flushed sessions.
- `stats() -> (usize, usize)` - (total sessions, dirty count).
- `flush_interval_ms() -> u64` and accessor helpers.

## Test obligations

- SESS-008-T01: `save_and_load` - saved session is loadable and equals input.
- SESS-008-T02: `flush_clears_dirty` - after flush dirty is 0 and session
  survives.
- SESS-008-T03: `compact_removes_orphans` - only un-flushed (dirty) sessions
  are removed; flushed sessions remain.
- SESS-008-T04: `load_missing` - load of an unknown id returns None.
- SESS-008-T05: `stats_track` - totals/dirty counts update correctly across
  save, flush, and re-save.

## Verification

```bash
cargo test -p opencode-rk-sessions && cargo check --workspace
```

Both commands pass with no errors.
