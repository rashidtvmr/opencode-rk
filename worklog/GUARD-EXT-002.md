# GUARD-EXT-002 reconciliation audit

## Claim and candidate

- Task: `EXT-002`; audit owner: `ses_f3c4de578ffelQv59xDXmOs03B`.
- Candidate base: `06ed486`; branch `lane/GUARD-EXT-002-20260923`.
- Scope: this scratchpad and the `EXT-002` coordination row only.

## Exact evidence

- `ralph.json:866-881` marks EXT-002 `accepted`, binds REQ-017, and lists T01-T05.
- `tasks/EXT-002.md` and `worklog/EXT-002.md` now exist and describe a built-in plugin wiring candidate.
- `sources/req017-extensibility-ownership-gap.json:43-51` retains null ownership and null task/worklog binding for EXT-002; EXT-001 and EXT-002 remain surface-indistinguishable.
- `sources/backlog-exhaustion.json:121-145` retains unresolved decomposition, null task/worklog, and no implementation commits.
- `FEATURES.md:47,293,615` projects accepted status, while `ralph.completion.json` states legacy accepted status is not release evidence.
- `tools/validate_backlog_exhaustion.py:1400-1417` requires the story to remain `in-progress` and requires its task/worklog to be absent.

A prior read-only validator run reported exactly two EXT-002-specific failures: stale Ralph semantics and task/worklog appearance. Those assertions conflict with the live accepted row and preserved evidence files.

## Disposition

**Blocked.** The audit cannot award ownership, change accepted controller state, delete evidence, or edit the frozen validator. The controller must resolve the contract conflict and then atomically reconcile Ralph, the REQ-017 ownership gap, backlog exhaustion, and FEATURES. Null ownership, EXT-001 equivalence, runtime/plugin-host wiring, permission, cancellation, and bounded-resource blockers remain in force. No acceptance is claimed.

## Verification

- `python3 -m json.tool tasks/completion/claims.json`: required before landing.
- `git diff --check`: required before landing.
- No Cargo, network, browser, database, product, test, canonical, or validator command/edit belongs to this audit lane.
