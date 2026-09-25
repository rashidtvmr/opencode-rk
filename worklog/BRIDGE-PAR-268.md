# BRIDGE-PAR-268 scratchpad

Claim: BRIDGE-PAR-268 via cc.claim, session ses_par268. OK.

Source evidence:
- crates/opentui-bridge/src/question_view.rs:21 QuestionView (title/options/cursor/answered) - read only.
- crates/opentui-bridge/src/question_gate.rs:8 QuestionGate { full: QuestionFull } + ask_single/toggle/answers - read only.
- crates/opentui-bridge/src/question_full.rs:21 QuestionFull (single/multi toggle, done) - read only.
- Pattern: crates/opentui-bridge/src/dialog_message_full.rs:72 lines(width) + MAX_ROWS cap, char-safe clip.

Target boundary: ONE new file crates/opentui-bridge/src/question_view_full.rs. No lib.rs/Cargo.toml/question_view.rs/question_gate.rs edits. No cargo, no commit.

Tests: 4 in-file (empty-hint-only, answer-joined-before-hint, width-clip, cap-8 + width-0-safe).

Decisions: QuestionRender { pub gate: QuestionGate } + lines(width) (joined answers + "enter to confirm" hint, char-clip, truncate 8) + count() (answers len). std-only, forbid(unsafe_code), 89 lines.

Verification: rustfmt --check crates/opentui-bridge/src/question_view_full.rs -> clean (FMT_OK).

Remaining: none. Unwired into lib.rs by design (other lane owns it).
