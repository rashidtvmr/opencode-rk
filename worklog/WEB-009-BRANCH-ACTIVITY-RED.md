# WEB-009 branch structured activity RED

## Claim

- Task: `WEB-009`
- Session: `ses_f30db8445ffedcmUPVoqxoxNG7`
- Branch: `lane/WEB-009-branch-activity`
- Base: `3d10640`
- Owned test: `crates/sessions/tests/web009_branch_activity_red.rs`
- Owned production file: `crates/sessions/src/branch_v2.rs` (solo owned file for the session)

## Source evidence

- `crates/sessions/src/branch_v2.rs:147-248` - `synchronize_legacy_shadow` copies legacy messages into v2 workspace
- `crates/sessions/src/branch_v2.rs:230` - `synchronize_legacy_shadow` locates matching `AssistantActivity` entry by message_id
- `crates/sessions/src/lib.rs:509-519` - `SessionService::append_assistant_with_activity` rejects non-empty tool_calls/references for branch sessions with `Contract("structured assistant activity is not yet available for branch sessions")`
- `crates/storage/src/writer_v2.rs:83-97` - `V2Writer::append_message_with_activity` delegates to `insert_message_with_activity`
- `crates/storage/src/writer_v2.rs:254-263` - `insert_message_with_activity` binds `tool_calls_json` and `references_json` (both `&String`) as TEXT params into `payloads.inline_data BLOB` column in the STRICT table, causing `ConstraintViolation(3091)` at runtime
- `crates/storage/src/writer_v2.rs:16-21` - message-part kind constants: `MESSAGE_PART_TEXT=0`, `MESSAGE_PART_REASONING_SUMMARY=1`, `MESSAGE_PART_TOOL_CALLS=2`, `MESSAGE_PART_REFERENCES=3` (already existed and public)
- `crates/storage/src/writer_v2.rs:179` - `validate_assistant_activity` is private (crate-re-exported via `use crate::validate_assistant_activity`)
- `crates/contracts/src/lib.rs:25-28` - `MAX_ASSISTANT_TOOL_CALLS=128`, `MAX_ASSISTANT_REFERENCES=64`
- `crates/contracts/src/lib.rs:245-257` - `AssistantToolCall` and `AssistantReference` structs with Serialize/Deserialize

## Observable contract

1. `synchronize_legacy_shadow` copies reasoning + tool_calls + references from `AssistantActivity` entries to the v2 writer via `V2Writer::append_message_with_activity`.
2. `append_fork_assistant_with_activity` on `SessionManager` mirrors `append_fork_assistant_with_reasoning` semantics, delegating to `V2Writer::append_message_with_activity`.
3. `list_assistant_activity` decodes reasoning/tool/reference message parts, preserving assistant message IDs/order, failing closed on malformed JSON/payload. Empty activity messages are omitted. Duplicate part kinds (one reasoning/tool/reference part max per message) fail closed.

## Decisions (revised per session instructions)

- The local `insert_message_with_activity` workaround (which bound JSON as `&[u8]` to fix the BLOB bug) was REJECTED per session instructions.
- Both `synchronize_legacy_shadow` and `append_fork_assistant_with_activity` now call the landed `V2Writer::append_message_with_activity` directly.
- Removed: `MESSAGE_PART_TEXT`, `Transaction`, `TransactionBehavior` imports (no longer needed).
- Removed: local `insert_message_with_activity` method (including accidental duplicate `fn` line).
- Added: duplicate kind detection in `list_assistant_activity` (one reasoning/tool/reference part max per message, fail closed on duplicates).
- Added: empty activity filtering in `list_assistant_activity` (messages with no reasoning/tool_calls/references are omitted).
- Cannot edit `writer_v2.rs` (BLOB/TEXT type mismatch bug) or `lib.rs` (SessionService guard), per lane scope constraints.

## Verification

### rustfmt
```text
rtk rustfmt --edition 2021 crates/sessions/src/branch_v2.rs
```
Result: OK (no errors, no warnings).

### cargo check
```text
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo check -p opencode-rk-sessions
```
Result: OK (0 errors, 6 pre-existing warnings unrelated to changes).

