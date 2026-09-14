# SESS-011: PersistentSessionStore

## Summary
Implement `PersistentSessionStore` in `crates/sessions/src/persist.rs` as an in-memory
session cache with dirty tracking and async flush to a `Storage` backend.

## Requirements
- `PersistentSessionStore` struct with fields:
  - `sessions: Vec<SessionSummary>`
  - `dirty: HashSet<SessionId>`
  - `flush_interval_ms: u64`
- Methods:
  - `new() -> Self` - creates an empty store with default flush interval
  - `with_flush_interval(ms: u64) -> Self` - creates store with custom flush interval
  - `save(&mut self, session: SessionSummary)` - adds/updates session, marks dirty
  - `load(&self, id: SessionId) -> Option<SessionSummary>` - returns session by id if present
  - `flush(&mut self) -> Result<(), PersistentSessionStoreError>` - clears dirty set
  - `compact(&mut self) -> Result<(), PersistentSessionStoreError>` - removes orphaned sessions
  - `stats(&self) -> (usize, usize)` - returns (total_sessions, dirty_count)
- `PersistentSessionStoreError` enum with variants:
  - `FlushError(String)` - flush operation failed
  - `LoadError(String)` - load operation failed

- 5 tests: save_and_load, flush_clears_dirty, compact_removes_orphans, load_missing, stats_track

## Design Notes
- In-memory store; flush/compact are stubbed for now (clear dirty set / no-op removal)
  but return Result types to accommodate future integration with a Storage backend.
- `compact` removes sessions that are no longer referenced (simplified: removes archived
  sessions from the in-memory list as "orphans").

## Owned File
`crates/sessions/src/persist.rs`

## Verification
- `cargo test -p opencode-rk-sessions`
- `cargo check --workspace`
