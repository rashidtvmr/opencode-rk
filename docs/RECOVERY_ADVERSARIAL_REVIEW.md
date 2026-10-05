# Recovery Mode adversarial review

Date: 2026-10-05
Status: HARDENED FOR ACTIVATION

This review applies a devil's-advocate pass and an evaluator-optimizer pass to Recovery Mode. It verifies the controller, not product completion.

## Objective

The only convergence metric is unresolved observable Phase-1 behavior on the exact current `main-v2` SHA. Branch count, worktree count, merge count, Ralph completion and legacy claims are not progress metrics.

## Surviving adversarial findings and fixes

### P0 — canonical writes could bypass the integrator
Direct Git could advance `main-v2` outside controller state. Activation now creates a 0600 canonical-write capability; the reference hook rejects canonical movement without it. `tools/recovery_state.py integrate` is the normal writer and remains fast-forward-only.

### P0 — acceptance bootstrap could deadlock
The bootstrap judge failure could remain unresolved while final eligibility required zero unresolved failures. `phase1_judge.py --sync-state` now ingests the same judge result first, reloads runtime state, recomputes eligibility, then synchronizes gates.

### P0 — freezing every evaluator file could make a required probe impossible
The Phase-1 contract, controller, judge engine and existing semantic installed tests are frozen. The missing `tests/e2e/native_second_client.py` is an explicit evaluator-sensitive extension point. It requires two independent approvals including a test evaluator. The frozen judge already has a fail-closed slot for it.

### P0 — rejected candidates could still accumulate approvals
Any current `REJECT` or `NEEDS_FIX` now blocks preverification. A new candidate SHA must be produced and all approvals rerun. A third evaluator may diagnose a disagreement but cannot outvote a rejection on the same SHA.

### P1 — quick prechecks could create fake installed-test work
Quick-mode observation now records only checks actually executed and failed. Missing installed checks become work only in artifact mode, where absence is a real acceptance defect.

### P1 — legacy salvage could become a hidden 524-ref queue
`salvage-query` is failure-scoped, scans the full frozen snapshot read-only, collapses alias tips, materializes at most eight candidates, and blocks integration until all materialized candidates have terminal dispositions. Historical salvage inventories are evidence only.

### P1 — optimizer attempts could thrash forever
After three failed candidate cycles, further claims are blocked until an independent evaluator records a `strategy-review` with both root-cause diagnosis and a materially new approach.

### P1 — crash and partial-state recovery
Runtime state is outside tracked history, serialized by file locks, revision-matched across state files and backed up before multi-file mutation. `recover-state` restores the last consistent revision. `recover-integration` handles interrupted integration and fails closed on ambiguity.

## Evaluator-optimizer invariants

1. Author and evaluator are independent.
2. P0/P1 need two approvals; P2/P3 need one.
3. Security-sensitive diffs also need a security evaluator.
4. Evaluator-sensitive extension diffs need at least two approvals including a test evaluator.
5. Any negative verdict blocks the current candidate.
6. Reviews bind to the exact candidate SHA and never carry to a changed SHA.
7. Candidate diffs must fit the lane intent and explicit path lease.
8. A lane owns at most one active failure and active leases cannot overlap.
9. Canonical integration is single-writer, capability-gated and fast-forward-only.
10. Canonical movement invalidates stale exact-SHA acceptance.
11. Judge ingestion, not prose, creates or resolves observed product failures.
12. Three failed optimizer cycles force independent strategy review.

## Known product-red state

Recovery Mode being healthy does not mean Phase 1 is green. The installed second-client overlap/no-duplicate-work probe remains an explicit durability failure until `tests/e2e/native_second_client.py` exists and passes. The controller must expose this instead of hiding it.

## Stop condition

Phase 1 stops only when, on one clean canonical SHA, runtime unresolved failures are zero, every Phase-1 gate is accepted, the installed native release artifact passes all required checks, the recovery guard is green, no integration/salvage work remains unresolved, and frozen legacy refs/contracts have not moved.
