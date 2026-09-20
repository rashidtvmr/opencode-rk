# FIX-TIMELINE scratchpad

claim: FIX-TIMELINE session ses_fix_timeline (pre-claimed in ledger).
commit: 62f7eb1.
evidence:
- crates/cli/src/native_timeline.rs:52 TimelineBuilder, :145 render_page(page,height,width)->Vec<String> bounded MAX_PAGE=200, :159 TimelinePage scroll.
- crates/cli/src/native_shell.rs:108 ShellPages, :124 push_transcript, :137 transcript_window; pure state, MAX_LINES=500, MAX_LINE=1024.
- crates/cli/src/main.rs:42-44 mod native_timeline/native_transcript/native_shell.
- native_transcript wired main.rs:43 (mod decl); no paint_timeline exists in shell (gap).
target: ShellPages::paint_timeline in native_shell.rs only. Calls builder.render_page(page,height,width), pushes rows via push_transcript. Bounded: render_page<=MAX_PAGE rows, buffer<=MAX_LINES, chars<=MAX_LINE. No renderer/daemon touch.
tests: frozen native_timeline + native_shell untouched; verify via cargo test --bin oc2 native_timeline (or bins check).
decisions: bold=false for timeline rows; scroll owned by caller TimelinePage.
impl done: added `use crate::native_timeline::{TimelineBuilder, TimelinePage}` + ShellPages::paint_timeline(builder,page,height,width)->usize. Owned-file only diff +25 lines.
verify:
- cargo test -p opencode-rk-cli --bin oc2 native_shell => ok 9/9 (all HEAD shell tests green).
- cargo test -p opencode-rk-cli --bin oc2 native_timeline => 10/11; t07_page_pins_to_bottom_and_scrolls FAILS left [2,3] right [1,2] at native_timeline.rs:511. PRE-EXISTING: reproduced with owned file stashed (HEAD code fails identically). Frozen test untouched per boundary; bug lives in native_timeline.rs `page()` (end=len-scroll shifts on push instead of pinning visible anchor) — NOT this lane's owned file, so left for its owner.
- cargo check -p opencode-rk-cli --bins => 0 errors.
unknowns: none.
verify(2026-09-20): native_shell 9/9 GREEN (oc2 bin). native_timeline 10/11; t07_page_pins_to_bottom_and_scrolls FAILS left [2,3] right [1,2] at native_timeline.rs:511. Pre-existing: page() end=len-scroll shifts anchor on push; owned file native_shell.rs untouched by that path. DO NOT fix native_timeline.rs here (owner: timeline lane). Zero test edits.
fix(2026-09-20, ses_f4037c6c1ffeFTarRZJvWavVSb): anchor bug fixed in native_timeline.rs ONLY. TimelinePage gains `anchor: usize` recorded on leaving pinned state; page() renders anchor-scroll when scrolled (len when pinned); scroll_down to 0 / reset re-pins. Ledger claim fenced by ses_f40414a21ffeobxGRbBXCFCI7M so no ledger update by me.
verify: CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 120 cargo test -p opencode-rk-cli --bin oc2 native_timeline => 11/11 ok. native_shell => 9/9 ok. Zero test edits (git diff shows no test files touched by this fix).
verify(2026-09-20, ses_f40321fd7ffeHN2p2HkWBucRbO): anchor fix already in worktree, no re-edit needed.
- CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 120 cargo test -q -p opencode-rk-cli --bin oc2 native_timeline => 11 passed, 0 failed.
- same cmd native_shell => 9 passed, 0 failed.
- zero test edits: git diff touches no tests/ or crates/cli/tests; owned diff native_timeline.rs only (+29/-8 anchor).
- no commit/push per lane bounds.
