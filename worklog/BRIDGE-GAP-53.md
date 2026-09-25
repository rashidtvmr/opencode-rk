# BRIDGE-GAP-53

Claim: BRIDGE-GAP-53, session ses_gap53.
Source: crates/opentui-bridge/src/run_footer.rs (FooterView Idle/Streaming/Done, queue), crates/opentui-bridge/src/run_types.rs (FooterPhase Prompt/Permission/Question/Subagent/Done, FooterView{phase,busy}).
Target: crates/opentui-bridge/src/run_footer_view.rs only. No lib.rs/Cargo.toml edits.
Tests: 8 in-file #[cfg(test)]: prompt/permission/question/subagent/done map, unknown idle, queue streaming, queue empty idle.
Decisions: FooterSurface union of both shapes + Streaming; Default=Idle; from_run_types match on prompt/permission/question/subagent/done else Idle; from_queue const fn. std-only, forbid(unsafe_code), 148 lines.
Evidence: rustfmt --check FMT_OK.
Unknowns: none; wiring into lib.rs is orchestrator/integration lane.
