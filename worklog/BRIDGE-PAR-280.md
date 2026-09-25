# BRIDGE-PAR-280 scratchpad

Claim: BRIDGE-PAR-280, session ses_par280, status completed.
Source: packages/tui/src/ui/dialog-prompt.tsx:20 DialogPromptProps title/value, :28-31 confirm(); style ref dialog_select.rs (forbid unsafe, const caps, Result errs, in-file tests).
Target: crates/opentui-bridge/src/dialog_prompt_full.rs only. No lib.rs/Cargo.toml edits. No cargo/commit per task.
Tests: 6 in-file (bad title, append+cap, char-vs-byte, submit done, cancel blocks, set_title closed). Verification: rustfmt --check FMT_OK, 120 lines (limit 120).
Done: file+tests exist, fmt clean.
