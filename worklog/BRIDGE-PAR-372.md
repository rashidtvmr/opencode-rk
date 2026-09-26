# BRIDGE-PAR-372 (unclaimed: orchestrator owns claims.json, file-only lane)

## Claim
No ledger claim per task override. Proceeding file-only.

## Source evidence
- TS truth: `packages/tui/src/runtime.tsx:1-9` (repo has only `abbreviateHome`, no boot/ready export; contract taken from task spec).
- Pattern: `crates/opentui-bridge/src/run_runtime_full.rs:1-58` (forbid unsafe, doc mirror line, struct + impl + tests).

## Target boundary
- ONE file: `crates/opentui-bridge/src/runtime_tsx_full.rs`, <70 lines, std-only, forbid(unsafe_code).
- `RuntimeTsx { ready, boots }` + `boot` + `shutdown` + `is_ready` (+ `boots` getter, `new`).

## Tests
- `new_not_ready`, `boot_marks_ready_and_bumps`, `shutdown_clears_ready_keeps_boots`.

## Decisions
- `ponytail:` saturating_add caps boots at u32::MAX; wrapping/reset API add when lifecycle demands.
- Extra `boots()`/`new()` getters kept (needed for tests, Default covers construction).

## Unknowns
- None. lib.rs wiring explicitly out of scope.
