# BRIDGE-GAP-65

Claim: runtime shared handle table. Session ses_gap65.
Source: TS `packages/opencode/src/cli/cmd/run/runtime.shared.ts:1-17` (reusePendingTask slot); naming from `crates/opentui-bridge/src/run_runtime.rs:1-158` (caps, forbid unsafe, in-file tests).
Target: `crates/opentui-bridge/src/run_runtime_shared.rs` only. No lib.rs/Cargo.toml/run_runtime.rs edits. No cargo. No commit.
Tests: register_ok, register_dup_errs, register_empty_errs, register_cap_errs, kill_flips_alive, alive_false_unknown (6).
Decisions: std-only; RuntimeHandle fields private + accessors; register validates empty/len/dup/cap in that order; kill false-unknown; is_alive false-unknown.
Unknowns: none.
