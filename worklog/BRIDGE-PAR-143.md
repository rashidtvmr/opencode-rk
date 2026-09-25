# BRIDGE-PAR-143 scratchpad

Claim: ledger in-progress, session ses_par143.
Source evidence:
- packages/tui/src/routes/session/dialog-fork-from-timeline.tsx:12 DialogForkFromTimeline (full-session undefined entry + per-message onSelect fork).
- crates/opentui-bridge/src/session_fork_dialog.rs:1-47 (ForkDialog single-message confirm gate; naming MAX_MESSAGE_ID/message_id/confirmed; NOT edited).
Target boundary: ONE new file crates/opentui-bridge/src/fork_dialog_full.rs only. No lib.rs/Cargo.toml/session_fork_dialog.rs/fork_route.rs edits. No cargo, no commit.
Tests: 7 in-file (add ok, blank reject, truncate, cap, confirm bounds, confirmed none before, overwrite confirm).
Decisions: std-only, forbid(unsafe_code), MAX_ID 64 / MAX_PICKS 64, index = position at insert, confirm bounds-checked overwrite, confirmed_id maps via picks.get.
Remaining: rustfmt --check only per scope.
