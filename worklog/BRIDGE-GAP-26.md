# BRIDGE-GAP-26 scratchpad

claim: ses_gap26 owns BRIDGE-GAP-26, file crates/opentui-bridge/src/run_question.rs
source: run_replay.rs (style mirror); task spec stands in for TS question.shared.ts (not in repo)
target: QuestionTabs {tabs cap 8, selected, answers, confirmed} + select/answer/confirm/submit
tests: 6 (cap, select OOB, answer len, confirm gate, submit none, submit clears)
decisions: OOB ops ignored; select/answer reset confirm (stale confirm unsafe); submit clears slots + gate; std-only; 180-line cap
