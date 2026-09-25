# BRIDGE-PAR-252
claim: BRIDGE-PAR-252 ses_par252 ok
source: crates/opentui-bridge/src/footer_menu_full.rs:15 FooterMenuFull (open_menu/close_menu/move_cursor/is_open/cursor)
target: crates/opentui-bridge/src/footer_menu_full2.rs MenuFlow{menu,opened}+open/close/select_next/opened_count
tests: open_empty_no_count, open_counts_success, close_keeps_count, select_next_gates_and_wraps
verify: rustfmt --check PASS, 89 lines, std-only, forbid(unsafe_code)
