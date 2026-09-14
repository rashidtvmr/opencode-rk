# SESS-014

Status: PENDING. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: None.
Dependencies: crates/contracts provides base types.

## User-observable outcome

Session archive functionality with compression in `crates/sessions/src/archive.rs`.

## Source evidence

- SessionId exists in `crates/contracts/src/lib.rs:59`.
- SessionState enum exists in `crates/contracts/src/lib.rs:139`.
- SessionSummary exists in `crates/contracts/src/lib.rs:147`.

## Observable contract

- `ArchiveEntry`: id SessionId, title, created_at, archived_at, reason.
- `SessionArchiveV2`: entries Vec<ArchiveEntry>, compression: Compression, max_age_days: u32.
- `Compression` enum: None, Gzip, Brotli.
- `add(session)`, `list(filter)`, `get(id)`, `remove(id)`, `compress()`, `extract_stats()` -> ArchiveStats.
- `ArchiveStats`: total_entries, total_size, compressed_size, compression_ratio, avg_age_days.

## Test obligations

- SESS-014-T01: add_entry - verifies adding a session to the archive.
- SESS-014-T02: list_filters_by_state - verifies listing with filters.
- SESS-014-T03: get_finds_by_id - verifies getting a session by ID.
- SESS-014-T04: remove_deletes - verifies removing a session.
- SESS-014-T05: compression_stats - verifies compression statistics.

## Verification

```bash
cargo test -p opencode-rk-sessions
cargo check --workspace
```

Both commands must pass with no errors.
