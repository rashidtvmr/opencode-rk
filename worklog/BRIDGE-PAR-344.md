# BRIDGE-PAR-344 (unclaimed, ledger overflow, orchestrator owns ledger)

- Claim: skipped per prompt (do NOT touch claims.json). Status: unclaimed, file-only.
- Source evidence: packages/tui/src/feature-plugins/sidebar/files.tsx:14-52 (Modified Files diff rows); crates/opentui-bridge/src/sidebar.rs:1-49 (existing Sidebar/Panel/FileRow/McpRow, MAX_ROWS 512); crates/opentui-bridge/src/clip_line.rs:1-22 (clip_line/pad_line width pattern, forbid unsafe); crates/opentui-bridge/src/bg_pulse_full.rs:1-50 (style template: forbid, ponytail note, small struct + tests).
- Observed: sidebar/*.tsx unbounded row lists (files/lsp/mcp/todo/context/footer). Existing bridge sidebar.rs covers typed rows; no bounded generic string-row buffer for full sidebar render.
- Target boundary: ONE new file crates/opentui-bridge/src/plugin_sidebar_full.rs. No lib.rs/Cargo.toml edits. No cargo. No commit.
- Tests: 4 unit tests in-file (push_and_len, rejects_when_full, truncates_long_row, lines_clip_width).
- Decisions: char-count clipping (not display-width) to keep std-only under 100 lines; ponytail note documents upgrade path. is_empty added (clippy::len_without_is_empty avoidance).
- Unknowns: none for lane scope. Integration (lib.rs mod wiring) left to orchestrator.
- Verification: rustfmt --check PASS, 91 lines.
