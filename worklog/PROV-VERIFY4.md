# PROV-VERIFY4 worklog (verify-only, PROV-015..024)

## Claim
Verify-only rerun. Zero product/test/config edits. Fix-present: YES.
`claude_oauth.rs:462` emits raw `{LOOPBACK_REDIRECT_URI}`. 11 suites, 56 tests GREEN. Log `/tmp/opencode/yG-prov.log`. Verifier decides acceptance.

## Source evidence
- Rev `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b` (`git rev-parse HEAD`).
- `crates/providers/src/claude_oauth.rs:32` const `LOOPBACK_REDIRECT_URI = "http://127.0.0.1:1455/oauth/callback"`.
- `crates/providers/src/claude_oauth.rs:461-464` (read-only): `begin_login` interpolates raw `{LOOPBACK_REDIRECT_URI}` into `{CONSENT_ORIGIN}?redirect_uri=...&code_challenge=...&code_challenge_method=S256`, passes own `validate_consent_url` -> `contains_loopback_redirect` (`:621-647`). Line 462 verified by read.
- Uncommitted diff: 1 file, +1/-1 (percent-encoded literal -> const interpolation). Pre-existing from prior lane; not authored here.
- `PLAN.md:162-180` TDD RED/GREEN/freeze rules; `docs/TDD.md:43-66` RED-must-compile-and-fail + freeze-hash; `docs/SECURITY.md:24-32,62-72` no-secret-logging, disposable fixtures, fake grants.
- No `todo!/unimplemented!/stub` (case-insensitive grep, 8 owned src files): 0 hits.
- Frozen files untouched: `git status --porcelain` shows the 11 owned test files as untracked `??` (frozen, never edited); `lib.rs`, `ralph.json` show no modification. Zero edits this lane.

## Observed scenario
Serial verify-only reruns (`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`, `timeout 120`, `--test-threads=1`), one target at a time. Avail ~2.5Gi at start. All 11 suites GREEN, matching GATE3 hashes (sha256 re-verified: auth_profile `a23f9f32...`, codex_oauth `aa7459f6...`, bounds `9d2c8d87...`, prov_017 `b5a9cb29...`, prov_018 `f14ac9aa...`, prov_019 `04bad144...`, prov_020 `8d74c4ff...`, prov_021 `5881c079...`, prov_022 `22132855...`, prov_023 `257ef2ea...`, prov_024 `ba39df08...`).

## Target boundary
- Owned/verified only: 11 frozen test targets (10x5 + bounds 6). No new files, no edits.
- Explicitly NOT touched: `crates/providers/src/*`, `lib.rs`, `Cargo.toml`, `tasks/*`, `ralph.json`, any other file.

## Tests (11 suites = 56 GREEN, log /tmp/opencode/yG-prov.log)
| suite | target | result |
|---|---|---|
| auth_profile (PROV-015) | --test auth_profile | 5/5 |
| codex_oauth (PROV-016) | --test codex_oauth | 5/5 |
| codex_oauth_bounds | --test codex_oauth_bounds | 6/6 |
| claude_oauth (PROV-017) | --test prov_017_claude_oauth | 5/5 |
| local_credential_import (PROV-018) | --test prov_018_local_credential_import | 5/5 |
| request_profile (PROV-019) | --test prov_019_request_profile | 5/5 |
| auth_commands (PROV-020) | --test prov_020_auth_commands | 5/5 |
| usage_status (PROV-021) | --test prov_021_usage_status | 5/5 |
| auth_store (PROV-022) | --test prov_022_auth_store | 5/5 |
| provider-compatibility (PROV-023) | --test prov_023_provider_catalog | 5/5 |
| contracts (PROV-024) | --test prov_024_provider_contracts | 5/5 |

Total: 56 passed, 0 failed.

## Decisions
- Verify-only: fix already present, no edit needed or made.
- `ponytail:` prior-lane percent-encode vs raw-redirect detail kept inline; no cross-file refactor. Upgrade path: none needed.

## Remaining unknowns
- Acceptance verifier/controller-owned. Workdir has sibling-lane modifications outside owned paths; none touched here.
