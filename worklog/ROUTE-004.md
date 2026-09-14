# ROUTE-004 worklog

## Claim

Implement the pure success/expired-lock cleanup transformation observed in pinned 9router `clearAccountError`, preserving active unrelated locks and caller-owned persistence.

## Source evidence

- Candidate base revision before this lane: `2d1a47d` (`feat(providers): select routed accounts`). ROUTE-003 will commit first if both independent RED lanes are authored concurrently.
- Pinned `src/sse/services/auth.js:289-337` at `17c4cc76877bd1755030a8414f8d0083f48dcccf`.
- Pinned caller test: `tests/unit/fetch-success-clears-account.test.js:74-95`.
- Pinned lock helpers: `open-sse/services/accountFallback.js:105-160`.
- The ROUTE-003/004 mark-vs-clear split is an explicit lifecycle decomposition of jointly nominated source surfaces.

## Target boundary

- Product implementation: `crates/providers/src/rate_limit.rs` after ROUTE-003 is committed.
- Independent RED tests: `crates/providers/tests/account_recovery_cleanup.rs` only.
- No storage/network/dependency changes.
- Controller/verifier acceptance remains untouched.

## Tests

- Independent test file: `crates/providers/tests/account_recovery_cleanup.rs`.
- Initial independent authoring run exited with `E0432` because the recovery API did not yet exist. No behavioral test executed, so that run is authoring feedback rather than RED evidence.
- Frozen SHA-256: `c95bfed35ac2ed52e9836ecdcedab8a811a0b2c93880e2f92defd455bdb9dae7`; no edits were made after hashing.
- After ROUTE-003 committed and prime added only the ROUTE-004 public types/constant/signature scaffold, the focused suite compiled and established valid behavioral RED: 1 passed / 4 failed.
- Post-implementation focused GREEN: `cargo test -p opencode-rk-providers --test account_recovery_cleanup` => 5 passed / 0 failed.
- Full provider regression: `cargo test -p opencode-rk-providers` passed provider library 45/45 plus every current provider integration target, including ROUTE-001/002/003/004/005/007/012, config, PROV-014, REL-004, and doc tests.
- `/usr/bin/python3 tools/validate_plan.py` => `validate_plan: OK  stories=219 requirements=38 obligations=1095 deps_synthesized=True`.
- `git diff --check` passed. `tools/lane_gate.py` is storage-format-2-specific and has no provider lane, so it is not applicable here.

## Decisions

- Represent locks as typed caller-owned state rather than dynamic DB field names; the returned patch identifies scopes to clear and whether error/backoff state may reset.
- Caller supplies `now`; no wall clock is read.
- Use an explicit lock-count bound so lazy cleanup cannot become unbounded work.
- `MAX_ACCOUNT_MODEL_LOCKS = 16`; exact-bound input succeeds and overflow returns typed `TooManyLocks { max, actual }` before cleanup scanning.
- Cleanup preserves caller lock order in the returned clear list. Current-model locks, account-wide locks, and expired locks are selected for clearing; active unrelated model locks remain.
- Both account-wide scope spellings currently exposed by the shared rate-limit module are treated as account-wide semantics, preserving both independently frozen task surfaces without rewriting either test.
- Error/test/backoff reset flags are requested only when no active lock remains after planned clears. Caller owns actual persistence.
- No DB/network/environment/wall-clock/background work is performed; caller supplies `now`, state, and the bounded lock slice.

## Remaining unknowns

- None for this scoped implementation candidate. Persistence adapter integration and controller/verifier acceptance remain separately owned and are not claimed here.
