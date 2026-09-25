# BRIDGE-PAR-382 scratchpad

- Claim: UNCLAIMED (orchestrator owns tasks/completion/claims.json; file-only per task order, no ledger touch).
- Source: packages/tui/src/context/location.tsx:1-14 LocationRef accessor + LocationProvider.
- Boundary: one file crates/opentui-bridge/src/ctx_location_full.rs; no lib.rs/Cargo.toml edits.
- Tests: set_ok, set_rejects, file_caps (3, inline).
- Decisions: bool fail-closed set; file_of splits / and \\, chars().take(128); Default empty.
- Unknowns: none blocking; rustfmt check pending.
