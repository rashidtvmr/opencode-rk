# BRIDGE-GAP-66 scratchpad

Claim: new file crates/opentui-bridge/src/run_runtime_boot.rs.
Source: packages/opencode/src/cli/cmd/run/runtime.boot.ts (resolveModelInfo/resolveSessionInfo/resolveRunTuiConfig/resolveDiffStyle before first frame); sibling crates/opentui-bridge/src/run_runtime.rs:1 (forbid unsafe, lifecycle pattern).
Boundary: BootStep{Init,LoadConfig,ConnectDaemon,Ready} + BootLog{cap32,fail latch} + advance/fail/is_ready. std-only, forbid(unsafe_code).
Tests: advance_ok, fail_blocks_advance, ready_gate, ready_blocked_after_fail, cap_32, note_truncates_256.
Decisions: failure records last-step note then latches; is_ready requires Ready + no fail.
Unknowns: none. No cargo run per scope; rustfmt check only.
