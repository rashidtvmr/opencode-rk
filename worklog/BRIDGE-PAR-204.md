# BRIDGE-PAR-204 quit_gate

- Claim: ses_par204 via completion_claims, status in-progress.
- Source: crates/opentui-bridge/src/exit_ctx.rs:11-15 ExitCtx (read-only truth, not edited).
- Observed: no quit_gate.rs existed; two-press gate needed.
- Boundary: one owned file crates/opentui-bridge/src/quit_gate.rs; lib.rs/Cargo.toml/exit_ctx.rs untouched; no cargo/commit.
- Tests: 5 in-file (first_arms_second_quits, cancel_disarms, quit_on_quit_when_armed, quit_on_rejects_other_labels, quit_on_false_after_cancel).
- Decision: request() false-first-true-second; quit_on takes &self, exact "quit" && armed; field `confirming` private.
