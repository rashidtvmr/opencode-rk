# TOOL-018 scratchpad

Claim: TOOL-018 / ses_f384fedaeffenNesgxTRphfrXU / worklog/TOOL-018.md (ledger in-progress).
Owned file: crates/tools/src/mcp_lifecycle.rs only. Frozen tests: crates/tools/tests/mcp_lifecycle.rs (sha256 2fbaca626ae6a38419ec101573d4a8ff9da9f1d210f0c0f1692ae95b0c71b257). Zero test edits.

## Source evidence
- Task card: tasks/TOOL-018.md (lifecycle states, start-stop/retry, persist/restore, bounds).
- Owned impl: crates/tools/src/mcp_lifecycle.rs:1-471 (pre-landed at HEAD, matches contract).
- Wiring: crates/tools/src/lib.rs:32 `pub mod mcp_lifecycle;` (integrator-owned, untouched).
- Cited precedent: crates/tools/src/mcp.rs, ext_perms.rs, ext_secure.rs, tool_allow.rs:8 (not re-read; contract self-contained).

## Observed scenario
- Frozen suite GREEN on arrival, pre-implementation-change: 5/5 pass, 0 fail.
- RED gap (honest): no compiling-RED phase observable — impl pre-existed at HEAD; frozen hash 2fbaca62 recorded, zero test edits.
- One warning: `unused import: thiserror::Error` (mcp_lifecycle.rs:11). Derive emits fully-qualified refs; import unneeded.

## Target boundary
- Contract: Disabled/Enabled/Starting/Ready/Error{code}; set_enabled/mark_starting/mark_ready/mark_error/reconnect+refresh/is_runnable/persist/restore/restore_bytes; free fns mirror methods.
- Failure: EmptyId/Unknown/NotEnabled/Overflow(+Duplicate/IdTooLong/CodeTooLong/PersistedTooLarge/InvalidPersisted additive); rejections leave registry unchanged (validate-before-mutate).
- Bounds: MAX_SERVERS=64, MAX_ID_LEN=128 bytes, MAX_ERROR_CODE_LEN=64, MAX_PERSISTED_BYTES=16KiB; sorted ids; no Command/thread/IO/net/clock; forbid(unsafe_code); states/persist carry ids+codes only.
- Cancel/reclaim path: set_enabled(false) from any state -> Disabled, runnable_ids empty (off-means-off); reconnect explicit only, never timer/auto.

## Tests
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --test mcp_lifecycle -- --test-threads=1` → 5/5 ok (final rerun on byte-identical tree).
- Impl sha256 5e63c909997155a7afb4cd9288127b3eb179a8e4b8b8e18a20f4bd9f3938ece4 (pre-existing, zero writes by this lane); frozen test sha256 2fbaca62 unchanged.
- `python3 tools/lane_gate.py --run` → PASS (repo-wide gate; includes mcp-relevant checks, exit 0).

## Decisions
- No implementation edit: owned file byte-identical at HEAD and already satisfies full contract (states/transitions/guards/persist/restore/bounds, off-means-off, explicit-only reconnect, no IO/net/clock, forbid(unsafe_code)). Earlier warning note retracted: `use thiserror::Error` is required (derive macro in scope); the warning line seen was from `crates/security/src/os_backend.rs:46`, a foreign file, not this lane.
- Test file untouched (frozen sha 2fbaca62). lib.rs wiring (line 32) pre-present; no integrator action.

## Remaining unknowns
- None on contract. Integration seam: lib.rs wiring already present (line 32); no integrator action needed.
