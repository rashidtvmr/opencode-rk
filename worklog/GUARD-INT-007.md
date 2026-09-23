# INT-007 audit scratchpad

## Claim / status

- Task: INT-007. Session: ses_f320c500bffeObfwmbqSgfPc7.
- Scratchpad: worklog/GUARD-INT-007.md (audit-only owned file).
- Ledger row: tasks/completion/claims.json INT-007 (in-progress, set by claim).
- Final status to set: blocked (audit complete, no canonical/product/test edits).

## Scope

Audit-only. No canonical/product/test/validator edits. Own only this
scratchpad + INT-007 ledger row. Preserve permission, authentication,
transport, cancellation and bounded-resource blockers; no inferred
acceptance from isolated code. Commit/push scratchpad + ledger row on
lane/GUARD-INT-007-20260923 only.

## Source evidence (exact paths / lines)

- tasks/INT-007.md (1-178): full task card. INT-007-T01..T05 obligations.
  Contract: typed-integration-http-handler pure projection, secret-free,
  no I/O/env/persistence/events/transport. Suggested boundary
  `crates/integration/src/handler_projection.rs` (crate absent); actual
  placement deviated to `crates/providers/src/handler_projection.rs`.
- ralph.json:1115-1126: INT-007 `"status": "accepted"`, requirementIds []
  (DISC-002 residual), testObligations INT-007-T01..T05.
- FEATURES.md:61 (table row 19): INT-007 accepted, "quiesced-tree re-run
  formality". FEATURES.md:637, 889: INT-007 (accepted).
- sources/integrations-ownership-gap.json:6-23: storyIds include INT-007;
  equivalenceGroup INT-001/003/005/006/007/009 identical surface
  `opencode.integrations`. ownershipDecision.INT-007 = null.
  reviewedPartitions.typed-integration-http-handler (lines ~71-104): evidence
    source packages/server/src/handlers/integration.ts:1-104
    caller packages/client/src/contract.ts:19-46
    test packages/client/test/contract-identity.test.ts:24-45
    spec packages/client/README.md:3-14
  candidateTaskOwner = null. ownershipBlocker: "INT-004 already implements
  scoped integration metadata/auth-related behavior. Provider-specific
  credentials/OAuth/external protocols are broader than this handler and
  cannot be re-owned by a generic residual INT id."
- sources/backlog-exhaustion.json:585-603: INT-007 entry:
    category: local-implemented-stale
    controllerStatus: in-progress
    taskCard: null
    worklog: null
    taskStatus: null
    reasonKey: integration-family-not-decomposed
    reopenPolicy: source-grounded-task-decomposition-required
    surfaceIds: ["opencode.integrations"]
    evidenceIds: ["OC-SERVER-API","OC-INTEGRATION-HTTP-HANDLER",
      "OC-CLIENT-CONTRACT-TEST","OC-CLIENT-EVENT-TEST","OC-CLIENT-SPEC"]
- tools/validate_repository.py output (run via PYTHONPATH=.):
    line 59: "INT-007: integrations ownership-gap is stale after Ralph
      semantics changed"
    INT-007 listed in "backlog exhaustion classifies accepted/unknown
      stories" array. exit=1. Total convergence findings total=80.
- tools/convergence_gate.py output: CONVERGENCE BLOCKED, total=80.

## Observed scenario

Attributable guard failure: INT-007 state diverges across the audited
surfaces with no single source of truth:

1. ralph.json + FEATURES.md mark INT-007 `accepted` with a task card and
   frozen tests (tasks/INT-007.md; INT-007-T01..T05).
2. tasks/INT-007.md exists (full contract + test obligations).
3. worklog/INT-007.md exists with verification evidence: product sha256
   1a39f608b02d3258b973a2af54b349772f164c9e342210e01d105b89101240ec; test
   sha256 2ba919a4fe1db31fbb530c1a329ffac2f9f26bd7db81200d9b55ca3e47d7c759;
   RED mutation probe (CodeRequired -> AuthFailed => 4 passed/1 failed,
   restored byte-identical); GREEN 5 passed/0 failed (test result
   `int_007_t01..t05` all ok).
4. crates/providers/src/handler_projection.rs (4.7K) +
   crates/providers/tests/handler_projection.rs (11.5K) tracked, committed.
5. CONTRADICTION: sources/backlog-exhaustion.json:601 sets
   taskCard=null, worklog=null, taskStatus=null and reasonKey
   integration-family-not-decomposed (category local-implemented-stale).
   sources/integrations-ownership-gap.json sets ownershipDecision.INT-007
   =null and candidateTaskOwner=null for the typed-integration-http-handler
   partition, with an ownershipBlocker citing INT-004 overlap and no
   source-grounded owner for the generic residual.
6. tools/validate_repository.py:59 emits the stale-semantics failure for
   INT-007 (ownership gap stale after Ralph semantics changed) and lists
   INT-007 among accepted/unknown stories not exhausted by the backlog.

Net: the implementation and frozen-test pass are real (verified 5/5), but
the controller/ralph FEATURES acceptance is NOT backed by a
source-grounded ownership award: the integrations ownership-gap ledger and
the backlog-exhaustion row both remain null-awarded/undecomposed. Per
PLAN.md section 5 and the integration-ownership-gap `ownershipDecision=null`
policy, acceptance cannot be inferred from a passing isolated test or from
"accepted" ralph status while ownershipDecision and taskCard/taskStatus
stay null and the validator emits a stale-semantics failure.

## Integration ownership / source gaps

