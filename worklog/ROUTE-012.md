# ROUTE-012 worklog

## Claim

Implement the dependency-free ROUTE-012 slice: quota/topic/title/summarize chores select the cheapest available model that explicitly supports the chore, with a fail-open fallback to the caller's main model. Quota probes cap output at one token, matching the pinned reverse-harness observation.

## Source evidence

- Candidate base revision: `2780889f1182ea65bb61e18e939a6c2ab037caca`.
- `tasks/ROUTE-012.md`: REQ-018, no dependencies, five test obligations.
- `docs/upcoming-features/claude-code-reverse-harness.md:7,14,26,76-80`: reverse source pin `c0d99ea1ab7168c12ba74838cfea355ce10f6c56`; observed Haiku quota probe with `max_tokens=1`; cheap-model chores belong in ROUTE/CAT; chores are advisory or fail-open.
- Current code before this slice: `crates/providers/src/router.rs` exposed the frozen typed API but intentionally returned the main model for every chore; `crates/providers/src/lib.rs` already exported `router`.

## Observed scenario

The compiling RED scaffold preserved fail-open behavior but did not select eligible cheap candidates. Provider registry/health/budget remain separate concerns; this slice consumes only caller-supplied availability, capability, cost, and model identity.

## Target boundary

- Product implementation: `crates/providers/src/router.rs` only.
- Independent RED tests: `crates/providers/tests/route_chores.rs` only.
- Status/evidence bookkeeping: this worklog and `tasks/ROUTE-012.md`. Controller/verifier acceptance in `ralph.json` remains untouched.

## Tests

- Independent RED file: `crates/providers/tests/route_chores.rs`.
- Frozen SHA-256: `6c5bc40f499c44757baf45af6e37989936e4c4dbc660a24f0595d6f7a143f985`.
- Initial authoring run against the one-line stub failed to compile with unresolved expected router symbols; this was authoring feedback, not RED evidence.
- After a signature-only contract scaffold in `crates/providers/src/router.rs`, `cargo test -p opencode-rk-providers --test route_chores` compiled and failed behaviorally: 1 passed, 4 failed. This is the frozen RED baseline.
- GREEN: `cargo test -p opencode-rk-providers --test route_chores` passed 5/5 with the frozen test hash unchanged.
- Regression: `cargo test -p opencode-rk-providers` passed 45 library tests, 1 config integration test, 5 PROV-014 tests, and 5 ROUTE-012 tests.
- Plan validation: `/usr/bin/python3 tools/validate_plan.py` reported `OK stories=219 requirements=38 obligations=1095 deps_synthesized=True`.

## Decisions

- Keep selection pure and bounded over the caller-provided candidate slice; no queue, network call, retained output, or detached task is introduced.
- Represent model suitability explicitly per chore instead of inferring price/capability from model names.
- Fail open to the main model when no available candidate supports the chore.
- Select with a bounded linear scan over the caller-provided slice. Cost is primary; `ModelRef` lexical order is the deterministic equal-cost tie-breaker.
- Quota probes always request at most one output token, including the fail-open main-model case.

## Remaining unknowns

- None for this slice. Controller/verifier acceptance remains a separate repository-owned step.
