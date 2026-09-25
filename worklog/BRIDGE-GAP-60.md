# BRIDGE-GAP-60 scratchpad

Claim: session timeline dialog, TS truth dialog-timeline.tsx:10 DialogTimeline.
Source evidence:
- TS `/home/rashid/projects/opencode/packages/tui/src/routes/session/dialog-timeline.tsx:22-44` user-message options title+value, reverse chrono.
- Rust style `crates/opentui-bridge/src/dialog_select.rs:22-31` MAX_LABEL 256 pattern.
Target boundary: ONE file `crates/opentui-bridge/src/session_timeline.rs`, std-only, forbid unsafe, <200 lines.
Tests: push_cap, cursor_wraps_forward, cursor_wraps_backward, selected_none_when_empty, fork_returns_id_clone, overlong_truncates.
Decisions: trunc-on-construct (not Result) for id/label; wrapping via rem_euclid; fork_selected clones id.
Unknowns: none. rustfmt --check clean, 142 lines, 6 tests. No cargo run (scope ban).
