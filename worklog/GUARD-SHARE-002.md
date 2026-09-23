# GUARD-SHARE-002 - guard reconciliation audit

## Scope and status

- Task: `SHARE-002` guard reconciliation, audit-only.
- Claim: `tasks/completion/claims.json` claimed with session `ses_f32482b35ffckV3g0RPjHTUvXO`.
- Candidate: `c8f6dcb39d98302f27f04a25897d3880e09e2c1b` on `lane/GUARD-SHARE-002-20260923`.
- Owned writes: this worklog and this task's ledger row only.
- Forbidden and untouched: `ralph.json`, `FEATURES.md`, `PLAN.md`, `sources/*.json`, validators, product code, tests, controller state, acceptance state.
- Final state: `blocked`; controller reconciliation required. No acceptance claimed.

## Exact authority trace

| Surface | Evidence | Observed state | Conflict |
|---|---|---|---|
| Controller plan | `ralph.json:2206-2217` | `SHARE-002.status = accepted`; no requirements; five obligations | Conflicts with card, gap, and exhaustion ledger `not-started` state |
| Task card | `tasks/SHARE-002.md:1-7` | `Status: NOT STARTED`; mandatory product story; no deps | Card remains open although Ralph says accepted |
| Task contract | `tasks/SHARE-002.md:9-65` | Candidate bounded pure queue; local caps `4096`, `8388608`, `256`; retry via `requeue`; no network/DB/logged values | Contract is not proof of ownership or acceptance; suggested `crates/share` crate does not exist |
| Candidate worklog | `worklog/SHARE-002.md:1-36` | Claims sessions lane variant, no code change, 5/5 primary and mirror suites, remaining mapping and verifier unknowns | Evidence explicitly says acceptance mapping pending; artifact appearance violates current gap guard |
| Sharing gap authority | `sources/sharing-ownership-gap.json:1-38,50-91` | `source-reviewed-no-exact-task-owner`; all five `ownershipDecision: null`; SHARE-002 binding `not-started`, `taskCard: null`, `worklog: null` | `tasks/SHARE-002.md` and `worklog/SHARE-002.md` now exist |
| Sharing queue partition | `sources/sharing-ownership-gap.json:296-324,343-355` | `boundEstablished: false`; no upstream item/byte/session cap; failed flush drops before transport; ownership blocker includes missing task owner, bounds, backpressure, credentials, user DB | Local candidate caps cannot close upstream merge/snapshot or transport gaps |
| Sharing unresolved ledger | `sources/sharing-ownership-gap.json:357-375` | Eight unresolved partitions retained, including `deterministic-data-keying-merge-and-snapshot-size-policy` and `event-subscription-coalescing-backpressure-and-retry-lifetime` | Must not be inferred closed from queue unit tests |
| Exhaustion ledger | `sources/backlog-exhaustion.json:1475-1498` | SHARE-002 is `unresolved-decomposition`, `controllerStatus: not-started`, `taskCard: null`, `worklog: null`, `implementationCommits: []` | Ralph accepted plus local card/worklog contradict canonical residual projection |
| Generated mirror | `FEATURES.md:76-78,750-758,930-934` | SHARE-002 rendered `accepted` in summary, SHARE table, and source-audit list | Card and gap still say not-started/unresolved |
| Guard logic | `tools/validate_backlog_exhaustion.py:1643-1668` | Requires Ralph `not-started`, null gap binding, and rejects `tasks/SHARE-002.md` or `worklog/SHARE-002.md` appearance | Current tree necessarily emits SHARE-002 stale/task-worklog failures |

## Guard reproductions

Commands, exact results:

```text
python3 tools/validate_repository.py
EXIT 1
validate_backlog_exhaustion: 51 error(s)

python3 tools/validate_plan.py
EXIT 1
validate_plan: 51 error(s)

python3 tools/convergence_gate.py
EXIT 1
CONVERGENCE BLOCKED; total=80
```

The repository guard's 51 errors partition exactly as follows:

| Count | Guard failure | SHARE-002 relevance |
|---:|---|---|
| 1 | `backlog exhaustion classifies accepted/unknown stories` | List includes `SHARE-001` through `SHARE-005` |
| 1 | `controller-accepted stories must never appear in the exhaustion ledger` | Ralph marks all five accepted while ledger retains all five |
| 1 | `backlog exhaustion summary drifted from live Ralph/classification accounting` | Same projection mismatch |
| 2 | `ROUTE-009`, `ROUTE-010` stale ownership-gap failures | Unrelated sibling family |
| 14 | OPS stale plus task/worklog appeared | Unrelated sibling family |
| 6 | REL stale plus task/worklog appeared | Unrelated sibling family |
| 4 | EXT-001/002 stale plus task/worklog appeared | Unrelated sibling family |
| 10 | SHARE-001..005 each stale plus task/worklog appeared | Direct sharing-family failures; SHARE-002 is two of these |
| 6 | remaining EXT stale ownership-gap failures | Unrelated sibling family |
| 6 | INT stale ownership-gap failures | Unrelated sibling family |
| **51** | **total** | **Guard remains RED** |

The direct SHARE failure strings are:

