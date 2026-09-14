# SESS-007

Status: PENDING. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: None.
Dependencies: crates/contracts provides base types (SessionSummary, Timestamp, SessionId).

## User-observable outcome

SessionArchive provides an in-memory collection for archived sessions with
metadata tracking (total_archived, total_messages, storage_bytes, retention_days).

## Source evidence

- SessionSummary, Timestamp, SessionId defined in crates/contracts/src/lib.rs.
- archive.rs is a new module declared in crates/sessions/src/lib.rs.

## Observable contract

- SessionArchive struct with sessions Vec<SessionSummary>, metadata: ArchiveMetadata,
  created_at: Timestamp.
- ArchiveMetadata with total_archived: u64, total_messages: u64, storage_bytes: u64,
  retention_days: u32.
- Methods: new(retention_days), add(session), list() -> &[SessionSummary],
  remove(id) -> Option<SessionSummary>, size_bytes() -> u64, metadata() -> ArchiveMetadata.
- 5 tests: new_creates_empty, add_appends, list_returns_all, remove_deletes, metadata_tracked.

## Test obligations

- SESS-007-T01: new_creates_empty - empty archive has zero metadata.
- SESS-007-T02: add_appends - sessions and metadata increment on add.
- SESS-007-T03: list_returns_all - list returns all added sessions in order.
- SESS-007-T04: remove_deletes - remove returns and deletes session; None for missing.
- SESS-007-T05: metadata_tracked - metadata reflects add/remove operations.

## Verification

```bash
cargo test -p opencode-rk-sessions
cargo check --workspace
```
