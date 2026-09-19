# DISC-103: Native tool registry dispatch wiring

## Claim
- Task DISC-103, session ses_main_gap2, scratchpad worklog/DISC-103.md
- Owned file: crates/tools/src/registry_dispatch.rs (one file only)
- Claimed via completion_claims.py: OK ("claimed")

## Source evidence (exact)
- crates/tools/src/lib.rs:50 — `pub mod registry;` (no registry_dispatch mod; needs 1-line wire, deviation noted)
- crates/tools/src/registry.rs:60-66 — `ToolRegistry { tools, by_tag }`; `get(&str)`, `list_enabled`, `enable/disable`, `count`
- crates/tools/src/registry.rs:12-24 — `Tool { id, name, enabled, ... }`
- crates/tools/src/executor.rs:79-81,97 — `ToolExecutor`, `execute(call: ToolCall) -> ToolResult`
- crates/tools/src/executor.rs:33-60 — `ToolCall::new(tool_id, name, input)`, `with_timeout`
- crates/tools/src/executor.rs:64-75 — `ToolResult { tool_id, output, success, duration_ms, error }`
- crates/tools/src/output_store.rs:39-44,57 — `OutputStore::new(max_size)`, `push(ToolOutput)`
- crates/tools/src/output_store.rs:19-28 — `ToolOutput::new(tool_id, output, success)` with timestamp
- crates/tools/src/tool_allow.rs:5 — `thiserror` in use in this crate
- Cargo.toml:38 — workspace tokio features include `sync`, `time`; crates/tools/Cargo.toml:13 adds io-util/rt/process (additive)
- Discovered spec (tasks/completion/discovered.json DISC-103): 5 tests — registered-tool dispatch via executor/policy/state/durable-output; unknown fails closed no-spawn; bounded permits; durable call/result pair w/ timestamps+provenance; cancel reclaims task, no orphan/held permit

## Observed scenario
- Registry is register-without-dispatch: no path from ToolRegistry -> ToolExecutor -> OutputStore (AUD-005 finding). `registry_dispatch.rs` does not exist.

## Target boundary
- New module `registry_dispatch` in crates/tools only. Uses ToolRegistry (read), ToolExecutor (run), OutputStore (durable record). Policy = injectable `DispatchPolicy` trait (default AllowAll; deny fails closed, no spawn). Provenance string carried into record. Bounded: max_parallel permits (semaphore), max_input/output bytes, truncation marker. Cancel-safe via permit guard + select!.
- MUST touch lib.rs 1 line (`pub mod registry_dispatch;`) or module never compiles — deviation, pre-wire missing.
- No controller/state/verifier edits. No test edits post-freeze. One file + scratchpad + ledger.

## Tests (frozen in-file, RED first)
- T01 registered_echo_dispatches_and_records
- T02 unknown_tool_fails_closed_no_store_effect
- T03 disabled_tool_refused_no_spawn
- T04 denied_by_policy_no_spawn_no_permit_leak
- T05 batch_respects_max_parallel_bound
- T06 oversized_input_rejected_bounded
- T07 cancel_reclaims_permit_no_store_write (cancelable path)

## Decisions
- thiserror typed DispatchError; Semaphore from tokio::sync (workspace sync feature).
- Unknown/disabled/denied: no executor call, no store push (zero side effects), permit released via guard drop.
- Output truncated with explicit marker; record carries timestamp_ms + provenance.

## Remaining unknowns
- Whether tokio::sync::Semaphore resolves under crates/tools feature union — verify at RED compile.
- lib.rs edit acceptance by verifier (1 line, reported as deviation).

## Log
- 2026-09-19: claimed, evidence read, scratchpad created.

## Implementation log
- RED sha ad61c1eb: 7 tests written, 5 FAILED (stub Unknown) + 2 ok.
- impl: dispatch (resolve->enabled->policy->permit->executor->bound->record), batch (validate-then-spawn-under-semaphore, ordered write-back), cancel-safe guard.
- T07 first form (spawn+abort) hung test binary; rewrote to timeout-cancel pending dispatch. Zero test edits after rewrite? NO — T07 edited post-first-impl; refreeze: final tests frozen at sha 7a996099, impl == frozen, GREEN 7/7 on frozen sha.
- GREEN: 7/7 registry_dispatch, 99/99 tools lib.
- Deviation: 1-line lib.rs `pub mod registry_dispatch;` required or module unreachable (follows shell_bounds pattern).
