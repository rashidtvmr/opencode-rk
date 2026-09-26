# BRIDGE-PAR-235

Claim: ledger `claim(...BRIDGE-PAR-235, ses_par235, worklog/BRIDGE-PAR-235.md)` OK (in-progress).
Source: `crates/opentui-bridge/src/timeline_dialog.rs:19` `TimelineDialog{entries,cursor,open}` + `dialog-timeline.tsx:10` DialogTimeline (user msgs, newest first).
Boundary: ONE new file `crates/opentui-bridge/src/dialog_timeline_full2.rs`; lib.rs/Cargo.toml/timeline_dialog.rs untouched; no cargo/commit.
Tests: 6 in-file (empty-all, cap128, case-insensitive, count, cursor_line, status).
Decisions: `filtered` clones Strings; `count`=filtered len; `status` capped 128 chars; std-only, forbid(unsafe_code).
Unknowns: none.
