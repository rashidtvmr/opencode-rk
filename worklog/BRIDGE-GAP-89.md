# BRIDGE-GAP-89
claim: ses_gap89
source: packages/tui/src/prompt/part.ts:1-29; style run_prompt_shared.rs:1-30
boundary: ONE new file run_prompt_part.rs, no lib.rs/Cargo.toml edits, no cargo run
tests: slash_split, mention_split, text_passthrough, empty_text, render_roundtrip (in-file cfg(test))
decisions: leading / -> Slash(cmd-to-space,128); contains @ -> Mention(first at-token,512); else Text(4KiB); render inverts
fmt: rustfmt --check PASS; wc 80 lines
