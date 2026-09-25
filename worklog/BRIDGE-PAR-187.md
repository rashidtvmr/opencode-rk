# BRIDGE-PAR-187 scratchpad

claim: BRIDGE-PAR-187 via cc.claim, session ses_par187, ok.
source: crates/opentui-bridge/src/runtime_shared_full.rs:11-68 (RuntimeSharedFull: model/busy/turns, start_turn/end_turn/status); TS truth packages/opencode/src/cli/cmd/run/turn-summary.ts:5-47 (turnSummaryCommit "agent · model · duration").
boundary: ONE file crates/opentui-bridge/src/turn_wire.rs. No lib.rs/Cargo.toml edits.
pattern ref: focus_wire.rs thin-wire + fail-closed style.
tests: 5 tests (begin_finish_cycle, turn_line_format, turn_line_empty_model, turn_line_caps_256, double_finish_saturates).
decisions: delegating begin/finish to rt; turn_line "model turns=N busy?" char-capped 256; forbid(unsafe_code), std-only.
evidence: rustfmt --check EXIT=0, wc 102 lines (<120).
