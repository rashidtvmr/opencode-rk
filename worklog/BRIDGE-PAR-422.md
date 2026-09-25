# BRIDGE-PAR-422 (unclaimed, orchestrator owns ledger)

- Claim: skipped per prompt (no claims.json touch).
- Sources: packages/tui/src/feature-plugins/sidebar/context.tsx:1-30 (context token/cost memo); lsp.tsx:1-30 (LSP list view); ref crates/opentui-bridge/src/sidebar_panels.rs (cap/trunc pattern).
- Target: crates/opentui-bridge/src/side_panels_full.rs only; lib.rs/Cargo.toml/sidebar*.rs untouched.
- Tests: add_caps_at_16, item_truncates, lsp_truncates, summary_caps.
- Decisions: Vec<String> cap 16, per-item 128 chars, lsp 128, summary 256; ponytail: skipped multi-line render, add when panel UI wired.
- Unknowns: none.
