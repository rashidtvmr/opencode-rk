# PROV-016 worklog (verify-only, dedicated)

## Claim
`crates/providers/src/codex_oauth.rs` (535 lines) satisfies `tasks/PROV-016.md` contract. Frozen suite `crates/providers/tests/codex_oauth.rs` 5/5 GREEN. No valid RED (product predates tests). Verifier decides acceptance.

## Source evidence
- Base rev `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`.
- `tasks/PROV-016.md:8-9`: owns `codex_oauth.rs` only; lib.rs/Cargo.toml/schemas untouched.
- Impl `crates/providers/src/codex_oauth.rs:12-535`: CodexAuthState{LoggedOut,PendingConsent,Ready,Expired}, begin/complete/refresh/logout/status/route_target, caller-supplied time, fake grants only.
- Wired `crates/providers/src/lib.rs:13` (`pub mod codex_oauth;`, integrator-owned, untouched).
- Precedents `auth.rs:6-18,40-101`, `oauth_flow.rs:1-65`, `integration.rs:1-18`, SECURITY.md §5. Read-only.

## Observed scenario
Product predates lane. No T01..T05 coverage before. New target first run GREEN 10/10 (with PROV-015). Per TDD §3 NOT valid RED. GREEN-only regression evidence.

## Target boundary
- Owned/new: `crates/providers/tests/codex_oauth.rs` (untracked, 5 #[test]).
- NOT touched: `src/*`, lib.rs, Cargo.toml, tasks/*, ralph.json, any other file. Zero edits.

## Tests
- Current sha256: `aa7459f644f45eae1815fe9c5dfc67fcee45f7ff9df5db8d2aa4369514098851` (differs from bundle-lane `dd08f305...`; untracked drift, 5 #[test] verified, GREEN).
- Rerun 2026-09-16: `--test auth_profile --test codex_oauth` → 10 passed, 0 failed.
- RED: NO valid RED. GREEN-only.
- Mem ~1.8Gi; serial JOBS=2 THREADS=2, timeout 120.

## Decisions
- Verify-only, no edits. Only `HumanGrant::for_test`/`RefreshGrant::for_test` + None negative path (fake grants, disposable fixtures, SECURITY.md §5).
- Isolation T05 asserts disposable-dir file count unchanged; no DB paths.

## Remaining unknowns
- Acceptance verifier-owned. ralph.json `not-started` untouched.

## Bounds addendum (PROV-016 boundary pins, 2026-09-16)
- New additive file `crates/providers/tests/codex_oauth_bounds.rs` (sha256 `9d2c8d87...f354e28`), 6 tests B06..B11: empty/oversize device code (`begin_login_with`, `from_device_code`, `for_device`, MAX=128 exact-boundary Ok), consent-URL >2048B reject + 2048B Ok, zero-expiry reject on `complete_login`/`refresh` (incl. `Expired` state), stale-expiry (`expires<=now`) reject via `complete_login_at`/`refresh_at`.
- Frozen `tests/codex_oauth.rs` untouched (`aa7459f6...`). Impl untouched (`e16ef548...`, rev `248f519`).
- GREEN: `cargo test -p opencode-rk-providers --test codex_oauth_bounds -- --test-threads=1` → 6 passed, 0 failed (`/tmp/opencode/v2-bounds.log`).
- Mutation control (/tmp only, repo tests never mutated): copied new test, flipped one B10 assertion (`InvalidExpiry`→`ConsentRequired`), ran as disposable `__mut_control` target → FAILED 5 passed/1 failed as expected (`/tmp/opencode/v2-mut-control.log`); control file removed, `git status` clean for owned/frozen/impl paths.
- Kills B3/B3alt/B3exp stub classes from `worklog/RED-VALIDITY-PROV015-016.md`.

## Bounds2 addendum (PROV-016, 2026-09-16, rev 248f519)
- New additive file `crates/providers/tests/codex_oauth_bounds2.rs` (sha256 `3595ed6c1c9674885932a979191f4c05aa1c278f4b058747f946a62f582986fc`), 6 tests C12..C17: empty device code (all 3 entry points), oversize device code (all 3, MAX=128 exact-boundary Ok), consent-URL >2048B reject + 2048B Ok, zero-expiry reject on `complete_login`/`refresh` (incl. `Expired`), stale-expiry (`expires<=now`) reject via `complete_login_at` / `refresh_at` (split complete vs refresh).
- Untouched: frozen `tests/codex_oauth.rs` (`aa7459f6...`), sibling `tests/codex_oauth_bounds.rs` (`9d2c8d87...`), impl `src/codex_oauth.rs` (`e16ef548...`), lib.rs, ralph.json.
- GREEN: `cargo test -p opencode-rk-providers --test codex_oauth_bounds2 -- --test-threads=1` → 6 passed, 0 failed (`/tmp/opencode/aC-bounds2-green.log`); serial JOBS=1 THREADS=1, timeout 120; `free -h` start 6.2G total / 2.3G avail.
- Mutation control (/tmp only): copied new test, flipped one C15 assert (`InvalidExpiry`→`ConsentRequired`), disposable `__mut_control` target → FAILED 5 passed/1 failed as expected (`/tmp/opencode/aC-bounds2-mut.log`); control removed, owned/frozen/impl paths clean.
- Kills B3/B3alt/B3exp classes from `worklog/RED-VALIDITY-PROV015-016.md` (independent second pin; does not replace bounds.rs).
