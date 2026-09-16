# PROV-018 worklog (verify-only, dedicated)

## Claim
`crates/providers/src/local_credential_import.rs` (323 lines) satisfies `tasks/PROV-018.md` (consent-gated local credential import). Frozen suite `prov_018_local_credential_import.rs` 5/5 GREEN. GREEN-on-first-run vs existing code (no independent RED). Verifier decides.

## Source evidence
- Base rev `248f519`.
- `tasks/PROV-018.md:8-9`: owns `local_credential_import.rs` only.
- Impl: ImportSource, UserConsent, CredentialKind, ImportPlan, ImportError, validate_schema, check_permissions, plan_import.
- Wired `lib.rs:36`. Precedents `auth.rs:6-18,40-101`, `integration.rs:1-18`.

## Observed scenario
No `prov_0*` tests existed before lane; suite authored vs public crate API (not `#[path]`), passed first run vs existing code. Per TDD §3 no valid RED for this slice (only PROV-017 in bundle has RED history). Bundle worklog `PROV-017-024.md` is the RED/GREEN record.

## Target boundary
- Frozen test untracked, 5 #[test]; current sha256 `f14ac9aa86d4fea235d6109ec959b065587814d703ee77c5d05763fb2cf755a4` (differs from bundle-frozen `f459c586...`; drift noted, behavior GREEN).
- Zero edits this lane.

## Tests
- Rerun 2026-09-16 in 4-suite batch (017/018/019/020) → 20 passed, 0 failed.
- Isolation T04 secret-scan; T05 determinism; `tempfile` only, no DB.
- Serial JOBS=2 THREADS=2, timeout 120.

## Decisions
- Verify-only rerun; no weakening. Imported material never persisted by slice (lands in AuthHandler memory).

## Remaining unknowns
- Acceptance verifier-owned. ralph.json untouched.
