# BRIDGE-PAR-385 scratchpad (UNCLAIMED - orchestrator owns ledger)

Claim: none. Proceeded file-only per task instruction. No claim/update to tasks/completion/claims.json.
Source: packages/tui/src/context/route.tsx:23 Route = home|session|plugin; :47-51 initialRoute parsing.
Target: crates/opentui-bridge/src/ctx_route_full.rs only. lib.rs/Cargo.toml untouched.
Tests: 5 tests (home_empty, session_describes, plugin_describes, id_caps_128, describe_caps_160).
Decisions: struct mirrors union via kind+id strings; caps 16/128/160; std-only; forbid(unsafe_code); 79 lines.
Unknowns: wiring into lib.rs left to orchestrator.
