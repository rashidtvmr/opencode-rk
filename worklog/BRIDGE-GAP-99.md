# BRIDGE-GAP-99
claim: BRIDGE-GAP-99 via ses_gap99 ok.
source: packages/tui/src/runtime.tsx (abbreviateHome only), packages/tui/src/context/runtime.tsx (provider/defaults pattern), crates/opentui-bridge/src/run_runtime.rs (forbid unsafe, cfg(test) style).
target: crates/opentui-bridge/src/tui_runtime.rs only. no lib.rs/Cargo.toml edits, no cargo, no commit.
tests: defaults_match_spec, kitty_gate_accepts_level_3, kitty_over_returns_false, title_truncates_at_cap, summary_non_empty.
decision: chars().take(128) truncation (char-boundary safe); enable_kitty leaves flag untouched on reject.
