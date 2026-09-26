# BRIDGE-PAR-356 (unclaimed, file-only, per orchestrator)

- Claim: skipped ledger claim per task order (no claims.json touch). Status: unclaimed.
- Source: `packages/tui/src/component/prompt/workspace.tsx:16` (`usePromptWorkspace`, selection path state).
- Target: `crates/opentui-bridge/src/comp_prompt_ws_full.rs` (sole owned file, no lib.rs/Cargo.toml edit).
- Tests: 3 in-file (`set_caps_at_512`, `label_tildes_home_prefix`, `label_caps_at_128_no_home`).
- Decisions: std-only, `forbid(unsafe_code)`, byte-floor caps respect char boundaries; 64 lines (<80).
- Verification: `rustfmt --check` PASS (exit 0). No cargo per scope.
- Unknowns: none.
