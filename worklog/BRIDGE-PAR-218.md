# BRIDGE-PAR-218 scratchpad
- claim: ses_par218 ok (in-progress)
- source: footer.question.tsx:1-60 tabbed multi-question + Confirm tab, state in question.shared.ts; crate question_gate.rs:8 QuestionGate{full} ask_single/toggle/answers; question_full.rs caps OPT_CAP 8/PICK_CAP 8
- boundary: ONE file run_question_full.rs only; no lib.rs/Cargo.toml/question_full.rs/question_gate.rs edits; no cargo/commit
- target: QuestionFlow{gates cap 8}+ask->bool+answer->bool+done_answers; std-only forbid(unsafe) <130L >=5 tests; rustfmt --check only
- tests: 6 in-file (new_empty, cap, routes, oob, replaces, order)
- decision: Vec<QuestionGate> via ask_single, FLOW_CAP 8 mirrors OPT_CAP; ponytail single-select note
