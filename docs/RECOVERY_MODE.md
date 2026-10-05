# OpenCode-RK Recovery Mode

Status: **ACTIVE**

Recovery Mode replaces the failed strategy of treating hundreds of divergent Git refs as a merge queue. The existing branch/worktree universe is preserved as read-only evidence. The running product on `main-v2` is the only completion authority.

## 1. Objective

Finish a production-ready **Phase 1 local native application** by burning observable failures to zero on one exact `main-v2` SHA.

Phase 1 is intentionally narrower than the full roadmap. It covers the local macOS native product spine: default launch, daemon ownership/authentication, persisted provider setup and real request, native OpenTUI interaction, permissioned tool execution and continuation, second turn, restart/resume, and terminal/security restoration. Web, Ubuntu packaging, mobile/remote-control and broader extensions remain later gates unless they are required to fix a Phase-1 regression.

The target is not `0 divergent branches`. Historical branches may remain divergent forever.

The target is:

```text
Phase-1 failing/unproven behaviors -> 0
on one exact integrated SHA
```

## 2. Why this mode exists

The previous merge strategies failed because repository state grew while integration consumed it. Review, repair, verification and custody states were encoded as branches/worktrees, ancestry chains were split across buckets, policy review starved the merge engine, and many refs represented duplicate/superseded versions of the same behavior.

Recovery Mode separates four concepts that were previously mixed together:

1. **history preservation** — keep legacy refs reachable and immutable;
2. **salvage discovery** — search legacy refs for useful code/tests for a current failure;
3. **product repair** — write the smallest coherent fix for an observed failure;
4. **acceptance** — pass the judge on the exact integrated SHA.

Only product repair changes canonical code.

## 3. Sources of truth

Authority order:

1. current explicit user requirement;
2. `docs/PHASE1_ACCEPTANCE.md` for the active release boundary;
3. pinned upstream behavior / approved OpenCode-RK extension;
4. current `main-v2` observable behavior and tests;
5. legacy branches/worktrees as salvage evidence;
6. old plans, claims, worklogs and receipts as historical context only.

No legacy branch is authoritative merely because it is newer, larger, GREEN in isolation, or named `final`, `verified`, `repair`, `safe`, `accepted` or similar.

## 4. Git topology: bounded mutable state

Recovery Mode allows a small fixed set of mutable lines only. See `config/recovery-mode.json` for the exact names.

Recommended topology:

```text
main-v2                 canonical integration line
recovery/core           startup + provider
recovery/runtime        tools + sessions/durability
recovery/ui             native TUI
recovery/platform       release/platform-only repairs
controller/recovery-mode-v1  controller/docs bootstrap only
```

The hundreds of pre-existing branches/worktrees remain frozen. They are not deleted and are not individually merged.

### Activation barrier

Recovery Mode is not considered active for product work merely because this document exists on a controller branch. Before the first claim:

1. the Recovery Mode controller/contracts must be committed on canonical `main-v2`;
2. the four fixed lane refs must point at that exact `main-v2` SHA;
3. only the predeclared fixed lane worktrees may be added;
4. `tools/recovery_state.py init` must initialize shared operational state;
5. `python3 tools/recovery_guard.py` must be GREEN from `main-v2`;
6. any old merge-wave/controller process that can still create branches/worktrees must be stopped or treated as a blocking hostile writer.

Unexpected branch or persistent-worktree creation after activation is a fail-closed error, not merely a warning.

### Forbidden branch patterns during Recovery Mode

Do not create new branches for states such as:

```text
review/*
verify/*
custody/*
repair/*
candidate/*
red/*
plan/*
integration/*
*-safe
*-final
*-v2
```

If a candidate needs repair after integration preparation, update/recreate the candidate inside the same fixed lane or reopen the failure. Never create a lineage branch.

## 5. Failure queue is the scheduler

`recovery/failures.json` is an **immutable tracked seed**, not the mutable queue. Mutable queue, gate and salvage state lives under Git's shared common directory in `recovery-state/` and is changed only through `tools/recovery_state.py`. Evidence lives beside it under `recovery-evidence/<sha>/`.

This separation is mandatory: committing an `ACCEPTED` bookkeeping change would create a new Git SHA and invalidate the exact SHA that had just passed. Operational state therefore never participates in product history.

Queue items must originate from one of these sources:

- compiler diagnostics;
- focused/unit/integration/E2E test failure;
- Phase-1 golden-journey failure;
- parity difference against pinned upstream;
- security/adversarial check failure;
- a gate that is still `UNPROVEN` and therefore needs an executable probe.

Do not create work merely because an old branch exists.

Lifecycle:

```text
UNPROVEN/OPEN
  -> CLAIMED
  -> CANDIDATE
  -> PREVERIFIED
  -> INTEGRATION_READY
  -> INTEGRATING
  -> ACCEPTED
```

`ACCEPTED` means the required judge passed on the exact integrated `main-v2` SHA. Anything else is not done.

Claims are atomic leases. `claim` requires one fixed lane at the current canonical SHA plus explicit path grants; overlapping path prefixes are rejected. The same lane cannot own two active failures. If canonical moves before integration, the candidate is stale and must be reopened/reapplied from the new canonical base.

## 6. Agent/subagent operating model

Many agents may reason concurrently. Mutable Git state stays small.

Good parallel work:

- inspect pinned upstream;
- search legacy refs for the strongest implementation/test;
- diagnose a failing test;
- compare competing implementations;
- perform security review;
- independently inspect a candidate diff;
- prepare focused test cases;
- classify a legacy lineage as duplicate/superseded/evidence-only.

