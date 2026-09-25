# FIX-08 scratchpad

Claim: CtxKind 22-variant registry.
Source: ctx_bundle.rs:18-22 (CtxBundle facade), context_engines_full.rs:16-55 (22-engine bounded registry companion), context_stores.rs:30-297 (plain-state mirrors).
Target: crates/opentui-bridge/src/ctx_registry_full.rs only. lib.rs untouched.
Tests: count_matches, spot_unique, copy_eq.
Done: rustfmt --check EXIT 0, 100 lines, 3 tests, std-only, forbid(unsafe_code).
Remaining: lane gate (not run per scope: no cargo/commit).
