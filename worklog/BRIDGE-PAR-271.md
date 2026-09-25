# BRIDGE-PAR-271

Claim: ledger claim ses_par271 in-progress OK.
Source: crates/opentui-bridge/src/command_shim.rs:1-78 (CommandShim, MAX_NAME_LEN 64, MAX_COMMANDS 64, truncate, invoke pure). Read-only, untouched.
Target: crates/opentui-bridge/src/command_shim_full.rs (new, owned). ShimFlow {names cap 64 each 64, runs u64} + register(&str)->bool + run(&str)->bool (known bumps runs) + runs()->u64 + names(). std-only, forbid(unsafe_code), 108 lines.
Tests: 4 in-file (register_run_bump, run_unknown_no_bump, register_rejects_empty_dup_full, truncates_long_name).
Decisions: char-based truncation (not byte slice; avoids panic on non-ASCII). saturating_add for runs. Default-derived new.
Verification: rustfmt --check PASS (FMT_OK), no cargo/commit per scope.
Unknowns: none.
