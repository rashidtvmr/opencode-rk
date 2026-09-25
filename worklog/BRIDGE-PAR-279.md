# BRIDGE-PAR-279 scratchpad

claim: BRIDGE-PAR-279 ses_par279 worklog/BRIDGE-PAR-279.md
source: packages/tui/src/ui/dialog-select.tsx:23-30 DialogSelectProps options + cursor/select semantics
target: crates/opentui-bridge/src/dialog_select_full.rs, SelectDialog only, no lib.rs/Cargo.toml
tests: 6 in-file (empty_none, push_select_confirm, move_wrap, cap_32_reject, trunc_256_empty_reject, move_empty_no_panic)
decisions: chars().take(256) truncation; rem_euclid wrap; push empty reject
