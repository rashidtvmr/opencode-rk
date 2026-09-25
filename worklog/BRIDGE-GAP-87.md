# BRIDGE-GAP-87
claim: ses_gap87
truth: packages/tui/src/routes/home/session-destination.tsx:13 HomeSessionDestination new|directory
file: crates/opentui-bridge/src/home_destination.rs (86 lines)
tests: new_is_new, existing_id, empty_errs, id_none_new, truncates
verify: rustfmt --check pass
