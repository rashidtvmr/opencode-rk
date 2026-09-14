# AUTO-003 worklog

## Claim

Harden the existing parallel controller with durable owner-checked worktree leases and an owned heartbeat lifecycle.

## Source evidence

- Candidate base revision: `47d749c` (`feat(providers): plan account recovery cleanup`).
- `PLAN.md:202-220`, especially 210-213.
- `prompts/START_HERE.md:18-30`.
- `tools/ralph_loop.py:1-19,161-164,209-261,283-303,397-423`.

## Target boundary

- Product implementation: `tools/ralph_loop.py`.
- Independent RED tests: one dedicated bootstrap test file owned by the test author.
- Controller/verifier acceptance remains untouched.

## Tests

- Independent frozen test: `tests/bootstrap/test_auto003_leases.py`.
- Frozen SHA-256: `cbcd015b7afd11a4c027664852ba439a9356a102d75c5023815bd3979094c0be`; no edits after hashing.
- The independent author's one allowed initial run imported `tools.ralph_loop` but all five tests errored because `LeaseTable` did not exist. This is missing-API authoring feedback, not behavioral RED.
- Prime then added only the typed lease errors, `WorktreeLease`, and a deliberately permissive `LeaseTable` scaffold. The frozen suite collected and ran normally, establishing valid behavioral RED: 2 passed / 3 failed (foreign-owner acquire, foreign heartbeat, and TTL/capacity/owner-release policy missing).
- Post-implementation focused GREEN: `/usr/bin/python3 -m unittest tests.bootstrap.test_auto003_leases` => 5 passed / 0 failed.
- `tools/ralph_loop.py` compiles with `/usr/bin/python3 -m py_compile tools/ralph_loop.py`.
- `tests.bootstrap.test_validate_plan` => 12 passed / 0 failed; `tools/ralph_loop.py --status` reports the current 219-story controller state successfully.
- The older `tests.bootstrap.test_auto_controller` remains independently stale: 6/11 fail because it hard-codes the former 178-story plan and assumes already-accepted M0 stories are still ready. The failures reproduce against current plan data (`219` stories, `133 accepted`) and are not caused by lease behavior; this task does not rewrite unrelated historical assertions.
- `/usr/bin/python3 tools/validate_plan.py` => `validate_plan: OK  stories=219 requirements=38 obligations=1095 deps_synthesized=True`.
- `git diff --check` passed. `tools/lane_gate.py` is storage-format-2-specific and has no AUTO/controller lane, so it is not applicable.

## Decisions

- Lease state remains under controller-owned `state/`, never in worker worktrees.
- The deterministic `LeaseTable` accepts caller-supplied time and enforces positive finite TTL, owner checks, stale reclamation, and a configured active-lease capacity.
- `DurableLeaseTable` persists a schema-versioned lease snapshot atomically under trusted `state/worktree-leases.json`; loading invalid or over-capacity state fails closed.
- Each controller lane receives a unique owner token before worktree creation. A live foreign lease removes that task from the current run's safe-ready set rather than spinning or stealing it; expired records are reclaimable.
- Worker and verifier subprocesses run through a bounded `Popen.communicate(timeout=...)` loop that renews the owning lease at the configured heartbeat interval. Heartbeat loss kills the child and prevents successful integration.
- The main controller renews once more before serial integration and releases only with the matching owner token. A stale owner is explicitly unable to delete a lease reclaimed by another lane.
- Heartbeat work is not detached: it runs synchronously in the lane thread while waiting on the owned child process. No additional background thread, queue, network path, or dependency is introduced.
- Direct temporary-directory validation reloaded a persisted lease, rejected foreign takeover, renewed it, and owner-released it; a separate owned-process check observed repeated heartbeat renewal while a bounded child slept.
- No new dependencies.

## Remaining unknowns

- None for this candidate. AUTO-004/AUTO-006 still lack source-grounded one-to-one semantics, and AUTO-005 is only constrained to REQ-004 verification territory; those tasks remain unassigned rather than guessed. Controller/verifier acceptance remains external and is not claimed.
