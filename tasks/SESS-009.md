# SESS-009: In-memory SessionIndex for fast lookups

## Overview

Implement a `SessionIndex` data structure in `crates/sessions/src/index.rs` that provides O(1) lookup by session id, title, and state over `SessionSummary` records.

## Scope

- **File**: `crates/sessions/src/index.rs` (owned file)
- **Modifications**: `crates/sessions/src/lib.rs` (add `pub mod index;`)

## Goal

Provide an in-memory secondary index for session summaries used by query and list operations.

## Deliverable

`SessionIndex` struct with:

- Fields:
  - `by_id: HashMap<SessionId, usize>` - maps session id to index position
  - `by_title: HashMap<String, Vec<usize>>` - maps title to list of index positions
  - `by_state: HashMap<SessionState, Vec<usize>>` - maps state to list of index positions

- Methods:
  - `new() -> Self` - constructor
  - `index(&SessionSummary)` - adds/replaces a session in the index
  - `get(id: SessionId) -> Option<&SessionSummary>` - lookup by id
  - `find_by_title(title: &str) -> Vec<&SessionSummary>` - lookup by title
  - `find_by_state(state: SessionState) -> Vec<&SessionSummary>` - lookup by state
  - `rebuild()` - rebuild all indices from internal session list
  - `clear()` - remove all entries
  - `stats() -> (usize, usize, usize)` - return counts for by_id, by_title, by_state

## Tests

1. `index_and_get` - index sessions and retrieve by id
2. `find_by_title` - index sessions with shared titles, retrieve by title
3. `find_by_state` - index sessions with different states, retrieve by state
4. `rebuild` - mutate internal state, rebuild, verify lookups work
5. `stats_correct` - verify stats() returns correct counts after indexing

## Verification

```bash
cargo test -p opencode-rk-sessions
cargo check --workspace
```
