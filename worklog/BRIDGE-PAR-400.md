# BRIDGE-PAR-400 scratchpad (UNCLAIMED - file-only per orchestrator override)

Task: crates/opentui-bridge/src/plugin_runtime_full.rs (new file only).
Ledger: NOT claimed. Orchestrator owns tasks/completion/claims.json; proceeded file-only per task order. No claim/update attempted.

Claim: PluginRt loaded-flag state machine.
Source evidence:
- TS truth: /home/rashid/projects/opencode/packages/tui/src/plugin/runtime.tsx:12 (`createPluginRuntime`, load/clear shape)
- Sibling bounded table (not edited): crates/opentui-bridge/src/plugin_runtime.rs:1 (`forbid(unsafe_code)`), :20 (`PluginRuntime`)
Target boundary: ONE new file only. No lib.rs / Cargo.toml / plugin_runtime.rs edits. No cargo, no commit.
Tests: 4 unit tests in-file (initial, load bumps, load twice, unload keeps count).
Decisions: load = loaded=true + saturating_add(1) each call; unload = loaded=false, count kept; std-only, forbid(unsafe_code), 67 lines.
Unknowns: wiring into lib.rs left to orchestrator/integrator.