- INT-004 (crates/providers/src/handler_projection.rs is the integrator's
  additive-fragment home) is cited as the ownership blocker for INT-007's
  partition; INT-004 implementationCommit d8fa4854fcd680e5fe3dfe2b05c6a2151a340056
  (sources/integrations-ownership-gap.json excludedOwners list).
- INT-007 reuses INT-004's module placement; the card's suggested
  `crates/integration/...` crate does not exist (documented deviation in
  worklog/INT-007.md). No separate crate/module owned by INT-007.
- integrations-ownership-gap.json equivalenceGroup explicitly refuses
  assigning the generic residual to a single INT row ("cannot be assigned to
  one residual row by number or status"). INT-007 is therefore part of an
  undecomposed family, not independently owned at the controller level.
- backlog-exhaustion reasonKey integration-family-not-decomposed +
  reopenPolicy source-grounded-task-decomposition-required: controller
  acceptance pending decomposition, not satisfied by one lane's green tests.

## Backlog exhaustion

- INT-007 is counted in the validator "accepted/unknown stories" array
  (tools/validate_repository.py line 10) and separately flagged at line 59.
- backlog-exhaustion.json classifies INT-007 local-implemented-stale with
  null taskCard/worklog/taskStatus -> story not exhausted by backlog.
- convergence_gate.py total=80, CONVERGENCE BLOCKED (family-level:
  integrations ownership-gap stale covers INT-007).

## FEATURES

- FEATURES.md:61 table row 19: INT-007 accepted, "quiesced-tree re-run
  formality" (implies re-run formality, not source-grounded owner).
- FEATURES.md:637, 889: INT-007 (accepted), DISC-002 residual.
- Acceptance in FEATURES is not acceptance in the guarded backlog/ownership
  sense while ownershipDecision=null and the validator emits a stale-semantics
  failure for INT-007.

## Validator

- tools/validate_repository.py: exit=1. Output line 59:
  "INT-007: integrations ownership-gap is stale after Ralph semantics
  changed". INT-007 listed among accepted/unknown stories at line 10.
- The stale-semantics failure is attributable: the ownership-gap asset
  predates the Ralph task/worklog/card state (backlog-exhaustion still nulls
  taskCard/worklog while tasks/INT-007.md + worklog/INT-007.md exist).
- No validator edits made (frozen to auditors per PLAN.md section 7 /
  AUDITOR scope). This audit surfaces the gap; it does not repair it.

## Permissions / auth / transport / cancellation / bounded resource blockers (preserved)

- INT-007 card explicitly excludes: raw key/OAuth secret material, env reads,
  provider authorize/refresh/callback execution, credential persistence,
  attempt-map lifecycle/timers (INT-004), event publication (INT-008),
  HTTP transport/SSE, filesystem, background work, new dependencies.
- worklog/INT-007.md states NoContent/Projection/Err responses carry zero
  secret/env/provider-detail bytes; tests use disposable in-memory vectors;
  no SQLite/OpenCode DB writes in the projection path.
- project() is pure/synchronous, spawns no thread, owns no lifetime beyond
  caller &[..]; MAX_ITEMS=128 bounds the List vec; no unbounded queue.
- Auditor made no changes crossing the permission broker; audit read-only
  (read sources, task card, worklog, claims.json; ran validate_repository /
  convergence_gate which are read-only validators). No product/secret/env/
  network/cancel path was mutated.

## Decisions

- No canonical/product/test/validator edits (out of scope for audit-only
  lane; source of truth is the controller/integrator for ownership
  re-award/retirement).
- INT-007 implementation + frozen tests are green (5/5), but acceptance is
  blocked by: (a) integrations-ownership-gap ownershipDecision=INT-007=null,
  candidateTaskOwner=null, ownershipBlocker citing INT-004 overlap and the
  undecomposed family; (b) backlog-exhaustion taskCard=null/worklog=null/
  taskStatus=null, reasonKey integration-family-not-decomposed.
- These are controller/integrator responsibilities, not lane-level
  repairs. A lane cannot authoritatively award ownership or retire the
  residual family; doing so would edit controller state/policy (forbidden).
- Therefore: set INT-007 ledger status blocked with the bounded note below;
  do not claim completed.

## Remaining unknowns

- Whether the controller intends to (re)award INT-007 an owned file or
  retire/merge the residual INT family (INT-001/003/005/006/007/009) into a
  single source-grounded owner. That decision is outside this audit lane.
- The exact reconciliation commit the controller will use to align
  backlog-exhaustion taskCard/worklog from null to the extant artifacts.
- Verifier acceptance timestamp for INT-007 remains external and is not
  claimed here.

## Blocker note (for claims.json)

"INT-007 audit-complete: impl handler_projection.rs + frozen tests 5/5 GREEN (product sha1a39f608, test sha2ba919a4, RED mutation probe 4/1->5/5), task card tasks/INT-007.md and worklog/INT-007.md present. BLOCKED by upstream ownership/contract gaps outside audit lane authority: sources/integrations-ownership-gap.json ownershipDecision.INT-007=null, partition typed-integration-http-handler candidateTaskOwner=null (ownershipBlocker cites INT-004 overlap + undecomposed residual family); sources/backlog-exhaustion.json INT-007 taskCard=null/worklog=null/taskStatus=null reasonKey=integration-family-not-decomposed. tools/validate_repository.py:59 emits stale-semantics failure for INT-007 (exit=1, total convergence=80). No canonical/product/test/validator edits per audit-only scope; controller/integrator must re-award ownership or retire residual family before acceptance. Permission/auth/transport/cancellation/bounded-resource exclusions preserved (pure projection, no secret/env/persistence/network/events, MAX_ITEMS=128)."
