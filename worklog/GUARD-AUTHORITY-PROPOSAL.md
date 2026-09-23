# Atomic repository-guard authority proposal — 2026-09-23

## Decision requested

Authorize a new frozen backlog-reconciliation contract that distinguishes
**controller status**, **evidence binding**, **ownership award**, and **release
acceptance**. This is the recommended resolution of the 51-error contradiction.
It does not authorize deleting evidence, weakening accepted flags, awarding null
ownership, or marking an application parent complete.

No mutation is made by this proposal. The controller/human and repository code
owners must approve and apply the eventual patch atomically.

## Exact proposal base

- Candidate: `71db0693143394cd9232b6f4ad5ae0a4ae255139` on
  `lane/PHASE1-product-spine-20260923`.
- `ralph.json`: `8d1e89f91b4241a78a6d2f4188e5e73ae989889e28d8ba036c8448c84d4e3ecb`.
- `sources/backlog-exhaustion.json`:
  `b39952278dd59d9d7c66ad9c169337a04a72e52e84ec9b94202f9e40ca7718a9`.
- `tools/validate_backlog_exhaustion.py`:
  `dc54eed6631c7556ef24a10ef87f5a15e934e842aa7e6479009397e8e982fd58`.
- `tasks/completion/claims.json`:
  `00d8d99b580a7a13927a36e7f09b115e511e5e915283e59f86aa8b3e80921c82`.
- Exact current results: convergence 80 findings; repository validation reaches
  backlog exhaustion and reports 51 errors.

Any authority patch must abort if these inputs have changed and must regenerate
its proposal from the new exact revision.

## Conflict to resolve

Guard receipt `924301e:worklog/GUARD-CONTRACT-CONFLICT.md` demonstrates that the
current validator requires stale statuses and absent task/worklog files while
immutable live state contains accepted Ralph rows and real evidence. The conflict
cannot be repaired while simultaneously preserving accepted flags, preserving
evidence, and preserving the old status/absence assertions.

The critical semantic error is treating these independent dimensions as one:

1. Ralph status records a controller decision.
2. Task/worklog existence records an evidence binding.
3. Ownership-gap files may still record `ownershipDecision: null`.
4. Parent/release acceptance remains false until integrated journeys and external
   evidence pass.

An accepted Ralph row and an existing task card must not automatically award
ownership or erase a documented parent blocker.

## Authorized atomic patch shape

The authority patch should modify only the reviewed canonical set:

- `tools/validate_backlog_exhaustion.py` and its independently authored frozen
  fixtures/tests;
- `sources/backlog-exhaustion.json`;
- `sources/routing-ownership-gap.json`;
- `sources/operations-ownership-gap.json`;
- `sources/release-assurance-gap.json`;
- `sources/req017-extensibility-ownership-gap.json`;
- `sources/sharing-ownership-gap.json`;
- `sources/extensibility-remaining-ownership-gap.json`;
- `sources/integrations-ownership-gap.json`;
- any deterministic summary/checksum manifest directly derived from those files.

For each affected story, reconcile the live Ralph status and actual task/worklog
paths while preserving its independently supported ownership value. Where the
source currently says ownership is null, it remains null until source-grounded
authority awards or retires it. Existing blockers, equivalence classes, missing
callers, and acceptance caveats remain verbatim or become stricter.

The new validator must fail closed on all of the following:

- a live Ralph status that differs from the reconciled source record;
- a task/worklog path that differs from the actual canonical path;
- an ownership award inferred merely from task number, accepted status, module
  existence, isolated GREEN, or worklog existence;
- missing or newly fabricated evidence paths;
- any story represented in mutually exclusive exhaustion categories;
- summary/count/checksum drift;
- a parent marked complete while its own record admits follow-up, missing caller,
  partial wiring, absent acceptance, or external proof blockers.

## Explicitly not authorized

- No Ralph status change, accepted-flag demotion, or release acceptance.
- No deletion, rename, or concealment of a task card or worklog.
- No ownership award and no inference from an implementation file.
- No weakening of repository protection, CODEOWNERS, tests, checksums, or evidence
  requirements.
- No application of the separate 78-alias convergence proposal in the same
  commit. That proposal has its own hash lock and review boundary.

## Required review and verification order

1. Independent test author creates compiling RED fixtures for all four state
   combinations: accepted+bound+owned, accepted+bound+null-owner,
   in-progress+unbound, and malformed contradictory state.
2. Freeze hashes and exact commands before validator implementation changes.
3. Controller applies source reconciliation plus validator implementation in one
   commit; no intermediate canonical state is pushed.
4. Run focused validator tests, `tools/validate_backlog_exhaustion.py`,
   `tools/validate_repository.py`, and `tools/convergence_gate.py` on that commit.
5. Independent verifier confirms that formerly attributable 51 errors disappear
   only because records now match reality, while null ownership and parent
   blockers still fail any attempted acceptance claim.
6. Protected-path owners review and land without force-push. Hosting-platform
   protection remains a separate externally verified requirement.

Until that authority process completes, the repository guard remains blocked and
the current frozen validator and canonical source records remain unchanged.
