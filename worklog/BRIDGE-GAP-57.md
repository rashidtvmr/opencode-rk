# BRIDGE-GAP-57 loop_driver.rs
Claim: BRIDGE-GAP-57 / ses_gap57.
Source evidence:
- crates/opentui-bridge/src/native_input.rs:1-76 (InputChunk, Page, map_event/map_key, Enter=empty SubmitText, MAX_TEXT 4096).
- crates/opentui-bridge/src/tui_entry.rs MISSING from tree (glob shows no tui_entry.rs); header cites tui_entry.rs:626-672 per task card as read-only ref.
- crates/opentui-bridge/src/lib.rs declares loop_events, native_frame; no native_input mod line (untouchable per scope); loop_driver refs crate::native_input.
Observed: no loop_driver.rs; tui_entry.rs absent, match-arm mirror taken from task spec + native_input Enter convention.
Target boundary: ONE new file crates/opentui-bridge/src/loop_driver.rs only. No lib.rs/Cargo.toml edits, no cargo, no commit/push.
Tests: in-file #[cfg(test)] 8 tests (quit, page, append, backspace, paste cap, transcript cap, draft cap, resize/noop).
Decisions: LoopStep = InputChunk alias; step(&mut, chunk); empty SubmitText with non-empty draft = Enter submit (push transcript cap 500, clear draft); take_transcript drains via mem::take.
Unknowns: tui_entry.rs:626-672 unverifiable (file absent); integrator must add `pub mod loop_driver;` (+`pub mod native_input;` if missing) to lib.rs.
