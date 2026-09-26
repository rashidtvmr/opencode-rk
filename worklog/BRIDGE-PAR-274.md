# BRIDGE-PAR-274 scratchpad

claim: BRIDGE-PAR-274 ses_par274 worklog/BRIDGE-PAR-274.md
source: packages/tui/src/component/dialog-model.tsx:1-50 (DialogModel select w/ query/favorites/recents); pattern crates/opentui-bridge/src/dialog_select_full.rs:1-101 (SelectDialog push/move_cursor/selected)
target: crates/opentui-bridge/src/dialog_model_full.rs only; no lib.rs/Cargo.toml
tests: 6 in-file (empty_none, push_select, move_wrap, cap_64_reject, trunc_128_empty_reject, move_empty_no_panic)
decisions: mirror SelectDialog, caps 64/128 per spec; std-only forbid(unsafe_code)
unknowns: none
