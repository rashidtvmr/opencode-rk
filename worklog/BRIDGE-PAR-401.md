# BRIDGE-PAR-401 (unclaimed, file-only per orchestrator)

Claim: none (orchestrator owns claims.json; instructed proceed file-only).
Source: packages/tui/src/routes/session/dialog-fork-from-timeline.tsx:12 DialogForkFromTimeline full-session pick; sibling session_fork_dialog.rs (ForkDialog msg pick+confirm), dialog_fork_full2.rs (ForkPick cursor).
Boundary: ONE new file crates/opentui-bridge/src/sess_fork_dlg_full.rs. No lib.rs/Cargo.toml edits. std-only, forbid unsafe, <80 lines.
Tests: 4 unit tests (open clears dst, blank dst fails, ok exposes dst, src cap 128).
Decisions: struct ForkDlg {src,dst} + open/confirm->bool/dst_of; confirm rejects blank src/dst; truncate both to MAX_ID=128.
Unknowns: wiring into lib.rs left to orchestrator.
