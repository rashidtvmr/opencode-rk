# BRIDGE-PAR-219 scratchpad

claim: ses_par219 ok (in-progress).
source: packages/opencode/src/cli/cmd/run/runtime.lifecycle.ts:1-60 Lifecycle handle create/teardown ordering; crates/opentui-bridge/src/turn_wire.rs TurnWire begin/finish, turn_line cap 256.
target: crates/opentui-bridge/src/run_lifecycle_full.rs only. No lib.rs/Cargo.toml/turn_wire.rs edits. No cargo/commit.
tests: 8 in-file (idle, start, double-start, stop+count, stop-noop, reset, reset-noop, cap).
decisions: plain String phase (only 3 values, cap 32 trivially holds); saturating turns; start/stop/reset guards by phase.
verify: rustfmt --check only.
