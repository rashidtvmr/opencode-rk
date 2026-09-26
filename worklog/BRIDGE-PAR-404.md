# BRIDGE-PAR-404 scratchpad (UNCLAIMED - orchestrator owns claims.json)

Task: BRIDGE-PAR-404. Status: unclaimed (no ledger touch per spawn order; file-only lane).
Claim: skipped by explicit order. Do NOT touch tasks/completion/claims.json.
Owner file: crates/opentui-bridge/src/sess_tl_dlg_full.rs only.
Forbid: lib.rs, Cargo.toml, dialog_timeline_full2.rs edits; no cargo, no commit.

Source evidence:
- TS truth packages/tui/src/routes/session/dialog-timeline.tsx:10 DialogTimeline, :22-44 user-role text rows, :42 reverse newest first.
- Rust kin crates/opentui-bridge/src/timeline_dialog.rs:19 TimelineDialog entries/cursor/open, :48 wrap move_cursor, :57 selected.
- Rust kin crates/opentui-bridge/src/dialog_timeline_full2.rs:14 TimelineFull filter wrapper (do not duplicate; TlDlg is bare capped list).

Target boundary: TlDlg {items cap 32 each 256 chars, cursor} + push (trunc, evict oldest) + move_cursor (wrap) + selected. std-only, forbid(unsafe_code), <100 lines, >=4 tests.

Tests: push_truncates, push_evicts_oldest_at_cap, cursor_wraps, selected_none_when_empty.
Decisions: evict-oldest on push (matches append-then-cap kin); newest-first ordering left to caller via push order (ponytail: no reverse/insert API).
Remaining: rustfmt --check only per order; no cargo test/build.
