# BRIDGE-PAR-165 scratchpad

Claim: BRIDGE-PAR-165 via cc.claim session ses_par165. Owned file: crates/opentui-bridge/src/question_full.rs.
Source evidence:
- TS truth packages/opencode/src/cli/cmd/run/question.shared.ts:195-205 questionToggle (push/splice by label), 219-256 questionSelect (info.multiple toggles else pick/replace), 308-313 questionSubmit (answers per tab).
- crates/opentui-bridge/src/run_question_shared.rs:16-18 trunc chars().take, 7 OPTION_CAP=8, 10-14 ID/LABEL/TITLE caps, 57-64 pick OOB false (DO NOT EDIT).
Observed: QuestionCard single-pick Option<usize>; task needs multi toggle set + done labels + reset.
Target boundary: ONE new file question_full.rs. No lib.rs/Cargo.toml/run_question*/question_view edits. No cargo/commit.
Tests: in-file #[cfg(test)] 7 tests (toggle on/off, single replaces, bounds false, done labels, reset, caps).
Decisions: index-based picked Vec<usize> (label resolve in done, mirrors TS answers-as-labels); single toggle replaces vec![i]; multi push/remove position, PICK_CAP 8 guard; std-only, forbid(unsafe_code).
Unknowns: none. Wiring into lib.rs left to orchestrator.
