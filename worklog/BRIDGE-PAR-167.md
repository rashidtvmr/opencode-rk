# BRIDGE-PAR-167 scratchpad

Claim: BRIDGE-PAR-167 ses_par167 worklog/BRIDGE-PAR-167.md.
Source evidence:
- TS truth: packages/opencode/src/cli/cmd/run/prompt.shared.ts:1-153 (pure history state machine; text non-blank guard at :56,:74).
- Rust sibling: crates/opentui-bridge/src/run_prompt_shared.rs:1-124 (PromptEntry new caps 64/4096, PromptQueue cap 64).
Target boundary: ONE new file crates/opentui-bridge/src/prompt_shared_full.rs. No lib.rs/Cargo.toml/run_prompt_shared.rs/prompt_ctx.rs edits. No cargo/commit.
Tests: 6 in-file #[cfg(test)] (set trunc, attach cap, detach bounds, ready blank false, attach len cap, default empty).
Decisions: PromptSharedFull {text, attachments} + set/attach/detach/is_ready; MAX_TEXT 8192 chars, MAX_ATTACH 16, MAX_ATTACH_LEN 512; std-only forbid(unsafe_code).
Unknowns: none.
