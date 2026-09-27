# GAP-ROUTING-34-W1

## Claim

- Candidate: `f21a01e21a4a82b57dae641c032472585f84b2c4`, branch `reconcile/GAP-ROUTING-34-W1`.
- Reclaimed prior claim from `ses_f1df19243ffecScbtKR0q1Dke3`: orchestrator reported terminated quota/max-step sessions; prior scratchpad absent; no product-file changes present.
- Current owner: `ses_f1d90eefcffeqHIqwpYZ6KRAze`.

## Source evidence and observed state

- `ralph.json:1643-1654` sets ROUTE-009 to `accepted`; `ralph.json:1657-1668` sets ROUTE-010 to `accepted`. Both retain generic DISC-002 descriptions and empty requirement bindings.
- Current `ralph.json:userStories`: 258 stories; 224 accepted, 23 in-progress, 11 not-started. Thus 34 are non-accepted.
- `sources/routing-ownership-gap.json:29-35` currently retains null ownership decisions and marks both task bindings as generic/no-card/no-worklog. Source-review evidence, pins and ownership blockers remain intact.
- `tools/validate_backlog_exhaustion.py:737-745` hardcodes both Ralph statuses as `not-started`; lines 777-787 require both rows in the exhaustion ledger as `unresolved-decomposition`. Current `sources/backlog-exhaustion.json` has no ROUTE-009/010 rows. This lane cannot repair those external contracts.
- `FEATURES.md:713-714` also says controller-accepted; this lane does not modify it or controller state.
- `docs/CONVERGENCE.md:107-109` gate reports 60 existing claim blockers; it is not an acceptance signal.

## Boundary and decision

Own only `sources/routing-ownership-gap.json`, this scratchpad, and this task's claim row. Keep ROUTE-009/010 ownership unresolved (`ownershipDecision: null`); record the controller projection as accepted without rewriting status, reopening stories, changing pins, or inventing task ownership. Preserve all existing source evidence and closure constraints. Validator contradiction means report blocked, not falsify statuses or touch validator/tests/backlog/controller files.

## Checks

- `python3 -m json.tool sources/routing-ownership-gap.json`: pass.
- `git diff --check`: pass.
- `python3 tools/validate_backlog_exhaustion.py`: fail, 93 errors. Routing-specific errors: both accepted stories conflict with the validator's `not-started` assumption; both rows absent from the unresolved-only exhaustion ledger; ROUTE-006/008 and WEB-004 classifications also drift. The other 85 errors are unrelated existing gap-manifest reconciliation findings. Same routing errors were present before the owned manifest edit.
- Projection assertions against live `ralph.json`: pass, 258 total; accepted 224; non-accepted 34; ROUTE-009/010 remain accepted in projection with null source ownership.
- Frozen tests, validator, controller, backlog ledger and Ralph files untouched.

## Remaining blocker

- Routing validator contract assumes `not-started` and unresolved-decomposition ledger rows, contradicting current controller-accepted state and 224/34 classifier projection. Requires separately authorized integration reconciliation.