### Frozen RED tests
```text
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-sessions --test web009_branch_activity_red -- --test-threads=1
```
Result: 1 passed, 2 failed.

- `malformed_or_over_bound_child_activity_has_no_message_side_effect` -- **PASS**: validation rejects malformed/over-bound activity before any write; no message side effects.
- `legacy_assistant_activity_is_copied_to_branch_without_reexecution` -- **FAIL**: `Storage(Sqlite(SqliteFailure(ConstraintViolation(3091), Some("cannot store TEXT value in BLOB column payloads.inline_data"))))`. Root cause: `V2Writer::append_message_with_activity` in `writer_v2.rs:254-263` binds `tool_calls_json`/`references_json` as `&String` (TEXT) into the STRICT `payloads.inline_data BLOB` column. The `synchronize_legacy_shadow` method correctly calls `V2Writer::append_message_with_activity` but the underlying writer has a type-mismatch bug that cannot be fixed within this lane's scope.
- `child_activity_append_survives_manager_and_service_reopen` -- **FAIL**: `SessionService::append_assistant_with_activity` in `sessions/src/lib.rs:509-519` rejects branch sessions with `Contract("structured assistant activity is not yet available for branch sessions")` before `SessionManager::append_fork_assistant_with_activity` is reached. This is a separate guard outside this lane's owned file scope.

## Blocker

Blocked on two upstream bugs outside lane scope:

1. **writer_v2.rs BLOB/TEXT binding** (line 254-263): `insert_message_with_activity` binds `serde_json::to_string` result (a `String`) as `&String` param into `payloads.inline_data BLOB` column in the STRICT workspace schema. SQLite rejects with `ConstraintViolation(3091)`. Must bind JSON payloads as `&[u8]` slices. Cannot fix writer_v2.rs (not owned by this lane).

2. **lib.rs SessionService guard** (line 509-519): `SessionService::append_assistant_with_activity` hard-rejects branch sessions with non-empty tool_calls/references. Cannot fix lib.rs (not owned by this lane).

This is a partial blocked commit, not completion. The branch_v2.rs changes (V2Writer delegation, list_assistant_activity decoder with duplicate/failure-closed/malformed guards, message filtering) are complete and self-contained, but integration cannot achieve GREEN until writer_v2.rs and lib.rs are fixed by their respective owners.

## Repair lane (writer_v2.rs BLOB fix)
- Session: `ses_f30a8238effeCQCXu3Yq25LsTJ`
- HEAD: `90f6b245b66f2fb8fabf35210b3965eee2263788`
- Fix: `crates/storage/src/writer_v2.rs` `insert_message_with_activity` now serializes tool_calls/references with `serde_json::to_vec` (bounded `Vec<u8>`) and binds `tool_calls_json.as_slice()` / `references_json.as_slice()` (BLOB) into STRICT `payloads.inline_data BLOB`. Upfront `validate_assistant_activity`, Immediate transaction, message ordering, reasoning behavior unchanged. `rustfmt --edition 2021 crates/storage/src/writer_v2.rs` applied (also normalized two import lines).
- Results:
  - storage writer filter: `cargo test -p opencode-rk-storage --test writer_v2 -- --test-threads=1` => 10 passed, 0 failed.
  - storage lib: `cargo test -p opencode-rk-storage --lib -- --test-threads=1` => 122 passed, 0 failed.
  - existing `web_activity_branch`: 1 passed, 0 failed.
  - frozen `web009_branch_activity_red`: 2 passed, 1 failed. `legacy_assistant_activity_is_copied_to_branch_without_reexecution` ok (BLOB fix). `malformed_or_over_bound_child_activity_has_no_message_side_effect` ok. Only `child_activity_append_survives_manager_and_service_reopen` fails at `crates/sessions/tests/web009_branch_activity_red.rs:142` with `Contract("structured assistant activity is not yet available for branch sessions")` from `crates/sessions/src/lib.rs:509-519` SessionService guard. Out of this lane scope (sole source file `writer_v2.rs`).
