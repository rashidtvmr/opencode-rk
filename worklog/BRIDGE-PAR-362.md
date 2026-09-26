# BRIDGE-PAR-362 (unclaimed, file-only per delegation)

Claim: implement SessQues state machine for session question prompt.
Source evidence:
- TS truth: packages/tui/src/routes/session/question.tsx:14 `QuestionPrompt(props: {request: QuestionRequest})`, :22-31 tab/answer/selected store, :48-50 submit via sdk question.reply.
- Sibling: crates/opentui-bridge/src/route_session.rs:32 `SessionRoute`, crates/opentui-bridge/src/question_state.rs:16 `QuestionState` (TAB_CAP 8, CUSTOM_CAP 512).
Target boundary: ONE new file crates/opentui-bridge/src/route_sess_ques_full.rs; lib.rs/Cargo.toml untouched; no cargo/commit.
Tests: 3 in-file unit tests (cap+clear, pick roundtrip, empty title). rustfmt --check PASS.
Decisions: ask() truncates title to 128 chars and resets picked (new question resets selection, matches TS single-select tab reset); pick() stores index without bounds (option count lives in QuestionState/tabs layer); std-only, forbid(unsafe_code), 61 lines.
Remaining: wiring into lib.rs left to orchestrator.
