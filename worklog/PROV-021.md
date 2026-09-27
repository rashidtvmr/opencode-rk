# PROV-021 scratchpad

Claim: PROV-021 claimed by ses_f384fee20ffertocB89srK0rm7, scratchpad worklog/PROV-021.md. Ledger claim OK (no collision).

Source evidence (HEAD 0a3ea6c):
- tasks/PROV-021.md:1-63 contract, caps, failure states, T01-T05.
- crates/providers/src/usage_status.rs:1-310 existing impl (UsageKey/Counters/Delta/Entry/Snapshot/Error/Store, record/snapshot/reset, caps 256/10M, saturating math, BTreeMap deterministic order, no I/O/clock/net).
- crates/providers/tests/prov_021_usage_status.rs:1-151 frozen T01-T05 (record happy path, order+reset, overflow/delta/saturation, validation, determinism/isolation).
- crates/providers/src/lib.rs:58 `pub mod usage_status;` pre-wired by integrator.
- Referenced: config.rs:7-22 ProviderConfig, registry.rs routing, model_route.rs MAX_COMBO_MODELS, auth.rs:6-18 AuthMethod/:40-101 AuthHandler, integration.rs:1-5,11-18 bounded constants, docs/SECURITY.md:14-32, REQ-041 research doc.

Observed scenario: impl file pre-exists at HEAD (not authored by this lane). Task: implement/verify bounded secret-free snapshots. If focused suite pre-existing GREEN, record receipt gap, no invented RED.

Target boundary: owned file crates/providers/src/usage_status.rs only. No test/lib.rs/Cargo edits. Scratchpad + ledger row only extras.

Tests: focused cmd `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 180 rtk cargo test -p opencode-rk-providers --test prov_021_usage_status -- --test-threads=1`. Frozen tests immutable, zero edits.

Decisions: verify-first; fix impl only if failures; no stubs/todo; no secrets/env/net/fs in module (caller owns store lifetime; caller supplies timestamps).

Verification (no impl edits — impl pre-complete at HEAD 0a3ea6c):
- Focused: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-providers --test prov_021_usage_status -- --test-threads=1` → 5/5 GREEN (T01-T05), zero test edits.
- `cargo check -p opencode-rk-providers` → Finished dev, 0 errors. Warnings pre-existing outside owned file (auth.rs unused Duration, one unused thiserror import elsewhere); owned file warning-free.
- Secret/IO grep on usage_status.rs (`std::{fs,env,time,net,process,thread}`, tokio, Command, unsafe, todo!, unimplemented!, sk-, Bearer, secret, env!): zero hits (exit 1).
- Frozen test sha256: 5881c079922d647d0bc9990ec75ad20641dc1e52aecab4cfd6789cae3d169d4f. Impl sha256: 3c483814cd7dbde6e9f0566839e5629a69195800e3932c5eeb1945bf60b03524.
- RED receipt gap: tests+impl both pre-existed at HEAD; lane did not author RED (no invented RED per orders). Pre-existing GREEN verified, not claimed as own RED/GREEN cycle.
- lane_gate.py covers storage v2 lanes only — no task-scoped invocation exists for PROV-021; focused cargo test used instead.

Caller wiring gap: `pub mod usage_status;` pre-wired at lib.rs:58, but no product caller (status panel/CLI/daemon) consumes record/snapshot/reset yet — wiring left to integrator per card. Verifier decides acceptance.
