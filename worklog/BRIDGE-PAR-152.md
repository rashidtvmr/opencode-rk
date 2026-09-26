# BRIDGE-PAR-152 exit_ctx.rs
Claim: ses_par152, ledger in-progress.
Evidence: exit.tsx:1-8 Exit=(reason?: unknown)=>void; run_lifecycle.rs:130-134 CloseReason Done/Error/Interrupted.
Boundary: ONE file exit_ctx.rs; lib.rs/Cargo.toml/run_lifecycle.rs untouched.
Tests: 6 in-file (default, ask, trunc, confirm, cancel, status). rustfmt --check PASS.
Decisions: pop-loop cap (char-boundary, mirrors run_lifecycle.rs:159); getters added for test access.
Unknowns: none.
