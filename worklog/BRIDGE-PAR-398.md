# BRIDGE-PAR-398 scratchpad (UNCLAIMED: orchestrator owns claims.json, file-only per order)

Claim: none (did not touch tasks/completion/claims.json).
Source: TS truth packages/tui/src/plugin/command-shim.ts:1-40 (deprecated v1 `api.command`, palette show const, dialog shim); prior art crates/opentui-bridge/src/command_shim.rs (registry) and command_shim_full.rs (ShimFlow, forbid unsafe, ponytailed).
Target: ONE new file crates/opentui-bridge/src/plugin_shim_full.rs. No lib.rs/Cargo.toml/command_shim_full.rs edits. No cargo/commit.
Tests: run_bumps, run_empty_false, truncates (inline, 3).
Decisions: single-cmd struct (not registry) per spec; chars().take(128) truncation; saturating ran; ran() accessor extra for assertability; no blanks style to hold <70 lines (60).
Unknowns: module wiring into lib.rs left to orchestrator.
