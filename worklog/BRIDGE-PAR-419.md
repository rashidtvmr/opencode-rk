# BRIDGE-PAR-419 scratchpad (UNCLAIMED: orchestrator owns ledger, file-only lane)

- claim: skipped per prompt (no claims.json touch); status unclaimed.
- source: TS `packages/tui/src/feature-plugins/system/diff-viewer-ui.tsx:1-40` (PanelGroup/Panel chrome); Rust pattern `err_line.rs` (cap-512 line), `mem_line.rs` (ponytail note).
- target: `crates/opentui-bridge/src/diff_ui_full.rs` only; no lib.rs/Cargo.toml/diff_viewer.rs edits.
- contract: DiffUi{file cap 512, hunk u32} + set_file (cap+reset) + next_hunk (saturating) + line (cap 512).
- tests: set_file_caps_and_resets, next_hunk_saturates, line_format_and_cap.
- decisions: char-cap (not byte) for unicode safety; saturating_add; default = empty/0.
- unknowns: none; wiring into lib.rs left to orchestrator.
