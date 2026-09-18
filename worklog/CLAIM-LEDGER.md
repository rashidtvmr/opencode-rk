# CLAIM-LEDGER — Task-claim ledger, scratchpads, and commit-per-feature protocol

Status: IMPLEMENTED (lane evidence; verifier decides acceptance).
Base rev: c67b6c0. Date: 2026-09-18.
Bounds: `tools/completion_claims.py`, `tests/completion/test_claims.py`,
`AGENTS.md`, `prompts/COMPLETE_APP.md`, `.agents/WORKER.md` (NEW),
`worklog/CLAIM-LEDGER.md` (this file). No control-plane, task, ralph.json,
FEATURES.md, frozen-test, or verifier edits.

## Claim

Delegated lanes now coordinate through a checked-in claim ledger instead of
trust or chat: a worker claims its task (fail-closed fencing), persists session
state in its own scratchpad, moves status `not-started -> in-progress ->
completed` through legal transitions only, and reports the scratchpad path back
to the orchestrator. Completed features are landed (commit + push; worktree +
persistent `lane/<TASK-ID>` branch for larger lanes; worktree deleted after
merge, branch kept on the remote).

## Evidence (TDD)

RED frozen hash: `3e0b84bdf60e69a4c30c362bcc9c7113f7b631e98688bebd6082358428e70f21`
(`tests/completion/test_claims.py`) — RED observed as ImportError on the new
API (`plan_stories`, `reclaim`, `ROOT`) plus behavioral failures before impl.

GREEN: `python3 -m pytest tests/completion -q` → **51 passed** (13 ledger).
`python3 tools/completion_plan.py --check` → `SPEC OK: additions=109,
legacy=258, scenarios=545`.

## Contract encoded

- Ledger: `tasks/completion/claims.json`, managed ONLY via
  `tools/completion_claims.py` (stdlib, write-through).
- `claim()` fences `in-progress` AND `blocked` tasks (ClaimError); only the
  orchestrator's `reclaim(tid, session, evidence)` clears a foreign claim, with
  a mandatory recorded evidence note; scratchpad retained for reconciliation.
- `update(completed|blocked)` requires a non-empty evidence note (exact test
  commands for completed; exact blocker for blocked).
- `plan_stories()` loads the union plan through `completion_plan.load()` so the
  ledger can never disagree with the plan; statuses in plan files are forced
  `not-started` by the loader, so progress lives ONLY in the ledger.
- `ready_tasks()` excludes live claims and unmet deps (`before-each` policy).
- `scratchpad_report(document, session)` hands worker scratchpad paths back to
  the orchestrator.

## Instruction wiring

- `.agents/WORKER.md` (NEW): mandatory worker-side protocol — read first;
  claim before touching files; scratchpad maintenance; honest status with
  evidence notes; handback items; commit+push per feature; worktree on
  `lane/<TASK-ID>` for big tasks (push branch first, merge, delete worktree
  only, never the remote branch); hard boundaries (no force-push, no test
  edits, one owned file).
- `prompts/COMPLETE_APP.md`: new "Task-claim ledger and scratchpads" section
  (orchestrator pick/claim flow) + "Landing work: commit and push per feature"
  section (landing discipline, branch persistence, orchestrator hash
  verification).
- `AGENTS.md`: extended "Subagent lane gating" with the ledger protocol and a
  new "Landing work: commit and push per feature (mandatory)" section.

## Known pre-existing RED (not this lane)

`python3 tools/validate_repository.py` fails backlog exhaustion (INT-00x stale
ownership gaps). Verified identical on a stashed clean tree — pre-existing,
unrelated to these changes; preserved per contract, not suppressed.

## Remaining unknowns

- Acceptance decision belongs to the independent verifier.
- The 8 bootstrap tests remain RED (parked controller acceptance-flip,
  `worklog/ACCEPTANCE-FLIP-PROPOSAL.md`) — unchanged by this lane.
