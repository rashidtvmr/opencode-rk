# BRIDGE-PAR-304
claim: ses_par304 in-progress via tools/completion_claims.py
source: packages/tui/src/prompt/part.ts:1-29 (strip IDs, expand pasted-text placeholders, tracked ranges); crates/opentui-bridge/src/run_prompt_part.rs:1-39 (existing split_part/render caps)
target: crates/opentui-bridge/src/prompt_part_full.rs only; PromptPart{kind cap32,text cap4KiB}+new/is_text/preview64; std-only, forbid unsafe, <100 lines, >=4 tests
tests: caps_kind_len, caps_text_len, is_text_flag, preview_truncates, preview_short_passthrough
decision: pub fields for direct assert; char-based caps (unicode-safe); preview pure take(); no lib.rs/Cargo.toml edits; rustfmt --check only
