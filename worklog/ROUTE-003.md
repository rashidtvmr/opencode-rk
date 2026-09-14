# ROUTE-003 worklog

## Claim

Implement the pure failure-to-account-lock/update transformation observed in pinned 9router `markAccountUnavailable`, leaving persistence and generic retry execution caller-owned.

## Source evidence

- Candidate base revision: `2d1a47d` (`feat(providers): select routed accounts`).
- Pinned `src/sse/services/auth.js:12-18,229-287` at `17c4cc76877bd1755030a8414f8d0083f48dcccf`.
- Pinned lock helper: `open-sse/services/accountFallback.js:105-150`.
- Direct pinned lock-scope test: `tests/unit/github-monthly-usage-lock.test.js:31-85`.
- ROUTE-003/004 share the same discovered account-storage/routing/token-refresh ownership candidates; this worklog records the explicit lifecycle split used for independent TDD rather than claiming the task id encoded mark-vs-clear semantics.

## Target boundary

- Product implementation: `crates/providers/src/rate_limit.rs`.
- Independent RED tests: `crates/providers/tests/account_failure_lock.rs` only.
- No storage/network/dependency changes.
- Controller/verifier acceptance remains untouched.

## Tests

- Independent test file: `crates/providers/tests/account_failure_lock.rs`.
- Initial worker authoring run was not RED: the product API was absent (`E0432`), and the test also needed an explicit `u64` annotation for a numeric expression. The worker fixed only that authoring issue and formatted the file.
- A subsequent compile attempt exposed a test-local helper shadowing error (`E0618`). Because no compiling behavioral RED existed yet, the independent author made only the naming-only test fix. Those compile failures are authoring feedback, not RED evidence.
- Final frozen SHA-256: `9018241a6cb55e8f455604fe3884569915361af36f05af47ed20699235d8a0bd`. No edits were made to the test after this freeze.
- With only the public type/constant/signature scaffold present, `cargo test -p opencode-rk-providers --test account_failure_lock` compiled and established valid behavioral RED: 1 passed / 4 failed.
- Post-implementation focused GREEN: the same command => 5 passed / 0 failed.
- Provider regression excluding the still-unimplemented concurrent ROUTE-004 target passed: provider library 45/45 plus account eligibility, account selection, compatible prefix, config, debug export, model route, route chores, and turn-cost targets all GREEN.
- A blanket `cargo test -p opencode-rk-providers` was also run. It stopped at the intentionally unfinished ROUTE-004 frozen target because the recovery API/`AccountWide` variant had not yet been introduced; this is not a ROUTE-003 behavior failure.
- `/usr/bin/python3 tools/validate_plan.py` => `validate_plan: OK  stories=219 requirements=38 obligations=1095 deps_synthesized=True`.
- `git diff --check` passed. `tools/lane_gate.py` is a storage-format-2-only gate with hard-coded storage lanes, so it is not applicable to this provider task.

## Decisions

- Generic error-rule classification remains a separate fallback-policy concern; this slice accepts the generic fallback outcome as caller input and owns the source-observed lock/update transformation plus GitHub/precise-reset overrides.
- Caller supplies `now` and the next-UTC-month reset timestamp, avoiding hidden wall-clock/calendar access in the pure provider boundary.
- Persisted reason is bounded at the upstream 100-character limit.
- `MAX_RATE_LIMIT_COOLDOWN_MS` is the pinned 30-minute precise-reset cap. Non-Antigravity precise resets are capped at that interval; Antigravity preserves its exact future reset.
- Generic fallback lock deadlines use saturating addition, and reason truncation is character-bounded to avoid integer wrap or an unbounded retained error string.
- The pure planner performs no DB/network/environment/clock/background work; caller owns generic error classification, time, calendar reset, and persistence.

## Remaining unknowns

- None for this scoped implementation candidate. ROUTE-004 success cleanup and ROUTE-011 retry/fallback execution remain separately owned; controller/verifier acceptance is external and is not claimed here.
