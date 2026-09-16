# PROV-019 worklog (verify-only, dedicated)

## Claim
`crates/providers/src/request_profile.rs` (412 lines) satisfies `tasks/PROV-019.md` (documented request profiles). Frozen suite `prov_019_request_profile.rs` 5/5 GREEN. GREEN-on-first-run, no independent RED. Verifier decides.

## Source evidence
- Base rev `248f519`. `tasks/PROV-019.md:8-9` owns `request_profile.rs` only.
- Impl: RequestProfile, AuthHeaderKind, RequestError, RedactedDiagnostic, profile_for, headers_for, with_diagnostics.
- Wired `lib.rs:48`. Precedents config/registry/model_route/auth/oauth_flow/integration.

## Observed scenario
Suite authored vs public API, passed first run vs existing code. No valid RED (bundle record `PROV-017-024.md`).

## Target boundary
- Frozen test untracked, 5 #[test]; sha256 `04bad144c0ae2ddcdeac73861688adabca706cd972cbe8d8ad1beaa1ecef302f` (bundle-frozen `092e665d...`; drift noted, GREEN).
- Zero edits.

## Tests
- Rerun batch 017–020 → 20 passed, 0 failed. Serial JOBS=2 THREADS=2 timeout 120.

## Decisions
- Verify-only. Redacted diagnostics leak-scan per card.

## Remaining unknowns
- Acceptance verifier-owned. ralph.json untouched.