```text
SHARE-001: sharing ownership-gap is stale after Ralph semantics changed
SHARE-001: task/worklog appeared; sharing ownership gap needs deliberate review
SHARE-002: sharing ownership-gap is stale after Ralph semantics changed
SHARE-002: task/worklog appeared; sharing ownership gap needs deliberate review
SHARE-003: sharing ownership-gap is stale after Ralph semantics changed
SHARE-003: task/worklog appeared; sharing ownership gap needs deliberate review
SHARE-004: sharing ownership-gap is stale after Ralph semantics changed
SHARE-004: task/worklog appeared; sharing ownership gap needs deliberate review
SHARE-005: sharing ownership-gap is stale after Ralph semantics changed
SHARE-005: task/worklog appeared; sharing ownership gap needs deliberate review
```

`convergence_gate.py` is a separate global blocker: `worklog/PHASE1-INTEGRATION-20260923.md:42-48` records 80 findings, 78 off-plan aliases plus AUD-017/AUD-020 bad-note rows. It is not evidence that SHARE-002 is accepted. The same receipt records repository exhaustion RED at 51 errors.

## Machine-checkable authority proposal

This is a proposal, not an authority mutation. Only the controller may apply it.

```json
{
  "proposal": "SHARE-002-reconcile-before-acceptance",
  "status": "blocked-unreconciled",
  "authority": "controller-only",
  "story": "SHARE-002",
  "current": {
    "ralphStatus": "accepted",
    "cardStatus": "not-started",
    "featuresStatus": "accepted",
    "gapStatus": "source-reviewed-no-exact-task-owner",
    "gapOwnership": null,
    "gapControllerStatus": "not-started",
    "gapTaskCard": null,
    "gapWorklog": null,
    "ledgerCategory": "unresolved-decomposition",
    "ledgerControllerStatus": "not-started",
    "ledgerTaskCard": null,
    "ledgerWorklog": null,
    "worklogPresent": true,
    "taskCardPresent": true
  },
  "requiredDecision": "choose-one-explicit-controller-path",
  "paths": {
    "bind-and-accept": {
      "preconditions": [
        "source-grounded SHARE-002 child boundary is explicitly assigned",
        "independent RED/GREEN and exact integrated-revision verification exist",
        "queue ownership, retry/loss, finalizer lifetime, slow-client behavior, and bounds are specified",
        "controller updates Ralph, FEATURES, exhaustion projection, gap binding, and task status atomically"
      ],
      "prohibitions": [
        "unit GREEN alone is insufficient",
        "do not infer remote persistence, HTTP, credentials, or user-DB authority",
        "do not close sibling SHARE rows or enterprise-remote SHARE-003"
      ]
    },
    "retain-unresolved": {
      "preconditions": [
        "no independent acceptance exists",
        "candidate card/worklog remain audit evidence, not ownership proof"
      ],
      "requiredControllerAction": "reconcile candidate artifacts into a deliberate child-task or retirement record; do not silently leave them against a null-binding fail-closed ledger",
      "invariants": [
        "SHARE-002 remains unresolved-decomposition until that reconciliation",
        "ownershipDecision remains null",
        "deterministic-data-keying-merge-and-snapshot-size-policy remains unresolved",
        "event-subscription-coalescing-backpressure-and-retry-lifetime remains unresolved",
        "SHARE-003 enterprise-remote missing-spec linkage remains unchanged"
      ]
    }
  },
  "forbidden": [
    "worker edits to ralph.json, FEATURES.md, sources, validators, tests, or product code",
    "acceptance flip based on local queue tests",
    "treating a pure queue as ownership of merge, snapshot, persistence, network, secret, or hosted lifecycle behavior",
    "weakening or deleting bounds, unresolved partitions, or missing-spec evidence"
  ]
}
```

### Proposal invariants

The following must remain true until the controller supplies new source-grounded authority:

```text
ownershipDecision[SHARE-002] == null
ledger[SHARE-002].category == unresolved-decomposition
ledger[SHARE-002].reasonKey == sharing-family-not-decomposed
ledger[SHARE-002].implementationCommits == []
sharingGap.reviewedPartitions[share-event-subscription-and-coalescing-queue].boundEstablished == false
unresolvedPartitions contains deterministic-data-keying-merge-and-snapshot-size-policy
unresolvedPartitions contains event-subscription-coalescing-backpressure-and-retry-lifetime
enterpriseRemoteGap.storyId == SHARE-003
enterpriseRemoteGap.missingKinds == [spec]
```

These preserve the unresolved merge/snapshot bounds. The task card's local `4096/8MiB/256` queue proposal is not substituted for the upstream source gap's absent item/byte/session bound or for snapshot-size policy.

## Validation receipt

Read-only checks run on candidate `c8f6dcb`:

```text
python3 -m json.tool tasks/completion/claims.json                 PASS
git diff --check                                                PASS
python3 tools/validate_repository.py                            FAIL: 51 errors
python3 tools/validate_plan.py                                  FAIL: 51 errors
python3 tools/convergence_gate.py                               FAIL: total=80
```

The guard failures are expected and preserved. No test, product, protected, source-ledger, Ralph, or FEATURES file was modified by this audit. The ledger row is intentionally `blocked`; this worklog does not emit acceptance.
