# BRIDGE-GAP-32 scratchpad (ses_gap32)
- Claim: BRIDGE-GAP-32 via tools/completion_claims.py (ses_gap32).
- Source evidence: crates/cli/src/tui_entry.rs:452-543 `native_page_lines` Chat branch; crates/opentui-bridge/src/unicode_width.rs `clip_to_width`; crates/opentui-bridge/src/transcript_paint.rs `paint_lines`.
- Observed: no full-frame painter in opentui-bridge; tui_entry Chat branch composes title/status/rule/transcript/composer/footer inline with char-count clip.
- Target boundary: ONE new file crates/opentui-bridge/src/paint_full.rs only. No lib.rs/Cargo.toml edits. No cargo. No commit/push.
- Tests: in-file #[cfg(test)] >=5 (frame_height_truncates, lines_clip_to_width, empty_transcript_placeholder, draft_shown, hints_shown, clamps_minimum).
- Decisions: reuse clip_to_width for every row; reuse paint_lines with assistant role for transcript window; mirror tui_entry Chat layout (body window h-7, pad to h-3, bottom rule/composer/footer).
- Remaining: rustfmt --check file; flip ledger completed only if file+tests exist.
