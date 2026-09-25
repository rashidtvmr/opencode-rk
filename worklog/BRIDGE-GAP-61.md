# BRIDGE-GAP-61 run_footer_menu

Claim: ses_gap61, ledger completed.
Source: footer.menu.tsx (createFooterMenuState selected/offset/move/reveal); sibling run_footer_view.rs style (forbid unsafe, std-only).
Target: crates/opentui-bridge/src/run_footer_menu.rs, 161 lines.
Tests: 6 in-file (open_caps_at_16, cursor_wraps_forward_and_back, chosen_none_when_closed, close_clears, empty_open_false, caps_truncate_label_and_action).
Verify: rustfmt --check PASS. No cargo/commit per scope.
