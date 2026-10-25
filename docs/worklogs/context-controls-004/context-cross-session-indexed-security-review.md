# Cross-session indexed security review

Verdict: NEEDS_FIX

## Exact candidate hashes

- `crates/storage/src/lib.rs`: `16276aae18f305990f53893418788f2bc5bf913cacecf10a2ac28d2329034854`
- `crates/storage/src/session_context.rs`: `17d82aa6ae7aa7af2681d5132bcad46527c4e8504f2371c0763e5a265a7e5efd`
- `crates/storage/tests/context_cross_session_integrity.rs`: `34f2201db9f97da5d61ab91260488ab5a8c417678eec0ca951ed497809353f21`
- `crates/storage/tests/context_cross_session_display.rs`: `1a86735090d8f3a15adc107c3775bb2d1c6a4aa7ed5a592ff1d8d821b535188f`

## Query-plan verification

Extracted all four exact `invalid_queries` Rust SQL strings from `session_context.rs`. Python `sqlite3` `:memory:` schema used `messages_session_idx`, `tool_rounds_session_idx`, `typed_tool_round_idx`, and the declared new `typed_tool_message_idx(message_id,round_id)`, plus actual table columns/primary keys/unique turn index from `lib.rs`.

All four queries used indexed session seeds, with no global round/typed-record scan:

1. `SEARCH r USING INDEX tool_rounds_session_idx (session_id=?)`; turn lookup via message PK.
2. `SEARCH turn USING COVERING INDEX messages_session_idx (session_id=?)`; round lookup via unique turn index.
3. Session-indexed rounds then `SEARCH t USING INDEX typed_tool_round_idx (round_id=?)`; display lookup via message PK.
4. `SEARCH display USING COVERING INDEX messages_session_idx (session_id=?)`; `SEARCH t USING COVERING INDEX typed_tool_message_idx (message_id=?)`; round lookup via session index.

Replacing the declared message index with an existing same-name wrong-column index reproduced `SCAN t USING INDEX typed_tool_message_idx` for query 4. `validate_typed_index` is never called for this new index; `CREATE INDEX IF NOT EXISTS` preserves a pre-existing wrong index. Validate its ownership, uniqueness flag, and columns at migration time. On the correct schema, Q4's final `SEARCH r USING ... tool_rounds_session_idx (session_id=?)` does not constrain `round_id` in that index (the `created_at` column intervenes); per typed row it may search all rounds in the requested session, giving O(T*R) metadata work for T typed records and R rounds in one session.

## Blocker

`display_tool_history` now calls `validate_typed_ownership` before selecting payload, but does not open a read transaction around validation plus payload query (`lib.rs`, `display_tool_history`). Each validation `SELECT` can finish its autocommit snapshot before the subsequent payload `SELECT`. A concurrent independent SQLite writer can change a validated round/display link between those operations; the read then emits payload from the new malformed state. Keep validation and payload selection on one deferred read transaction/snapshot, as the preview reader does.

No Cargo/build/test commands. No product or test files modified. Prior `context-cross-session-security-review.md` NEEDS_FIX remains preserved; no approval granted.
