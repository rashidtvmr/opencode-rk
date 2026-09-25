# Phase 1 guard status rollback proposal

Status: proposal branch only. User authorized creating/pushing this reviewed
protected-file proposal, but explicitly did not authorize a PR, merge,
acceptance, validator change, or release claim.

Base: authority-review manifest commit
`8b1785e` (`origin/plan/PHASE1-GUARD-RECONCILE`).

## Applied field scope

- Compared current `ralph.json` to exact parent reference
  `009c094:ralph.json`.
- Restored exactly 82 changed `userStories[].status` values: 44
  `accepted -> in-progress` and 38 `accepted -> not-started`.
- Verified every resulting story status equals the parent reference; no other
  `ralph.json` field was rewritten.
- Ran `python3 tools/validate_backlog_exhaustion.py --sync-features` only after
  this explicit user authorization. It synchronized the corresponding
  `FEATURES.md` mirrors. `--write` was never run and
  `sources/backlog-exhaustion.json` remains untouched.

## Read-only validation result

`python3 tools/validate_backlog_exhaustion.py` after rollback and mirror sync
reports 53 errors, not the 51-error residual predicted by the earlier manifest:

- 27 newly non-accepted out-of-category stories are absent from the exhaustion
  ledger/classification (`PROV-015..024`, `UI-019`, `TOOL-016..020`,
  `SYNC-001/002`, `RUN-001`, `ACP-001/002`, `WSX-001/002`, `SDK-001/002`,
  `HEAD-001/002`);
- 43 pre-existing controller-accepted stories remain classified in the ledger;
- `PROV-015` has no validator classification;
- `WEB-006..008` task-card/accounting classifications disagree;
- routing, operations, release, REQ-017, sharing, extensibility, and integration
  gap records still require deliberate authority review.

This discrepancy is preserved rather than hidden. The proposal does not edit
the ledger, category lists, gap records, validator, tasks, worklogs, or accepted
flags beyond the exact 82 parent-status restorations. Integration authority must
decide whether each residual story is reopened/classified or whether its
evidence supports a reviewed accounting change.

## Safety and handoff

- `git diff --check`: required before commit.
- Product code and frozen tests are untouched.
- Repository validation is expected to remain RED because the residual 53
  errors are the purpose of the authority queue, not worker-fixable noise.
- No PR will be opened per user instruction. The pushed branch is review input
  only.