Writing work is bounded by fixed lanes and explicit non-overlapping path grants. Multiple subagents may propose patches for the same failure, but only the selected candidate is applied by the lane owner.

Reviewers return structured verdicts instead of branches:

```text
failure_id
base_sha
candidate_sha_or_patch
verdict: APPROVE | REJECT | NEEDS_FIX
evidence
commands/results
security/semantic concerns
```

### Evaluator-optimizer protocol

Candidate generation and candidate judgment must remain independent. For P0/P1 work, two independent evaluators must approve the exact candidate SHA; P2/P3 require one. A security/permission change additionally requires a security-role evaluator. The first review is diff/evidence-first rather than author-rationale-first. A reviewer disagreement invokes a third evaluator or reopens the failure. Any optimizer change produces a new candidate SHA and invalidates prior approvals.

Lifecycle changes must go through `tools/recovery_state.py`; do not hand-edit runtime JSON. The state tool serializes writers, checks path ownership, records candidate-SHA-specific reviews and enforces the candidate high-water mark.

## 7. Selective legacy salvage

When a failure is assigned, search legacy history only for that behavior.

For each useful legacy candidate:

1. identify the merge-base and exact changed paths;
2. determine whether the behavior is already present on current `main-v2`;
3. compare all known lineage variants and pinned upstream/current contract;
4. select the strongest useful implementation/test or mark it superseded;
5. transplant only the necessary code/test into the fixed recovery lane;
6. verify against current `main-v2` behavior;
7. record the disposition in the salvage index.

Never whole-tip merge a historical branch simply to preserve ancestry.

Allowed dispositions:

- `ALREADY_PRESENT`
- `SALVAGED_PRODUCT`
- `SALVAGED_TEST`
- `SUPERSEDED_BY`
- `DUPLICATE`
- `PROCESS_ONLY`
- `EVIDENCE_ONLY`
- `CONFLICTING_IMPL_PRESERVED`
- `PENDING_REVIEW`

## 8. Integration queue

Only the integration writer advances `main-v2`.

For every candidate:

1. ensure candidate base and paths are recorded;
2. independently preverify focused behavior;
3. prepare/cherry-pick/reapply it on the latest `main-v2` without semantic changes;
4. run the focused judge on that prepared tree;
5. if it fails, eject it and reopen/update the failure;
6. if it passes, land it immediately;
7. rerun the relevant Phase-1 gate on the exact integrated SHA;
8. mark the failure `ACCEPTED` only after step 7 passes.

Do not hold a large integration train. The candidate high-water mark is deliberately small.

The single integration writer must acquire the runtime integration lock with `tools/recovery_state.py begin-integration`. Before that transition the tool rechecks that `main-v2` has not moved and that the reviewed candidate is still the fixed lane HEAD. A crash/stale integration must be reopened explicitly; never guess from worklog text or retry blindly. After a candidate is accepted/rejected, the fixed lane is reset/fast-forwarded back to current `main-v2` before another claim.

## 9. Centralized heavy validation

At most one heavy workspace/build/E2E validation runs at once unless measured headroom proves otherwise. Agents should consume the resulting diagnostics rather than each independently launching full builds.

Preferred loop:

```text
run judge once
 -> capture machine-readable failures
 -> parallel diagnosis/repair
 -> integrate one candidate
 -> rerun focused judge
 -> periodically rerun full Phase-1 judge
```

## 10. Repeated failure means process/rule repair

If the same defect pattern appears more than twice, stop fixing instances independently. Update the appropriate architectural rule/test/helper and repair the class of error. The system must get smarter as it runs; it must not create a new branch family for a repeated mistake.

## 11. Today-mode / Phase-1 completion

Today-mode focuses only on `docs/PHASE1_ACCEPTANCE.md`.

The project is Phase-1 ready only when:

- all Phase-1 gates are `ACCEPTED` on one `main-v2` SHA;
- a release build from a fresh disposable HOME passes the golden local journey;
- security/permission denial has no forbidden side effects;
- terminal state is restored after normal exit, interruption and failure;
- no acceptance depends on a legacy branch that is not represented in the integrated tree;
- the failure queue has no Phase-1 `OPEN`, `CLAIMED`, `CANDIDATE`, `PREVERIFIED`, `INTEGRATION_READY`, `INTEGRATING` or `UNPROVEN` item.
- the final installed-artifact judge was run from a clean `main-v2` worktree, its `build.json` source SHA equals current `main-v2`, and **all Phase-1 gates PASS in that one judge result on the same SHA**;
- final judge evidence is stored outside tracked history under Git common-dir `recovery-evidence/<sha>/`; syncing runtime gate state must not create a new commit.

Branch counts are irrelevant to completion.

## 12. What the next prime agent does

On startup:

1. read `AGENTS.md`, this file, `docs/PHASE1_ACCEPTANCE.md`, `config/recovery-mode.json` and `recovery/failures.json`;
2. run `python3 tools/recovery_state.py status` and `python3 tools/recovery_guard.py`;
3. inspect the highest-priority non-accepted failure;
4. spawn read-only/review/research subagents as useful;
5. claim at most the failures that fit the fixed lanes and path grants;
6. search legacy refs selectively for those failures;
7. implement/preverify candidates without creating new persistent branch families;
8. feed candidates through the integration writer;
9. refresh the judge and repeat until Phase-1 failure count reaches zero;
10. run the installed-artifact `tools/phase1_judge.py ... --sync-state` only on clean canonical `main-v2` to certify all Phase-1 gates on one SHA.

If no failure exists but a Phase-1 gate is `UNPROVEN`, the next task is to create/run the executable probe for that gate, not to inspect random legacy branches.
