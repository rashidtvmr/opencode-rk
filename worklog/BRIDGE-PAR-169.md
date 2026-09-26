# BRIDGE-PAR-169

Claim: BRIDGE-PAR-169 via ses_par169, scratchpad worklog/BRIDGE-PAR-169.md.
Source: runtime.shared.ts (reusePendingTask slot) + run_runtime_shared.rs:1-146 (RuntimeTable, caps 64/16).
Target: crates/opentui-bridge/src/runtime_shared_full.rs only. No lib.rs/Cargo.toml edits.
Tests: in-file #[cfg(test)] 7 tests (empty false, over-cap false, ok, double-start, end bumps, saturate, status).
Decisions: std-only, forbid(unsafe_code), getters model()/is_busy()/turns(), status format model=X busy=B turns=N.
Unknowns: wiring into lib.rs left to orchestrator (out of scope).
