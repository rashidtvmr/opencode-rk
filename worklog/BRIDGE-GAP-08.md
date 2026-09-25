# BRIDGE-GAP-08 scratchpad (ses_gap08)

- Claim: BRIDGE-GAP-08 in-progress, session ses_gap08, scratchpad worklog/BRIDGE-GAP-08.md.
- Source evidence:
  - `crates/opentui-bridge/src/debounce.rs:1-72` — existing `Debounced<T: Clone + PartialEq>` with `set`/`flush`/`cancel`/`take`, NO clock/deadline. New logic per its own header (`:2-6`: "no upstream equivalent", "timer-free state machine; caller drives flush"). NOT TOUCHED (not owned).
  - TS `packages/tui/src/util/signal.ts:3 createDebouncedSignal` — absent in this checkout (`packages/` does not exist). Semantics per task card: set-then-fire-after-ms.
  - Sibling style: `crates/opentui-bridge/src/bg_pulse.rs:1-28` header w/ source evidence + ponytail note, `#![forbid(unsafe_code)]`, std-only, in-file `#[cfg(test)]`.
- Target boundary: ONE new file `crates/opentui-bridge/src/debounce_signal.rs`. No lib.rs edits (integrator prewires `pub mod`).
- Design: `Debounced<T: Clone>` (Clone-only, unlike debounce.rs PartialEq). `push(value, now_ms)` arms `deadline = now.saturating_add(delay)` (caller clock passed at push = timer-free). `take_if_due(now_ms)` fires iff armed and now >= deadline. `flush()` commits ignoring deadline. Last-push-wins supersede.
- Tests (in-file): no_fire_early, fires_at_deadline, supersede_rearms, flush_now_and_empty, clone_only_type (Clone-but-not-PartialEq proves looser bound).
- Remaining: write file, rustc --test, ledger completed, commit+push.
