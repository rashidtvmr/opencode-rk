# BRIDGE-GAP-110 scratchpad (ses_gap110)

Claim: append AnswerKind + QuestionFlow to run_question.rs, existing items byte-identical.
Source: crates/opentui-bridge/src/run_question.rs:1-146 (QuestionTabs, TAB_CAP, tests mod 86-146 frozen).
TS truth: packages/opencode/src/cli/cmd/run/footer.question.tsx:1-120 (confirm/submit verbs, questionConfirm/questionSubmit state machine).
Target: only APPEND to run_question.rs. No lib.rs/Cargo.toml/run_question_shared.rs edits. No cargo. No commit.
Tests: new mod tests2 >=5 (submit-needs-confirm, double-submit, reset, labels).
Decisions: AnswerKind::text() truncates chars to 512; QuestionFlow pub bool fields; state_label pending/confirmed/submitted; submit latches submitted, second false; reset clears both.
Unknowns: none.
