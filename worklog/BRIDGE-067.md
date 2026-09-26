# BRIDGE-067 prompt composer extensions

Claim: SubmitGate + can_submit_gated, PasteBranch + classify_paste, SUMMARY consts + summarize_pasted, next_image_n/next_pdf_n.
Evidence: index.tsx:954-969 gates; :1184-1210 paste branch + summary threshold; :1221-1231 counters. File crates/opentui-bridge/src/prompt_composer.rs appended after tests mod (existing preserved).
Tests: 10 new in bridge067_tests (gated x2, classify x4, summarize x3, counters x1). Existing 10 untouched. No cargo run per task constraint; logically green (std only, format!/trim/replace, no unsafe).
Decisions: exit-quit-:q + agent/model gates stay caller-side (need runtime state); gate struct covers pure bool flags only. SVG name from basename, mime default "image".
Unknowns: none.
