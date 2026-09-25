# BRIDGE-PAR-185 scratchpad

Claim: BRIDGE-PAR-185 ses_par185 in-progress.
Source: crates/opentui-bridge/src/question_full.rs:21 QuestionFull {id,header,opts,multi,picked}, new:31 toggle:43 done:62. question_view.rs:21 QuestionView single-select latch (reference only).
Target: crates/opentui-bridge/src/question_gate.rs QuestionGate{full} + ask_single + toggle + answers. std-only, forbid(unsafe_code), <120 lines, >=4 tests.
Tests: ask_single_empty, toggle_selects, toggle_oob_false, answers_labels (+single-replace).
Decision: id=header=title; multi=false; delegate toggle->full.toggle, answers->full.done.
