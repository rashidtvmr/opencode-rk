# GUARD-ROUTE-010 — Guard reconciliation audit for ROUTE-010

## Claim
- Task ID: ROUTE-010
- Session: subagent-guard
- Branch: lane/GUARD-ROUTE-010-20260923
- HEAD: c8f6dcb39d98302f27f04a25897d3880e09e2c1b
- Owner file: worklog/GUARD-ROUTE-010.md only (no canonical/product/test edits)

## Source evidence (exact paths/lines)

### ralph.json (controller-owned, immutable)
- `ralph.json` line 1657-1669: ROUTE-010 entry
  - id: ROUTE-010
  - status: "accepted"
  - userStory: "Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json"
  - requirementIds: []
  - testObligations: ["ROUTE-010-T01", "ROUTE-010-T02", "ROUTE-010-T03", "ROUTE-010-T04", "ROUTE-010-T05"]
  - dependencyIds: []

### sources/routing-ownership-gap.json (pinned 9router commit 17c4cc76877bd1755030a8414f8d0083f48dcccf)
- Line 6: status: "source-reviewed-no-exact-task-owner"
- Line 43-44: ownershipDecision.ROUTE-009: null, ownershipDecision.ROUTE-010: null
- Line 50-51: taskBindingState.ROUTE-009: "generic-disc-002-placeholder-no-task-card-or-worklog", taskBindingState.ROUTE-010: same
- storySurfaceSignatures: ROUTE-008/009 = [account-storage, dashboard-api, dashboard-settings, routing], ROUTE-010 = [routing] only
- surfaceAmbiguity.indistinguishablePair: ["ROUTE-008", "ROUTE-009"]
- surfaceAmbiguity.singletonCaveat: "ROUTE-010 is the only unresolved row left on 9router.routing after local implementations are subtracted, but singleton membership is not a task contract."
- subtractedOwners: ROUTE-001 through ROUTE-007, ROUTE-011 (all with implementation commits)
- frozenOwners: ROUTE-006 (proxy-authority-and-platform-boundary), ROUTE-008 (routing-dashboard-ownership-overlap)

### sources/backlog-exhaustion.json
- ROUTE-010 row:
  - category: "unresolved-decomposition"
  - controllerStatus: "not-started"
  - reasonKey: "routing-family-not-decomposed"
  - evidenceIds: ["NR-AUTH-ROUTING", "NR-CHAT", "NR-GITHUB-LOCK-TEST", "NR-ARCHITECTURE"]
  - implementationCommits: []
  - localEvidencePaths: []
  - surfaceIds: ["9router.routing"]
  - taskCard: null
  - taskStatus: null
  - worklog: null
  - surfaceEvidenceGaps: []

### sources/behavior-surface-rules.json
- 32 rules; one rule (id: "9router.routing", kind: "multi-account-routing") includes all ROUTE-001..011 in featureIds
- 9router.routing patterns: src/sse/**, src/store/**, src/lib/**, src/models/**, src/proxy.js

### Repository files
- tasks/ROUTE-001.md, 002, 003, 004, 005, 007, 011, 012 exist
- tasks/ROUTE-006.md, 008.md, 009.md, 010.md do NOT exist
- No worklog/ROUTE-010.md exists
- No claims.json entry for ROUTE-010

## Validator failures (exact)

### tools/validate_backlog_exhaustion.py (51 error lines)
1. `backlog exhaustion classifies accepted/unknown stories` — ROUTE-010 listed among 86 accepted/unknown stories appearing in ledger
2. `backlog exhaustion summary drifted from live Ralph/classification accounting`
3. `ROUTE-010: routing ownership-gap is stale after Ralph task semantics changed` — line 733-734: validator checks `story.get("status") != "not-started"`; ralph.json shows `"accepted"` for ROUTE-010
4. `ROUTE-010: routing ownership-gap expects no task-level requirement binding` — passes (requirementIds is [])
5. Surface signature check — passes (storySurfaceSignatures matches live rules)
6. Ledger row check (lines 768-778): ROUTE-010 present with correct category, reasonKey, surfaceIds; passes
7. No task/worklog/implementation check (line 775) — passes (all null/empty)

### tools/validate_repository.py (exit=1)
- Same 51-error list from validate_backlog_exhaustion.py embedded in output

## Root cause

The validator at line 733 expects `story.get("status") == "not-started"` for ROUTE-009 and ROUTE-010 in ralph.json. However, ralph.json marks both as `"accepted"` status. The ownership-gap files (routing-ownership-gap.json, backlog-exhaustion.json) correctly classify these as unresolved with no task owner, but the validator's freshness check against ralph.json fails because the ralph.json status was changed from "not-started" to "accepted" for the routing placeholder stories.

This is a controller-level inconsistency: ralph.json accepts ROUTE-010 but provides no task card, no worklog, and no implementation. The ownership-gap audit correctly identifies this as an ownership gap, but the validator treats the ralph.json "accepted" status as evidence of drift.

## Authority disposition

- **Canonical files**: ralph.json, ralph.completion.json, FEATURES.md — controller-owned, NOT editable by this lane
- **Source files**: sources/routing-ownership-gap.json, sources/backlog-exhaustion.json — owned by DISC-003 controller, NOT editable by this lane
- **Validator**: tools/validate_backlog_exhaustion.py, tools/validate_repository.py — verifier code, NOT editable
- **Allowed edit**: worklog/GUARD-ROUTE-010.md (scratchpad), tasks/completion/claims.json (ledger row)

## Patch fields required (controller action)

1. **ralph.json**: Either revert ROUTE-009/ROUTE-010 status to "not-started" (matching validator expectation at line 733), OR
2. **Validator fix**: Update validate_backlog_exhaustion.py line 733 to accept "accepted" status for routing placeholder stories, OR
3. **Ownership acceptance**: Create task cards (tasks/ROUTE-010.md) with worklogs and implementation, then update routing-ownership-gap.json and backlog-exhaustion.json to reflect the decomposition

## Remaining limitations

- The 51 validator errors are repo-wide and not fixable from this guard lane (no canonical/test/validator edits allowed)
- ROUTE-010 remains blocked: no source-grounded task decomposition exists, no one-to-one native Rust boundary identified
- The 9router.routing surface is shared across ROUTE-001..011; 7 are locally implemented, ROUTE-008 frozen, ROUTE-009 indistinguishable from ROUTE-008; only ROUTE-010 remains as a routing-only residual with no task contract
- Singleton arithmetic (ROUTE-010 is last routing-only row) explicitly rejected as ownership proof per routing-ownership-gap.json singletonCaveat and validator line 761

## Status: blocked

No product/test/validator/canonical edits permitted by lane boundary. The audit confirms ROUTE-010 ownership gap is correctly identified but the validator cannot pass without controller-level changes to ralph.json status, routing-ownership-gap.json, or new task/worklog decomposition.
