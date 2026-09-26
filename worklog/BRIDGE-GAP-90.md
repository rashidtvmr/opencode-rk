# BRIDGE-GAP-90 scratchpad

claim: BRIDGE-GAP-90 via cc.claim, session ses_gap90.
source: TS `packages/opencode/src/cli/cmd/run/runtime.ts` runInteractiveRuntime bookkeeping; Rust naming from `crates/opentui-bridge/src/run_runtime.rs` boot/fn style.
target: ONE new file `crates/opentui-bridge/src/run_runtime_main.rs` only. No lib.rs/Cargo.toml/run_runtime.rs edits.
tests: in-file cfg(test) 6 tests: start_fresh, zero_turns, turns_saturate, error_truncates, error_overwrites, turn_increments.
decisions: std-only, forbid(unsafe_code), 113 lines <150. Error cap via chars().take(512). Turns saturating_add.
evidence: `rustfmt --check` clean (exit 0). No cargo run per scope.
