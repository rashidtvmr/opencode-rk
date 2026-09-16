# PROV-015 worklog (verify-only, dedicated)

## Claim
`crates/providers/src/auth_profile.rs` (255 lines) satisfies `tasks/PROV-015.md` contract. Frozen suite `crates/providers/tests/auth_profile.rs` 5/5 GREEN. No valid RED (product predates tests). Verifier decides acceptance.

## Source evidence
- Base rev `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b` (clean HEAD; worktree has sibling-lane mods, none mine).
- `tasks/PROV-015.md:8-9`: owns `auth_profile.rs` only; lib.rs/Cargo.toml/schemas untouched.
- Impl `crates/providers/src/auth_profile.rs:12-255`: AuthProvenance{ApiKey,OfficialOAuth,ImportedLocal,Keyring}, profile_of/describe/redacted_debug, secret-free Debug/Serialize.
- Wired `crates/providers/src/lib.rs:8` (`pub mod auth_profile;`, integrator-owned, untouched).
- Precedents `auth.rs:6-18,40-101`, `integration.rs:1-18`, SECURITY.md broker rules. Read-only.

## Observed scenario
Product module predates lane (batch commit `248f519`). No T01..T05 coverage existed before (`--lib auth_profile` matched 0). New integration target first run GREEN 10/10 (with PROV-016). Per TDD §3 NOT valid RED. Recorded GREEN-only regression evidence. True RED (fail: no module) satisfiable only by deleting sibling artifact — refused.

## Target boundary
- Owned/new: `crates/providers/tests/auth_profile.rs` (untracked, 5 #[test]).
- Explicitly NOT touched: `src/*`, lib.rs, Cargo.toml, tasks/*, ralph.json, any other file. Zero edits this lane.

## Tests
- Current sha256 test file: `a23f9f322c959260d88fd0cbf033ce605b064ae8add93b843440a0bbd586756e` (differs from bundle-lane frozen `ac548d6e...`; file untracked, drifted since bundle — content verified 5 #[test], behavior GREEN).
- Rerun 2026-09-16: `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-providers --test auth_profile --test codex_oauth -- --test-threads=2` → 10 passed, 0 failed (2 suites).
- RED status: NO valid RED established. GREEN-only.
- Mem: ~1.8Gi avail; serial, timeout 120.

## Decisions
- Verify-only: no impl/test edits (would violate frozen-test + ownership rules).
- `ponytail:` exact 128/129-char boundary + leak-scan detail duplicated inline, no sibling-helper imports. Self-contained evidence.

## Remaining unknowns
- Acceptance verifier/controller-owned. ralph.json status untouched (`not-started`).
- No PROV lane-gate target in `tools/lane_gate.py`; verifier runs test target explicitly.
