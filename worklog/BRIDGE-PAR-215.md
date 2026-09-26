# BRIDGE-PAR-215 scratchpad

claim: BRIDGE-PAR-215 ses_par215 worklog/BRIDGE-PAR-215.md
source: footer.prompt.tsx:1-60 createPromptState history nav; prompt_full.rs:11 normalize_paste; prompt_ctx.rs/prompt_assemble.rs boundary (front-push vs back-push kept local)
target: crates/opentui-bridge/src/run_prompt_full.rs only; no lib.rs/Cargo.toml edit, no cargo
tests: 6 in-file (empty-none, push-clear, crlf-normalize, recall-last, evict-50, draft-cap)
decisions: back-push history + recall=last (simplest vs PromptCtx front-push); empty submit stores normalized text in draft; std-only forbid(unsafe_code); 99 lines <130
evidence: rustfmt --check exit 0, 99 lines
