# BRIDGE-PAR-308 prompt_display_full
Claim: ses_par308. Owned file: crates/opentui-bridge/src/prompt_display_full.rs.
Source evidence: TS truth packages/tui/src/prompt/display.ts:1-48 (grapheme width slice/char helpers); local clip_to_width crates/opentui-bridge/src/unicode_width.rs:142, line_width:119.
Observed: file written, 61 lines, forbid(unsafe_code), std-only.
Target boundary: display_text (first line, cluster-safe clip), display_count (N chars), is_empty. No lib.rs/Cargo.toml edits.
Tests: 5 (clips_ascii, first_line_only, wide_boundary_safe, count_label, empty_check). rustfmt --check EXIT=0.
Decisions: reuse clip_to_width (no new width table); lines().next for first line (handles \n, no \r strip); ponytail no singular/plural.
Unknowns: none blocking.
