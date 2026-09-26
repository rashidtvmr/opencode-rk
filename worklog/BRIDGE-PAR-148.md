# BRIDGE-PAR-148
claim: ses_par148 owns prompt_ctx.rs
evidence: prompt.tsx (PromptRef get/set ctx, 18 lines); prompt_composer.rs Composer/caps/paste/submit-gate
target: crates/opentui-bridge/src/prompt_ctx.rs, std-only, forbid unsafe, 167 lines
tests: 6 in-file (empty-false, front-push, prev/next walk, evict-50, clear, entry-4KiB)
decision: caps in chars; hcursor None=editing; set_draft exits browse
verify: rustfmt --check PASS
