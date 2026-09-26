# BRIDGE-PAR-342 scratchpad (UNCLAIMED: ledger overflow, orchestrator owns claims.json)

- Claim: skipped per task order (no claims.json touch). Status: unclaimed, file-only.
- Source evidence: TS truth `packages/tui/src/config/index.tsx:1-50` (re-export + Schema sections); sibling `crates/opentui-bridge/src/tui_config.rs:1-155` (forbid unsafe, bounded theme/leader/scroll style).
- Observed: no `config_index*` in `crates/opentui-bridge/src` (glob miss).
- Target boundary: ONE new file `crates/opentui-bridge/src/config_index_full.rs`; no lib.rs/Cargo.toml edits; no cargo/commit.
- Tests: 4 unit tests in-file (roundtrip, dup/empty/long, cap, missing).
- Decisions: add=false on empty/>128/dup/full; Vec preserves insertion order; is_empty added for clippy len_without_is_empty; std-only, forbid(unsafe_code).
- Unknowns: whether orchestrator wires module into lib.rs later (out of scope).
