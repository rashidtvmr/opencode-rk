# BRIDGE-GAP-50 scratchpad

Claim: BRIDGE-GAP-50 ses_gap50 worklog/BRIDGE-GAP-50.md, ledger in-progress confirmed.
Source: crates/opentui-bridge/src/run_footer.rs (RunFooter append cap 128, flush, destroy latch, event view) as style ref.
Target: crates/opentui-bridge/src/run_subagent.rs (161 lines, std-only, forbid unsafe).
Tests: 6 in-file (spawn_sets_running_with_id, spawn_empty_id_errs, spawn_long_id_errs, append_caps_at_500_evicting_oldest, finish_done_and_failed, snapshot_clones_transcript).
Verify: rustfmt --edition 2021 --check FMT_OK. No cargo per lane role. No lib.rs/Cargo edits, no commit/push.
