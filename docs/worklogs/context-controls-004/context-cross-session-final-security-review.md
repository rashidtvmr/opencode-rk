# Final cross-session storage security review

Verdict: APPROVE

## Exact source/test hashes

Python SHA-256 computed for the reviewed candidate:

- `crates/storage/src/lib.rs`: `f64ae295a3d9fa901a720c3fbc12493b8ad39c6c10fe7851d5e9798fd6d1edf1`
- `crates/storage/src/session_context.rs`: `feb3ca21b4b10d8389e43bb3da7565c92975ed7b94a1ecd1c88860dbdc02ea6d`
- `crates/storage/tests/context_cross_session_integrity.rs`: `34f2201db9f97da5d61ab91260488ab5a8c417678eec0ca951ed497809353f21`
- `crates/storage/tests/context_cross_session_display.rs`: `1a86735090d8f3a15adc107c3775bb2d1c6a4aa7ed5a592ff1d8d821b535188f`
- `crates/storage/tests/context_index_integrity.rs`: `6b483e226a3dcdea99f43674c7bc3b92a9258fa3af99cabe635f79e04633c4ea` (post-approval diagnostic formatting only; expected `TypedHistorySchema` assertion unchanged)

## Findings resolved

- `display_tool_history` validates and reads payload within one deferred read transaction/snapshot (`lib.rs:907-910`).
- Migration validates `typed_tool_message_idx` before and after creation, table/non-unique/order/columns via `validate_typed_index`; schema check now preserves column order (`lib.rs:1665-1685, 1831-1868`).
- Q4 round lookup no longer forces session index. Exact SQL extracted from `session_context.rs`, executed with Python `sqlite3` against minimal source-matching schema, keys and indexes. `EXPLAIN QUERY PLAN`:
  - `SEARCH display USING COVERING INDEX messages_session_idx (session_id=?)`
  - `SEARCH t USING COVERING INDEX typed_tool_message_idx (message_id=?)`
  - `SEARCH r USING INDEX sqlite_autoindex_tool_rounds_1 (round_id=?)`
  No global `SCAN r/t`.

Source review confirms malformed ownership in either session endpoint returns `TypedHistoryIdentity` before bounded history, display history, or preview payload output. The index-integrity test's post-approval edit only formats error diagnostics; its `TypedHistorySchema` assertion remains exact. Parent execution evidence `context-cross-session-final-execution.log` reports 143 passed, 0 failed across 8 storage targets, including all three cross-session/index test targets. Source approval and supplied test evidence only; no integrated or installed acceptance claim.

## Strategy-review diagnosis

After three candidate cycles, the materially different final approach repairs the same cross-session failure: per-session indexed ownership probes, validated declared index schema/access path, and a shared display-reader snapshot. Prior negative review artifacts remain unchanged; their specific concerns are addressed here, not erased. No concrete remaining scoped source blocker found.
