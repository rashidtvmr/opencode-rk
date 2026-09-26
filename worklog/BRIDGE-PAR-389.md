# BRIDGE-PAR-389 (unclaimed: orchestrator owns claims.json, file-only lane)

Claim: none (per task: do NOT touch claims.json).
Source: logo.ts:1-4 (left/right halves) + logo_art.rs:17-30 (verbatim halves); layout presentation.ts:25 left+gap+right.
Target: crates/opentui-bridge/src/logo_ts_full.rs only; lib.rs/Cargo.toml/logo_art.rs/logo_full.rs untouched.
Tests: rows_and_oob, exact_join, width_is_39 (all in-file, 3 tests).
Decisions: precomputed joined const (4x39 chars), pad empty, gap single space; logo_line OOB returns "".
Unknowns: none. Verification: rustfmt --check PASS, 47 lines (<60).
