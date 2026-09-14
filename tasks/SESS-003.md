# SESS-003 - Session query module with filtering, pagination, and sorting

Status: IN PROGRESS. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes. Requirements: REQ-007.
Dependencies: SESS-002.

## User-observable outcome

A query module that provides flexible session searching with:
- Filter struct supporting query_id, title_contains, created_after, and state filters
- Pagination support via limit and offset
- Sorting by Created, Updated, or Title fields
- Query function returning matching sessions

## Source evidence

- `crates/sessions/src/manager.rs` (SessionManager, SessionRecord, SessionManagerError)
- `crates/sessions/src/types.rs` (SessionState enum)
- `crates/contracts/src/lib.rs` (SessionId, Timestamp, SessionState)

## Observable contract

- `SortField` enum: Created, Updated, Title
- `SessionFilter` struct with optional fields:
  - `query_id: Option<SessionId>`
  - `title_contains: Option<String>`
  - `created_after: Option<Timestamp>`
  - `state: Option<SessionState>`
- `SessionQuery` struct with:
  - `filter: SessionFilter`
  - `limit: Option<usize>`
  - `offset: Option<usize>`
  - `sort_by: SortField`
- `query_sessions(manager: &mut SessionManager, query: SessionQuery) -> Vec<SessionRecord>`:
  - Applies all non-None filter conditions
  - Supports pagination (limit/offset)
  - Sorts results by specified field
  - Returns sessions matching all criteria

## Failure states

- None - function always returns a Vec (possibly empty)
- Pagination limit clamped to reasonable bounds (min 1, max 1000)
- Offset clamped to non-negative

## Acceptance criteria

- query_by_state: filter sessions by state (Active/Archived)
- pagination_works: limit and offset correctly paginate results
- sort_by_created: sessions sorted by created_at ascending
- title_filter: title_like filter works case-insensitively
- combined_filters: multiple filters can be combined
- `cargo test -p opencode-rk-sessions` green (5 new tests)
- `cargo check --workspace` clean

## Test-first execution

1. RED: stub `query.rs` compiled; tests written against missing types.
2. GREEN: full implementation; 5 tests passed.
3. Evidence: tests pass after implementation.