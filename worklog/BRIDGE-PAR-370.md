# BRIDGE-PAR-370 scratchpad (unclaimed lane)

Status: unclaimed (orchestrator owns claims.json; proceeded file-only per task order).
Task: ONE new file `crates/opentui-bridge/src/ctx_sync_full.rs`. No lib.rs/Cargo.toml edits. No cargo/commit.

Claim: none (skipped `tools/completion_claims.py` by instruction).
Source evidence:
- TS truth `packages/tui/src/context/sync.tsx:1-50` (store/event batch pattern; reconcile/batch imports).
- Style ref `crates/opentui-bridge/src/sync_store.rs:1-20` (`#![forbid(unsafe_code)]`, std-only).
Target boundary: `CtxSync { tick: u64, dirty: bool }` + `mark` + `flush()->bool` + `tick_of()->u64`.
Tests: 3 in-module (`fresh_is_clean_at_zero`, `mark_bumps_tick_and_flush_reports_dirty_once`, `flush_without_mark_stays_clean`).
Decisions: derived `Default`; `const fn new/tick_of`; `wrapping_add`; `flush` true when was dirty, clears latch, keeps tick.
Unknowns: none; lib.rs wiring left to orchestrator.
