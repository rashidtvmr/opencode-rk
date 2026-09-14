# ROUTE-001 worklog

## Claim

Implement the shared account-eligibility phase for built-in multi-account routing before any fill-first, preferred, sticky, or fallback strategy is applied.

## Source evidence

- Candidate base revision: `01e22f7f73599e4b487a273cd58eba206f38eb09` (`feat(cli): add doctor diagnostics`).
- Pinned 9router `src/sse/services/auth.js:72-140` at `17c4cc76877bd1755030a8414f8d0083f48dcccf`.
- Reviewed DISC-003 records `9router.account-storage` and `9router.routing` plus `NR-GITHUB-LOCK-TEST` evidence.
- Existing target `crates/providers/src/router.rs` already owns pure routing decisions and is the product boundary for this slice.

## Target boundary

- Product implementation: `crates/providers/src/router.rs` only.
- Independent RED tests: `crates/providers/tests/account_eligibility.rs` only.
- Status/evidence: this card/worklog; `ralph.json` acceptance/status remains controller-owned.

## Tests

- Frozen independent test: `crates/providers/tests/account_eligibility.rs`.
- Frozen SHA-256: `9bed8bea8d05715701e9fa24f6c8542740558138f2c9d7e6c3abb2656810082f` (rechecked unchanged before final validation).
- Initial worker authoring run hit unrelated `E0583` because concurrent ROUTE-005 prewiring exposed a missing `model_route` module. This was authoring feedback, not valid RED evidence.
- After the unrelated ROUTE-005 signature scaffold existed, the focused suite compiled and produced the valid RED baseline: 1 passed / 4 failed behaviorally.
- Post-implementation focused GREEN: `cargo test -p opencode-rk-providers --test account_eligibility` => 5 passed / 0 failed.
- A blanket provider run subsequently passed provider lib 45/45, ROUTE-001 5/5, config integration 1/1, and PROV-014 5/5 before reaching the intentionally unfinished ROUTE-005 suite (0/5 RED). This known unrelated RED target was not disabled or modified.
- Focused provider regression batch excluding only the intentionally RED `model_route_resolution` target passed: provider lib 45/45, account eligibility 5/5, config 1/1, debug export 5/5, route chores 5/5, and turn cost 5/5.
- `python3 tools/validate_plan.py` => `validate_plan: OK  stories=219 requirements=38 obligations=1095 deps_synthesized=True`.
- `tools/lane_gate.py` is a storage-format-2 delegated-lane gate only, so it is not applicable to this provider routing slice.

## Decisions

- Accept caller-supplied timestamps/lock expiries so the selector is deterministic and has no clock dependency.
- Do not persist account state here; storage/account lifecycle remains separately owned.
- Return a bounded owned eligible vector plus typed exhaustion; no retries, queues, locks, or background tasks are created.
- Runtime/resource bound is the caller-provided candidate slice only; there is no storage, network, wall-clock, or background-work dependency.

## Remaining unknowns

- None for this implementation candidate. Controller/verifier acceptance remains external and is not claimed by this worklog.
