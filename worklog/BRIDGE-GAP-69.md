# BRIDGE-GAP-69
claim: ses_gap69 ok
source: packages/opencode/src/cli/cmd/run/session-data.ts (reducer; snapshot pick = lexicographic updated_at max)
owned: crates/opentui-bridge/src/run_session_data.rs
tests: latest_picks_max, empty_none, saturating_total, preview_truncates, preview_short_passthrough, id_caps_at_64
verify: rustfmt --check PASS
