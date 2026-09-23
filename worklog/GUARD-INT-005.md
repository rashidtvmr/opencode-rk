# GUARD-INT-005 audit

## Claim

- Task: INT-005.
- Scope: audit only. No product, test, controller, or verifier files changed.
- Verdict: blocked. No acceptance claim.

## Source evidence

- `tasks/INT-005.md:1-16`: card remains `Status: NOT STARTED`; contract limits this slice to decision, single-flight, and outcome recording. Provider refresh execution, secrets, persistence, events, and network remain caller-owned.
- `sources/integrations-ownership-gap.json:3-46`: source status is `source-reviewed-no-exact-task-owner`; INT-005 is in the indistinguishable `generic-integration-residual`; `ownershipDecision.INT-005` is `null`; task card and worklog are `null`.
- `sources/integrations-ownership-gap.json:211-215`: integration-auth evidence excludes provider callback/refresh execution, secrets, persistence, events, and network; refresh direct integration test and lifecycle/capacity specifications are not established.
- `sources/backlog-exhaustion.json:530-553`: INT-005 is `unresolved-decomposition`, has no implementation commits, local evidence, task card, or worklog.
- `FEATURES.md:59,635,887` and `ralph.json:1087-1097`: controller-facing accounting says `accepted`, conflicting with the task card and unresolved ownership ledger.
- `crates/providers/src/int_refresh.rs:1-10`: pure in-memory candidate documents caller-owned refresh callback, persistence, events, and scheduling.
- `crates/providers/src/lib.rs:48-49`: exports `refresh_gate`, not `int_refresh`; no production server caller was found in `crates/server/src`.

## Guard results

- `python3 tools/validate_repository.py`: exit 1, 51 errors.
- `python3 tools/validate_backlog_exhaustion.py`: exit 1, 51 errors.
- `python3 tools/validate_plan.py`: exit 1, 51 errors.
- `python3 tools/convergence_gate.py`: exit 1, `total=80`.

No cargo, provider, database, browser, network, or heavy resource command ran in this audit.

## Preserved blockers

Acceptance evidence is insufficient. Production export/caller wiring is absent. Installed authenticated parent journey, provider/network/auth behavior, secret handling, persistence/event transitions, callback lifetime, and whole-process capacity evidence remain unproven. The repository-wide accounting and convergence guards remain RED. Controller reconciliation is required before this task can be considered owned or accepted.

## Decision

Mark the INT-005 ledger row `blocked`, not `completed`. Reopen only after controller-owned ownership/accounting reconciliation and real production-path integration evidence.
