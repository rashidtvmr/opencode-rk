# BRIDGE-PAR-183 prompt_assemble.rs

Claim: ledger in-progress, session ses_par183.
Evidence:
- crates/opentui-bridge/src/prompt_ctx.rs:18 PromptCtx, :42 set_draft, :48 submit
- crates/opentui-bridge/src/prompt_full.rs:11 normalize_paste
Boundary: ONE new file prompt_assemble.rs. No edit to lib.rs/Cargo.toml/prompt_ctx/prompt_full/prompt_shared_full. No cargo/commit.
Tests: 6 in-file (roundtrip, empty+ansi None, crlf, prefix, char-safe clip, zero-width+first-line).
Decision: empty check = normalized is_empty (spec-literal, no trim); draft_line = first line only + char-safe width clip.
