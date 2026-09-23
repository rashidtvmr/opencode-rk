# WEB-009 branch structured activity RED

## Claim

- Task: `WEB-009`
- Session: `ses_f30db8445ffedcmUPVoqxoxNG7`
- Branch: `lane/WEB-009-branch-activity`
- Base: `3d10640`
- Owned test: `crates/sessions/tests/web009_branch_activity_red.rs`
- Owned production file: `crates/storage/src/writer_v2.rs`

## Source evidence

- `crates/storage/src/writer_v2.rs:16-17` - existing message-part kind constants `MESSAGE_PART_TEXT=0`, `MESSAGE_PART_REASONING_SUMMARY=1`
- `crates/storage/src/writer_v2.rs:34` - `V2Writer` struct, currently stateless
- `crates/storage/src/writer_v2.rs:129-211` - `insert_message` handles text + reasoning in one `TransactionBehavior::Immediate` transaction
- `crates/storage/src/writer_v2.rs:71-77` - `append_message_with_reasoning` delegates to `insert_message`
- `crates/storage/src/lib.rs:769-802` - crate-level `validate_assistant_activity(tool_calls, references)` validates counts, field sizes, JSON serialization bounds (64KB tools, 128KB references)
- `crates/storage/schema/v2/workspace.sql:100-107` - `message_parts.kind INTEGER NOT NULL CHECK(kind BETWEEN 0 AND 15)` allows kinds 0..15
- `crates/contracts/src/lib.rs:25-28` - `MAX_ASSISTANT_TOOL_CALLS=128`, `MAX_ASSISTANT_REFERENCES=64`
- `crates/contracts/src/lib.rs:245-257` - `AssistantToolCall` and `AssistantReference` structs with Serialize/Deserialize
- `crates/sessions/src/branch_v2.rs:228-232` - `synchronize_legacy_shadow` currently passes only reasoning to `V2Writer::append_message_with_reasoning`
- `crates/sessions/src/branch_v2.rs:296-338` - `list_assistant_activity` reads only reasoning parts, constructs empty tool_calls/references
- `crates/sessions/src/lib.rs:509-519` - `SessionService::append_assistant_with_activity` rejects non-empty tool/reference vectors for branch sessions
- `crates/storage/src/lib.rs:231-283` - legacy `Storage::append_message_with_activity` validates and atomically inserts message plus activity in one transaction (reference for v2)

## Observable contract

1. Add non-conflicting message-part kinds for assistant tool calls (kind 2) and references (kind 3) in `writer_v2.rs`.
2. Add `V2Writer::append_message_with_activity` API that accepts reasoning summary, `AssistantToolCall` slice, and `AssistantReference` slice, validates using crate-level `validate_assistant_activity` before any side effect, serializes bounded JSON payloads using existing storage conventions, and inserts message plus all present parts in one `TransactionBehavior::Immediate` transaction.
3. Reuse/refactor `insert_message` internals without changing current semantics for reasoning-only callers.

## Boundary (single-file)

The frozen tests in `web009_branch_activity_red.rs` also require changes to:
- `crates/sessions/src/branch_v2.rs` - `synchronize_legacy_shadow` must pass tool_calls/references to the new writer method; `list_assistant_activity` must read tool/reference parts
- `crates/sessions/src/lib.rs` - `SessionService::append_assistant_with_activity` must route branch sessions to a new `SessionManager::append_fork_assistant_with_activity` instead of rejecting them

These files are outside this lane's owned file scope (`writer_v2.rs` only). The v2 writer support can be implemented standalone; the callers will remain blocked until separate lanes update `branch_v2.rs` and `sessions/src/lib.rs`.

## Decisions

- New constants: `MESSAGE_PART_TOOL_CALLS = 2`, `MESSAGE_PART_REFERENCES = 3` (non-conflicting with 0=text, 1=reasoning_summary)
- New method: `V2Writer::append_message_with_activity` delegates to a new `insert_message_with_activity` function
- `insert_message_with_activity` extends `insert_message` pattern: validates upfront, starts Immediate transaction, inserts session/message/payloads/parts, commits atomically
- Tool calls and references stored as separate message_parts with JSON mime type, following existing payload conventions
- `validate_assistant_activity` is crate-private in lib.rs, accessible from writer_v2.rs submodule

## RED verification

Test SHA-256: `23f591fcb7d2990a60aabc23a9b5f9677e9e46974ce7a89fccc1db88551a0872` (matches frozen commit `4cfd87c`).

### Writer unit tests (owned file)

Command:
```text
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-storage --lib writer_v2 -- --test-threads=1
```

Result: 2/2 pass (checked_append_generous_budget_inserts_same_rows_as_plain, checked_append_zero_db_budget_rejects_and_writes_nothing).

### Frozen integration test (caller wiring outside lane scope)

Command:
```text
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-sessions --test web009_branch_activity_red -- --test-threads=1
```

Result: 1 pass, 2 failures (expected, caller wiring not in scope).

- `malformed_or_over_bound_child_activity_has_no_message_side_effect` -- PASS (validation works, no side effects)
- `legacy_assistant_activity_is_copied_to_branch_without_reexecution` -- FAIL: `list_assistant_activity` in `branch_v2.rs:296-336` returns empty `tool_calls`/`references`. Test line 97 asserts `child_activity[0].tool_calls == tool_calls` but left is `[]`.
- `child_activity_append_survives_manager_and_service_reopen` -- FAIL: `SessionService::append_assistant_with_activity` in `sessions/src/lib.rs` rejects branch sessions with `Contract("structured assistant activity is not yet available for branch sessions")`. Test line 142.

## Blocker

The two remaining failures require changes to `crates/sessions/src/branch_v2.rs` (`synchronize_legacy_shadow` at 228-232 must pass tool_calls/references; `list_assistant_activity` at 296-336 must read tool/reference parts) and `crates/sessions/src/lib.rs` (`SessionService::append_assistant_with_activity` at 492-542 must route branch sessions to a writer method instead of rejecting). These files are outside this lane's owned file scope (`writer_v2.rs` only). The writer v2 support is complete and self-contained; only caller wiring remains.
