# BRIDGE-PAR-276 scratchpad
- claim: BRIDGE-PAR-276 / ses_par276 / worklog/BRIDGE-PAR-276.md
- source: packages/tui/src/component/dialog-session-list.tsx:45 DialogSessionList; pattern: crates/opentui-bridge/src/dialog_select.rs (forbid unsafe, consts, tests)
- target: crates/opentui-bridge/src/dialog_session_list_full.rs only; lib.rs/Cargo.toml untouched
- impl: SessionListDialog {ids cap 64 x 128 chars, cursor} + push/move_cursor(selected wrap via rem_euclid)/selected/selected_short(id8); std-only, forbid(unsafe_code), 117 lines
- tests: 5 (select_and_short, push_rejects_bad, push_caps_at_64, cursor_wraps_both_ways, cursor_empty_noop)
- verify: rustfmt --check FMT-OK (no cargo per scope)
