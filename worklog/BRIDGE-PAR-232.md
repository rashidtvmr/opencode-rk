# BRIDGE-PAR-232 scratchpad

- Claim: BRIDGE-PAR-232 via ses_par232, scratchpad worklog/BRIDGE-PAR-232.md.
- Source evidence:
  - crates/opentui-bridge/src/page_router.rs:9 PageRouter { page: Page }, label() via page_label.
  - crates/opentui-bridge/src/transcript_store.rs:15 TranscriptStore { lines: Vec<String> }, len(), push().
  - crates/opentui-bridge/src/draft_store.rs:10 DraftStore { text, cursor }, new().
- Target boundary: ONE new file crates/opentui-bridge/src/tui_state_full.rs. No lib.rs/Cargo.toml/other edits. No cargo, no commit.
- Tests: 6 unit tests in-file (new_defaults, tick_increments, tick_wraps, status_reports_page_and_count, status_empty_zero, status_caps_256).
- Decisions: status format `"{label} ({n} msgs)"`, char-truncated to 256. tick uses wrapping_add(1).
- Remaining: rustfmt --check, ledger flip.
