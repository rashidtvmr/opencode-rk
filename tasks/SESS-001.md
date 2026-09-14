# SESS-001

Status: COMPLETED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: None.
Dependencies: crates/contracts provides base types.

## User-observable outcome

Core session types and constants are defined in `crates/sessions/src/types.rs`.

## Source evidence

- SessionId/MessagId already exist in `crates/contracts/src/lib.rs:59-60`.
- Task specifies `[u8; 16]` array types for sessions crate's types module.

## Observable contract

- `SessionState` enum with Pending, Active, Paused, Archived, Deleted variants.
- `ArchivedSession` struct with session_id, archived_at, old_state, reason.
- `SessionMetadata` struct with id, title, position, parent_id, fork_depth, created_at, updated_at, archived_at, state.
- Constants MAX_SESSIONS=10000, MAX_MESSAGES=100000, PAGE_SIZE=100.
- Type aliases SessionId=[u8;16], MessageId=[u8;16], SessionTitle=String.

## Test obligations

- SESS-001-T01: session_state_serializes - SessionState variant serialization round-trips.
- SESS-001-T02: archived_session_fields - ArchivedSession field correctness.
- SESS-001-T03: metadata_accessors - SessionMetadata field accessors.
- SESS-001-T04: constants_valid - Constants have expected values.
- SESS-001-T05: session_id_from_uuid - UUID round-trip via From traits as [u8;16].

## Verification

```bash
cargo test -p opencode-rk-sessions
cargo check --workspace
```

Both commands pass with no errors.