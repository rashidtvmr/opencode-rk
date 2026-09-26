# BRIDGE-GAP-104
claim: ses_gap104 ok
evidence: runtime.lifecycle.ts close-once/splash-state, run_runtime.rs:8-14 RuntimePhase
impl: crates/opentui-bridge/src/run_lifecycle.rs 126 lines, std-only forbid(unsafe_code)
tests: boot_is_ready, begin_turn_ok, begin_turn_busy_false, end_turn_counts, close_once, closed_is_terminal
verify: rustfmt --check PASS; no cargo per scope
