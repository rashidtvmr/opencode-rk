# PROV-017 worklog (verify-only, dedicated)

## Claim
`crates/providers/src/claude_oauth.rs` satisfies `tasks/PROV-017.md` (Claude OAuth consent machine). Frozen suite `prov_017_claude_oauth.rs` 5/5 GREEN. Valid RED history exists (4/5 fail pre-fix `BadConsentUrl`). Verifier decides acceptance.

## Source evidence
- Base rev `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`.
- `tasks/PROV-017.md:8-9`: owns `claude_oauth.rs` only.
- Impl `crates/providers/src/claude_oauth.rs` (720 lines); fix at `:461-463` emits raw `{LOOPBACK_REDIRECT_URI}` so self-emitted URL passes own `validate_consent_url` → `contains_loopback_redirect` (`:621-647` delimiter rule).
- Wired `lib.rs:12`. One-line product fix present as uncommitted worktree diff (`git status`: Modified `claude_oauth.rs` only among impl files); tests untracked.
- Full history in `worklog/PROV-017-024.md` incl. regression section + W2 confirm.

## Observed scenario
RED baseline (committed base + frozen tests): 1 passed / 4 failed (`t01,t03,t04,t05` `BadConsentUrl`, t02 pass). Root cause: percent-encoded `redirect_uri=http%3A%2F%2F127.0.0.1%3A...` has `f` (from `%2F`) before `127.0.0.1`, failing delimiter check. Fix: emit raw constant. Not sibling churn (`git log --oneline -5 -- claude_oauth.rs` = only `248f519`).

## Target boundary
- Frozen test: `crates/providers/tests/prov_017_claude_oauth.rs` (untracked, 5 #[test]); sha256 `b5a9cb2990bf471b16ac6424f9843d7401a0ce5318404fc95aac35b9beadc4f5` — matches lane-frozen hash, test never edited by fix.
- Product diff: 1 file, +1/-1. Zero edits this lane (verify-only reruns).

## Tests
- Rerun 2026-09-16: `--test prov_017_claude_oauth --test prov_018_local_credential_import --test prov_019_request_profile --test prov_020_auth_commands` → 20 passed, 0 failed (4 suites).
- Siblings prov_018..024 → 35/35 pass per lane logs.
- Full providers 62 targets / 352 passed per W2 confirm (`/tmp/opencode/w2-prov.log`).
- Mem ~1.8Gi; serial JOBS=2 THREADS=2, timeout 120.

## Decisions
- One-line impl fix, never test weakening. `begin_login_with_url` still accepts raw loopback URLs.
- Isolation/redaction: T04 scans Debug+serde for `sk-`/`refresh_token`/`Bearer `; T05 byte-identical determinism, `tempfile` dirs only.

## Remaining unknowns
- Acceptance verifier-owned. ralph.json `not-started` untouched.
