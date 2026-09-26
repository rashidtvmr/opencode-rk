# BRIDGE-GAP-84 scratchpad

claim: BRIDGE-GAP-84 ses_gap84 worklog/BRIDGE-GAP-84.md
source: packages/tui/src/routes/session/dialog-fork-from-timeline.tsx:12 DialogForkFromTimeline; crates/opentui-bridge/src/session_timeline.rs:14 MAX_ID, :25 TimelineEntry
observed: TS forks on onSelect with messageID; full-session undefined fork exists
target: ONE file crates/opentui-bridge/src/session_fork_dialog.rs, ForkDialog pick-then-confirm gate, no lib.rs/Cargo edits, no cargo run
tests: open_empty_errs, open_blank_errs, open_keeps_id_unconfirmed, confirm_sets_confirmed, cancel_clears_confirmed, double_confirm_idempotent, id_truncates_to_cap
decisions: trim+empty err; chars().take(64) cap mirrors TimelineEntry; confirm/cancel toggle bool; std-only forbid(unsafe_code)
unknowns: none
