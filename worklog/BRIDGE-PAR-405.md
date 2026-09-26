# BRIDGE-PAR-405 (unclaimed, file-only per orchestrator)

Claim: none (orchestrator owns claims.json; instructed proceed file-only).
Source: packages/tui/src/routes/home.tsx:22 `Home` prompt-first; existing home_route.rs / home_route_full.rs / home_index_full.rs (cursor pattern).
Boundary: crates/opentui-bridge/src/route_home_full.rs only; no lib.rs/Cargo.toml edits.
Tests: set_and_get, empty_ignored, truncates.
Decision: minimal HomeRoute{tab} + set_tab + tab_of, char-boundary trunc 32, ponytail: no Default/new, add when caller needs.
Unknown: lib.rs wiring left to orchestrator.
