# ROUTE-002 worklog

## Claim

Implement the source-grounded account-selection phase that follows ROUTE-001 eligibility in pinned 9router: preferred affinity, fill-first, and sticky round-robin with caller-owned state transitions.

## Source evidence

- Candidate base revision: `79448b7` (`feat(providers): resolve compatible provider prefixes`).
- Pinned `src/sse/services/auth.js:136-193` at `17c4cc76877bd1755030a8414f8d0083f48dcccf`.
- Priority-order prerequisite: pinned `src/lib/db/repos/connectionsRepo.js:92-102`.
- Settings/default evidence: pinned `src/lib/db/repos/settingsRepo.js:7-19`.
- `REQ-022` and `9router.routing` candidate ownership were reviewed before assigning this placeholder task; the assignment is a documented decomposition of the observed call path rather than an id-name guess.

## Target boundary

- Product implementation: `crates/providers/src/router.rs`.
- Independent RED tests: `crates/providers/tests/account_selection_strategy.rs` only.
- No storage schema/network/runtime/dependency changes.
- Controller/verifier acceptance remains untouched.

## Tests

- Frozen independent test: `crates/providers/tests/account_selection_strategy.rs`.
- Frozen SHA-256: `391179819e5f7755e65c9477f24783790af01f37daab2a54823451520d748419` (rechecked unchanged before GREEN).
- The independent author's first focused run exited with `E0432` because the new public router API did not yet exist. No behavior test executed, so that run is authoring feedback rather than RED evidence.
- After the integrator added only the public types/constant/signature scaffold, the focused suite compiled and established the valid behavioral RED baseline: 0 passed / 5 failed.
- Post-implementation focused GREEN: `cargo test -p opencode-rk-providers --test account_selection_strategy` => 5 passed / 0 failed.
- Full provider regression: `cargo test -p opencode-rk-providers` passed provider library 45/45; ROUTE-001 5/5; ROUTE-002 5/5; ROUTE-007 5/5; config 1/1; PROV-014 5/5; ROUTE-005 5/5; ROUTE-012 5/5; REL-004 5/5; doc tests 0/0.
- `python3 tools/validate_plan.py` => `validate_plan: OK  stories=219 requirements=38 obligations=1095 deps_synthesized=True`.
- `git diff --check` passed.

## Decisions

- Keep selection separate from persistence: round-robin returns the state patch that the caller may persist; preferred/fill-first return no patch because pinned 9router mutates recency only in its round-robin branch.
- Caller supplies `now` as an integer timestamp to remove wall-clock dependence.
- Preserve caller order for equal recency/priority ties, matching the fact that the upstream input is already priority ordered and modern JS sort is stable.
- `MAX_ACCOUNT_SELECTION_CANDIDATES = 16`; overflow is rejected before strategy work as typed `TooManyCandidates`. This is an explicit safer native resource deviation from the unbounded upstream DB result.
- Empty post-eligibility input is typed `EmptyCandidates`; a zero sticky limit is rejected for round-robin as `InvalidStickyLimit`.
- Preferred selection is evaluated before strategy validation and returns no persistence patch, matching upstream's strategy short-circuit. Fill-first also returns no patch.
- Round-robin uses a bounded stable ordering equivalent to upstream: most-recent used candidate is sticky while its count is below the limit; otherwise unused candidates sort first, unused ties use priority, and equal comparator ties retain caller order. The returned patch uses caller-supplied `now`; consecutive-use increment is saturating to keep the native counter bounded rather than wrap.
- The selector performs no DB/network/environment/wall-clock access and creates no queue/background work. Candidate vectors created for ordering are bounded by the 16-candidate public maximum.

## Remaining unknowns

- None for the scoped account-selection implementation candidate. Controller/verifier acceptance remains external and is not claimed.
- ROUTE-011 fallback/execution semantics remain separately owned and are not claimed here.
