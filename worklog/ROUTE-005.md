# ROUTE-005 worklog

## Claim

Implement the pure alias/combo model-routing boundary observed in pinned 9router before provider/account execution.

## Source evidence

- Candidate base revision when this lane was prewired: `01e22f7f73599e4b487a273cd58eba206f38eb09` (`feat(cli): add doctor diagnostics`). ROUTE-001 was committed independently as `5e1c98e` before final ROUTE-005 implementation/commit.
- Pinned `src/sse/services/model.js:27-94` at `17c4cc76877bd1755030a8414f8d0083f48dcccf`.
- Reviewed DISC-003 `9router.model-combos` record and `NR-MODEL-ROUTING` evidence.
- No equivalent alias/combo resolver currently exists in providers/catalog target code.

## Target boundary

- Product implementation: `crates/providers/src/model_route.rs` only.
- Independent RED tests: `crates/providers/tests/model_route_resolution.rs` only.
- Shared module export in `crates/providers/src/lib.rs` is prewired by the integrator.
- Status/evidence: this card/worklog; controller/verifier acceptance remains untouched.

## Tests

- Frozen independent test: `crates/providers/tests/model_route_resolution.rs`.
- Frozen SHA-256: `a60998ff42c5004ee1dd187aae5e411f2c93a96c87dc0931ae6dd5e20bf7d92e` (rechecked unchanged immediately after implementation).
- Initial worker authoring run hit `E0583` because `pub mod model_route;` had been prewired before `crates/providers/src/model_route.rs` existed. This was authoring feedback, not valid RED evidence.
- After the integrator added only the public types/signature scaffold, the focused suite compiled and produced the valid behavioral RED baseline: 0 passed / 5 failed.
- Post-implementation focused GREEN: `cargo test -p opencode-rk-providers --test model_route_resolution` => 5 passed / 0 failed.
- Provider regressions passed across every current provider target: library 45/45; account eligibility 5/5; config integration 1/1; debug export 5/5; model route resolution 5/5; route chores 5/5; turn cost counters 5/5.
- `python3 tools/validate_plan.py` => `validate_plan: OK  stories=219 requirements=38 obligations=1095 deps_synthesized=True`.
- `git diff --check` passed after implementation and bookkeeping.

## Decisions

- The caller supplies alias/combo maps; this slice performs no DB/network lookup and retains no registry state.
- Combo members are caller-owned input and copied only in the returned decision; the resolver applies an explicit member-count bound frozen by tests.
- `MAX_COMBO_MODELS` is the public bound and is frozen at 8; overflow returns `TooManyComboModels` with the actual count and bound.
- Explicit `provider/model` input is parsed before named lookups and bypasses aliases/combos. For non-provider/model names, non-empty combo lookup precedes alias lookup, matching pinned 9router. Empty combos are not treated as combos, consistent with upstream `getComboModels` requiring `models.length > 0`.
- Runtime/resource ownership is caller-bounded only: aliases and combos remain caller-owned, and only the selected alias/model or bounded combo member vector is cloned into the result. No storage, network, clock, queue, or background task is introduced.
- Provider-node prefix compatibility and combo execution/fallback strategy remain later routing slices.

## Remaining unknowns

- None for this implementation candidate. Compatible-provider prefix mapping, combo rotation/sticky/fallback execution, and controller/verifier acceptance remain separately owned and are not claimed here.
