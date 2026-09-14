# ROUTE-007 worklog

## Claim

Implement the uniquely source-grounded compatible-provider prefix precedence boundary from pinned 9router without claiming the still-shared combo execution surface.

## Source evidence

- Candidate base revision: `116ef47` (`feat(providers): resolve model aliases and combos`).
- Pinned 9router `src/sse/services/model.js:12-17,41-66` at `17c4cc76877bd1755030a8414f8d0083f48dcccf`.
- Pinned direct behavior tests: `tests/unit/model-routing.test.js:41-79`.
- `sources/behavior-surface-rules.json` `9router.model-combos` nominates ROUTE-005/ROUTE-007 for this model service; ROUTE-005 worklog explicitly leaves compatible-provider prefix handling later.
- Audit of remaining IDs found combo runtime is jointly nominated to ROUTE-006/ROUTE-007 through `9router.translation-proxy`, so this card does not assign that unresolved shared behavior.

## Target boundary

- Product implementation: `crates/providers/src/model_route.rs`.
- Independent RED tests: `crates/providers/tests/compatible_provider_prefix.rs` only.
- No new dependencies or shared storage/runtime contracts.
- Controller/verifier acceptance remains untouched.

## Tests

- Frozen independent test: `crates/providers/tests/compatible_provider_prefix.rs`.
- Frozen SHA-256: `7698eee928af1818c02effea1e34d07489df37c97a0d58f928c7ac6b6d7924eb` (rechecked unchanged before GREEN).
- The test author's first focused run exited with `E0432` because the four new public symbols did not yet exist. That was authoring feedback only and is not cited as RED evidence.
- After adding only the public types/constant/signature scaffold, the suite compiled and established the valid behavioral RED baseline: 0 passed / 5 failed.
- Post-implementation focused GREEN: `cargo test -p opencode-rk-providers --test compatible_provider_prefix` => 5 passed / 0 failed.
- Full provider regression: `cargo test -p opencode-rk-providers` passed provider library 45/45 plus account eligibility 5/5, compatible provider prefix 5/5, config 1/1, debug export 5/5, model route resolution 5/5, route chores 5/5, turn cost counters 5/5, and doc tests.
- `python3 tools/validate_plan.py` => `validate_plan: OK  stories=219 requirements=38 obligations=1095 deps_synthesized=True`.
- `git diff --check` passed.

## Decisions

- Keep the resolver pure and caller-owned rather than reproducing 9router's DB lookup inside routing logic.
- Preserve built-in-prefix precedence as the observable invariant; compatible nodes are only candidates for non-reserved prefixes.
- `MAX_COMPATIBLE_PROVIDER_NODES = 32`; overflow returns typed `TooManyCompatibleProviderNodes` before scanning. This is an explicit safer resource-bounded native deviation from the unbounded upstream DB result.
- Malformed input and unknown prefixes return typed errors. Resolution performs no DB/network/env/clock access, persistence, queueing, or background work.
- Reserved alias lookup precedes compatible-node lookup and returns the caller-supplied canonical provider id; compatible-node lookup is exact-prefix and preserves the model remainder.

## Remaining unknowns

- None for the scoped compatible-prefix implementation candidate. Controller/verifier acceptance remains external and is not claimed.
- Combo ordering/sticky/fallback/stream-commit semantics remain a shared ROUTE-006/ROUTE-007 DISC-003 ownership question and are intentionally not claimed by this task status.
