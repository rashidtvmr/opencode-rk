# BRIDGE-GAP-74
Claim: ses_gap74, ledger completed.
Source: question.shared.ts (questionSelect/pick idea), run_question.rs (cap/OOB style, NOT edited).
Target: crates/opentui-bridge/src/run_question_shared.rs only.
Tests: 6 in-file (pick ok, OOB false, none default, picked label, cap 8, trunc caps).
Decisions: std-only, forbid(unsafe_code), 141 lines, rustfmt FMT_OK, no cargo per scope.
Unknowns: lib.rs wiring left to orchestrator (out of scope).
