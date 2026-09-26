# BRIDGE-GAP-108 scratchpad

Claim: BRIDGE-GAP-108 via cc.claim session ses_gap108. Owned file: crates/opentui-bridge/src/run_runtime_main.rs (append-only).
Source evidence:
- crates/opentui-bridge/src/run_runtime_main.rs:1-113 (RunMain started/turns/last_error, ERROR_CAP 512, mod tests 6 tests).
- packages/opencode/src/cli/cmd/run/runtime.ts:1-120 skimmed (runInteractiveRuntime boot/lifecycle/queue wiring); grep TurnState|RuntimeStatus|interrupt|queue_depth no matches (names are new Rust-side model, no exact TS symbol).
Observed: file ends line 113 `}` closing mod tests. No trailing items.
Target boundary: APPEND TurnState, RuntimeStatus, RuntimeQueue2, TurnTracker + interrupt/queue_depth/complete_turn/interrupt_count/totals. Keep lines 1-113 byte-identical. New mod tests2 >=5. No lib.rs/Cargo.toml/run_runtime.rs edits. No cargo. No commit/push. Verify rustfmt --check only.
Tests: tests2 (7): turn_default_idle, status_interrupt_clears_turn, status_queue_depth_pushes, queue2_cap_bounds, tracker_totals_saturate, tracker_counts, status_from_queue.
Decisions: RuntimeStatus owns interrupt()+queue_depth() (only struct with turn+interrupted+queue_depth fields, matches spec semantics); RuntimeQueue2 {depth,cap} push/pop counter backing; TurnTracker {completed,interrupted_count} saturating counters per spec.
Unknowns: none.
