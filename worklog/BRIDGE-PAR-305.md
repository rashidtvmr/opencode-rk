# BRIDGE-PAR-305 scratchpad

claim: BRIDGE-PAR-305 via ses_par305, scratchpad worklog/BRIDGE-PAR-305.md
source: packages/tui/src/prompt/traits.ts:1-29 `computePromptTraits`, `PromptTraitsInput`
observed: native `run_prompt_traits.rs:13-22` covers slash/mention/paste flags only, no part-trait abstraction
target: ONE file crates/opentui-bridge/src/prompt_traits_full.rs, trait PromptTrait + TextPart 4KiB + kind_of helper
tests: kind_text_roundtrip, cap_4kib, kind_of_helper (+ empty)
decisions: char-based cap 4096 (match prompt_ctx.rs MAX_ENTRY_CHARS), kind "text", std-only, forbid(unsafe_code)
remaining: done - rustfmt --check PASS, 70 lines (<90)
