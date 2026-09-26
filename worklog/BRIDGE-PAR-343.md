# BRIDGE-PAR-343 scratchpad (unclaimed)

Status: unclaimed (ledger overflow; orchestrator owns claims.json; file-only per task).
Task: crates/opentui-bridge/src/plugin_home_full.rs, no lib.rs/Cargo.toml edits, no cargo/commit.
Claim: skipped per explicit task instruction (do NOT touch claims.json).
Source: TS truth `packages/tui/src/feature-plugins/home/tips.tsx:1-40` (home_bottom slot, tips toggle); prior art `crates/opentui-bridge/src/home_footer.rs:1-116` (MAX_TIPS 16, trunc chars, render clip), `home_plugin.rs:1-85` (slot passthrough).
Target: PluginHome {cards cap16 each 128} + push->bool + lines(width) clipped cap18 + len.
Tests: 6 unit tests in-file. Verify: rustfmt --check only.
Unknowns: none; wiring left to orchestrator (lib.rs not touched).
