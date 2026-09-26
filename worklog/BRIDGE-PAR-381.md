# BRIDGE-PAR-381 (unclaimed, file-only per orchestrator)

Claim: skipped ledger claim (orchestrator owns claims.json; instructed proceed file-only).
Source: kv.tsx:1-40 (global store get/set over kv.json); context_kv.rs:30-89 (flat KvStore, MAX_KV 256); kv_toggles.rs:1-11 (bool toggles beside KvStore).
Boundary: ONE new file crates/opentui-bridge/src/ctx_kv_full.rs. No lib.rs/Cargo.toml edits. No cargo/commit.
Tests: 5 unit tests in-file (roundtrip, overwrite, missing_none, cap_64, empty_len).
Decisions: overflow new keys ignored (fail-closed cap); no delete/validation (YAGNI).
Unknowns: wiring into lib.rs left to integrator.
