# BRIDGE-GAP-96 scratchpad

claim: ses_gap96, ledger in-progress ok.
source: packages/tui/src/component/prompt/autocomplete.tsx (options memo slice(0,10), move(-1|1) wraps, selected index).
target: crates/opentui-bridge/src/prompt_autocomplete.rs only. lib.rs/Cargo.toml untouched. no cargo run.
impl: CompleteItem{text cap256,hint cap128}, CompleteList{items cap32,cursor}, filter(prefix match,new,cursor0), move_cursor(isize,rem_euclid wrap), current(). std-only, forbid unsafe.
tests: 6 (filter_matches_prefix, filter_none_empty, cursor_wraps_forward, cursor_wraps_backward, current_none_empty, cap_32_items_and_text).
verify: rustfmt --check only. pending.
