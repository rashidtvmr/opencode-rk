# BRIDGE-PAR-273 scratchpad

claim: BRIDGE-PAR-273 via cc.claim, session ses_par273. OK.
source: packages/tui/src/component/dialog-agent.tsx:6 DialogAgent, :10-18 options from local.agent.list, :23 current name, :25-28 set+clear. Pattern: dialog_select.rs:1 forbid unsafe, struct+cursor+tests.
target: crates/opentui-bridge/src/dialog_agent_full.rs only. No lib.rs, no Cargo.toml.
tests: 6 unit tests (accept/select-first, reject bad+dup, cap 32, clamp ends, signed steps, empty none).
decisions: Vec<String> cap 32, per-name 64 chars, dup reject, cursor clamp isize delta.
evidence: rustfmt --check PASS, 113 lines (<120).
