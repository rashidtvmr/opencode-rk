# BRIDGE-PAR-307 scratchpad

claim: BRIDGE-PAR-307 via ses_par307 ok.
source: packages/tui/src/prompt/frecency.tsx:8-36 (FrecencyEntry, parse, freq/(1+ageDays) decay); sibling prompt_history_full.rs:1-60 (style: forbid unsafe, MAX const, struct+impl+tests).
target: crates/opentui-bridge/src/prompt_frecency_full.rs only. lib.rs/Cargo.toml untouched.
tests: 5 (empty_top, count_and_rank, blank_ignored, cap_64_evicts_oldest, top_truncates_n). No cargo run per scope; rustfmt --check PASS.
decision: simplified TS decay to count-ranked MRU (ponytail: counts only; add lastOpen decay when prompt store wires timestamps).
