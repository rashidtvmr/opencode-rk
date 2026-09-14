# ROUTE-012 worklog

## Claim

Implement the dependency-free ROUTE-012 slice: quota/topic/title/summarize chores select the cheapest available model that explicitly supports the chore, with a fail-open fallback to the caller's main model. Quota probes cap output at one token, matching the pinned reverse-harness observation.

## Source evidence

- Candidate repository revision: `2c2764e010b86373e6c9092ba17f1e00d52af205`.
- `tasks/ROUTE-012.md`: REQ-018, no dependencies, five test obligations.
- `docs/upcoming-features/claude-code-reverse-harness.md:7,14,26,76-80`: reverse source pin `c0d99ea1ab7168c12ba74838cfea355ce10f6c56`; observed Haiku quota probe with `max_tokens=1`; cheap-model chores belong in ROUTE/CAT; chores are advisory or fail-open.
- Current code: `crates/providers/src/router.rs` is a one-line stub; `crates/providers/src/lib.rs` already exports `router`.

## Observed scenario

No ROUTE-012 behavior exists in the provider router. Provider registry/health/budget are separate concerns and do not expose a chore-selection contract.

## Target boundary

- Product implementation: `crates/providers/src/router.rs` only.
- Independent RED tests: `crates/providers/tests/route_chores.rs` only.
- Backlog/status bookkeeping: this worklog, `tasks/ROUTE-012.md`, and the existing ROUTE-012 status field in `ralph.json`.

## Tests

Pending independent RED authoring and frozen hash.

## Decisions

- Keep selection pure and bounded over the caller-provided candidate slice; no queue, network call, retained output, or detached task is introduced.
- Represent model suitability explicitly per chore instead of inferring price/capability from model names.
- Fail open to the main model when no available candidate supports the chore.

## Remaining unknowns

- Exact public API shape will be frozen by the RED test contract before behavior implementation.
