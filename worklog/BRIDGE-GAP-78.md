# BRIDGE-GAP-78
claim: ses_gap78 via completion_claims.claim
source: packages/opencode/src/cli/cmd/run/turn-summary.ts:5 turnSummaryCommit; crates/opentui-bridge/src/run_splash.rs:47 summary_commit naming ref (no edit)
target: crates/opentui-bridge/src/run_turn_summary.rs only
tests: add_caps_rows_at_32, add_clips_label_to_128_chars, mark_done_oob_returns_false, render_checks_prefixes, render_empty_is_empty, done_flag_set_by_mark, render_caps_at_2kib
decisions: char-based LABEL_CAP clip; byte-based RENDER_CAP with char_boundary guard; Vec cap 32 fail-closed false
unknowns: none
