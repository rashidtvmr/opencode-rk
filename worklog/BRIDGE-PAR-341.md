# BRIDGE-PAR-341 scratchpad (UNCLAIMED - orchestrator owns ledger, overflow blocks claims)

Claim: skipped per parent order; no tasks/completion/claims.json touch.
Source: packages/tui/src/config/keybind.ts:45-240 Definitions, :449-458 parse.
Observed: keybind_tables.rs covers literal tables + object form; no owned mutable full config map.
Target: crates/opentui-bridge/src/keybind_config_full.rs only; no lib.rs/Cargo.toml.
Tests: 5 inline (roundtrip, replace, reject, cap-64, miss). Written before impl (single file).
Decisions: Vec<(String,String)> flat, replace-on-same-action, fail-closed 64/64.
Unknowns: none.
