# BRIDGE-GAP-45

Claim: QuestionState tabs/answers/custom/multi in `crates/opentui-bridge/src/question_state.rs`.
Source: `crates/opentui-bridge/src/run_question.rs:1-146` gate conventions (TAB_CAP=8, OOB ignore, confirm-gated submit clears).
Target: QuestionState {tabs cap 8, answers, custom cap 512, multi} + select/answer OOB false + set_custom truncates + confirm (all answered or custom) + submit Option clears.
Tests: 6 in-file (cap, OOB x2, custom bypass, confirm gate, submit clears).
Decisions: select returns bool not mutating (spec); submit custom-wins single vec; forbid(unsafe_code), std-only.
