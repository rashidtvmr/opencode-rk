# Convergence V2 agent strategy

## Objective

Continuously converge the Rust port toward a production-ready OpenCode-compatible application while preserving all pre-V2 work. Work is driven by observable failures on the integrated product rather than an expanding file-level backlog.

## Canonical branch model

- Historical branches and worktrees are immutable salvage inputs.
- `main-v2` is the canonical convergence branch during the reset.
- One integration writer lands changes on `main-v2`.
- Once all release gates pass on one exact SHA, merge `main-v2` into `main` without force-push.
- Never delete historical branches as part of convergence.

## Product gates

| Gate | Acceptance |
| --- | --- |
| G0 Salvage | every pre-V2 branch/worktree has an explicit disposition and all detached/dirty work is preserved |
| G1 Startup | fresh HOME launches oc2, creates/reuses exactly one authenticated daemon, rejects foreign listeners, no manual serve |
| G2 Provider | in-app/persisted provider configuration reaches the actual outbound provider request and first response streams |
| G3 Tool | provider tool call -> real permission broker -> tool execution -> typed result -> provider continuation |
| G4 Durability | second turn after tool succeeds; exit/restart resumes identical typed history; second client attaches without duplication |
| G5 Native TUI | real PTY uses native OpenTUI, raw input, streaming redraw, resize/suspend/resume and exact terminal restoration |
| G6 Web | real browser uses the same daemon/session engine for prompt, stream, approval/tool, second turn and reload/resume |
| G7 Ubuntu | release archive installs on Ubuntu, native OpenTUI dependency resolves, same golden journey runs |
| G8 Release | Mac + Web + Ubuntu pass on one exact release SHA with security/adversarial smoke checks |

## Parallel topology

Use many agents for independent reasoning, evidence and verification, but keep writing/integration bounded.

Typical high-capacity allocation when the harness supports it:

- 6 gate/train owners;
- 10-12 implementation candidates across non-overlapping packages;
- 6 upstream parity investigators;
- 8 independent verifiers;
- 5 salvage/rebase/integration-prep agents;
- 5 E2E/platform/security agents;
- 1 trusted integration writer.

Do not manufacture work merely to fill slots. The candidate high-water mark is four.

## Package rules

A package has one product gate/failure, base SHA, explicit allowed path set, semantic test manifest or existing product gate, upstream/current-requirement evidence, one primary implementer and an independent verifier. At most two competing implementations are allowed for the same failure.

Packages may span files. Two active packages may not own overlapping paths.

## Integration train

For each candidate:

1. produce from a declared base SHA;
2. independently preverify;
3. integration-prep updates/rebases/cherry-picks onto current `main-v2` without changing behavior;
4. rerun focused verification against the prepared current candidate;
5. the single integration writer lands it;
6. rerun the required product gate on the exact integrated SHA;
7. only then mark ACCEPTED.

If Git state is ambiguous after a crash, inspect ancestry before retrying. Never rerun implementation blindly when integration may already have occurred.

## Backpressure

If verified/unintegrated candidate count >= 4, stop new implementation packages and move capacity to verification, rebase, conflict resolution, upstream analysis and integration support. Resume implementation only after candidates are consumed or rejected.

## Decision policy

Do not ask the user when pinned upstream or an existing approved requirement answers the question. Internal Rust design, branch mechanics, retries, fixture repairs and mechanical test maintenance are delegated engineering decisions.

Escalate only true user-visible ambiguity, intentional deviation, missing external authority, credentials/signing/device authority, or irreversible external actions.

## Test policy

Semantic frozen tests remain protected. Mechanical test maintenance is owned by an independent test role and does not require user approval. A contract evaluator may supersede a historically wrong test when higher authority is unambiguous, preserving audit history.

## Salvage policy

Never equate preservation with blindly merging stale file contents. Preserve history and selectively activate code:

1. snapshot refs, dirty state, untracked files and detached heads;
2. create archive refs/bundle;
3. inventory every branch relative to `main-v2`;
4. integrate unique useful product/test changes by behavior slice;
5. mark duplicates/superseded/process-only branches explicitly;
6. after content salvage, create ancestry-only heritage merges for remaining branches so their history remains reachable without reintroducing obsolete files.

## What does not count as progress

Adding a task card, writing a handoff, isolated module compilation, source-presence checks, worker-reported GREEN, candidate GREEN that is not integrated, or a parent note admitting unwired/follow-up work are not product completion.

Progress is an integrated product gate moving from RED to GREEN.
