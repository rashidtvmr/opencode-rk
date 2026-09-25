# BRIDGE-PAR-136 scroll_step

claim: ses_par136 via completion_claims.claim, ok.
source: packages/tui/src/util/scroll.ts:1-27 (CustomSpeedScroll selector only, no accel logic); crates/opentui-bridge/src/scroll_accel.rs:1-126 (trait boundary, read-only).
target: crates/opentui-bridge/src/scroll_step.rs only. lib.rs/Cargo.toml/scroll_accel.rs untouched.
tests: 6 in-file #[cfg(test)] (first=1, fast-repeat 1-2-4, slow-gap reset, cap 10, direction reset, reset clears).
decisions: velocity tracks emitted step magnitude with sign (doubling chain 1-2-4-8-10); step_for pure abs-capped; wrapping_sub for ms wrap; delta=0 treated positive (signum 0 != stored, resets to 1).
verify: rustfmt --check clean. No cargo per scope.
