# AUTO-006

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-002.
Dependencies: none.
Test obligations: AUTO-006-T01, AUTO-006-T02, AUTO-006-T03, AUTO-006-T04, AUTO-006-T05.

## User-observable outcome

Autonomous-loop driver slice: computes the ready queue from `ralph.json`
plus milestone ranks, leases exactly one owned file per lane, runs gate
verification before commit, commits accepted work per milestone order, and
stops cleanly when no ready work remains. No unbounded lanes, no shared-file
races, no acceptance without a passing gate, no spin when the queue is empty.

## Source evidence

- Commit `863a0019fc6f0b9779d867792fe00c4a9ab84c87` (HEAD at card authoring).
- PLAN.md sections 5-6: slice independence rules, one owned file per lane,
  serialized integration lane, mandatory RED-then-GREEN lifecycle.
- PLAN.md section 8: unattended never unbounded; stop when no safe ready work
  remains; retry only classified repairable failures.
- docs/TDD.md sections 2-5: lifecycle order, compiling RED, frozen hash,
  GREEN minimum.
- requirements/user-requirements.json REQ-002: "Plan file Ralph JSON
  independent slices and autonomous loop"; tasks AUTO-001..AUTO-007, mandatory.
- tools/plan_model.py PHASE_RANK (`AUTO: 6`) + PHASE_OVERRIDE
  (`AUTO-001: 0, AUTO-002: 0` only): AUTO-006 has no override, rank 6 (M6
  hardening/completion milestone gating with synthesized lower-rank deps);
  `Plan::ready(accepted, blocked)` returns rank-sorted ready queue,
  `blocked_dependents` reports waiter set.
- docs/AUTONOMOUS_EXECUTION.md sections 1-4: ready-queue contract, one leased
  task plus one owned worktree per worker, mandatory verification commands,
  serial mainline-only integration (`finalize_worktree`), append-only
  receipts, `stopWhenNoReadyWork: true` exit 0.
- tasks/TOOL-016.md: canonical card format followed here
  (Status/Kind/contract/test obligations/failure states/bounds/TDD/verification).
- tasks/AUTO-004.md: sibling REQ-002 card precedent (deps none, milestone
  gating via plan_model.py, frozen T01..T05 asserts).
- Classification: new user requirement (REQ-002 autonomous loop driver) over
  prototype controller reference (`tools/ralph_loop.py`: `ready_queue`,
  `run_task`, `record_result`, `finalize_worktree`). Deliberate
  resource-bounded deviation: bounded lanes, one-file lease, gate-before-commit,
  stop-when-empty; full parallel orchestration tuning stays operator-owned via
  `config/controller.settings.json`.

## Observable contract

- `ReadyQueue::next(plan, accepted, blocked, limit) -> Vec<TaskId>`: returns
  only `not-started` tasks whose dependencies (authored plus synthesized
  rank edges) are all accepted; blocked tasks never returned; dependents of
  blocked tasks reported as `waiting_on_blocker`, not ready; output sorted by
  `(rank asc, id asc)`; truncated to `limit`.
- `Lease { task_id: TaskId, owner: OwnerToken, file: OwnedPath }`: one lane
  owns exactly one file; acquiring a second file or a file owned by a live
  lease fails; only the owning lane may release/renew.
- `drive(batch)`: for each leased lane, run worker, then run gate
  (`validate_repository` + lane gate), then commit: gate PASS commits the
  worktree branch and fast-forwards mainline per milestone order; gate FAIL
  never commits and follows retry-then-block.
- `stop()`: when ready queue is empty, exit 0 with blocked-task report and
  receipt path; never spin, never weaken tests, never invent credentials.
- Suggested module boundary: `crates/autonomy/src/driver.rs` (new crate
  `opencode-rk-autonomy`); worker ships additive fragment only, never edits
  shared `lib.rs`, `Cargo.toml`, schemas, migrations, controller state.
- Stdlib + Tokio only; no new dependency, no network, no secret logging.

## Failure states

- Empty queue: `Ok(Stopped::NoReadyWork)` with blocked report; no worker
  spawned, no commit attempted.
- Gate FAIL: no commit; lane retried up to attempt cap, then `blocked` with
  exact failure (`verification failed` or `worker exit <code>`); receipts
  appended, never rewritten.
- STUB artifact (missing owned file, marker API absent, short/placeholder
  body): gate reports `STUB`/`MISSING`/`INCOMPLETE`; treated as gate FAIL,
  never committed.
- Lease conflict (second file, stolen lease, non-owner release): `Err(LeaseDenied)`;
  lane state unmutated; live lease never deleted by a stale lane.
- Integration conflict (fast-forward fails after concurrent writer landed):
  task stays `accepted`, flagged `accepted but not integrated`, branch preserved
  for integrator; never force-push, never drop the branch.
- No changes to the user's existing OpenCode database; tests use disposable
  fixtures only.

## Resource bounds

- No unbounded queue: ready batch bounded by `max_lanes: usize` (small-machine
  default 1-2, ceiling from operator settings); over-cap tasks wait, never queue.
- No unbounded retained output: per-lane tails capped (e.g. 12 lines / 4 KiB
  with truncation marker); full logs flow to bounded receipts only.
- Owner/cancel path: every lane has one owner token; per-task wall-clock cap
  (`perTaskTimeoutSeconds`) yields timeout exit, reclaims worktree lease, joins
  lane thread; cancellation asserts task/lease reclamation, not just a status string.
- No per-agent OS process for orchestration bookkeeping; workers run through the
  bounded adapter; no detached lane without an owner; no background thread beyond
  the lane pool, all joined before stop.

## Test obligations (frozen)

- AUTO-006-T01 (ready queue respects ranks): fixture plan with rank-0 accepted
  and rank-6 pending: `assert!(ready.contains("AUTO-006"))` only after lower
  ranks accepted; `assert!(ready.windows(2).all(|w| rank(w[0]) <= rank(w[1])))`;
  blocked dep => dependent absent from ready and present in
  `blocked_dependents`.
- AUTO-006-T02 (one-file lease enforced): `acquire(task, owner, file_a)` ok;
  `acquire(task, owner, file_b)` => `assert_eq!(err, LeaseDenied)`;
  `acquire(task, other_owner, file_a)` while live => `assert_eq!(err, LeaseDenied)`;
  non-owner release => `assert_eq!(err, LeaseDenied)` and lease retained.
- AUTO-006-T03 (gate blocks STUB): STUB owned file (missing markers) =>
  `assert_eq!(gate(task), STUB/INCOMPLETE)`; `assert!(!committed(task))`;
  lane status after cap => `assert_eq!(status, blocked)` with `last_error`
  containing `verification failed`.
- AUTO-006-T04 (commit per milestone): all-green batch across ranks =>
  `assert!(integrated_in_rank_order)` (commit sequence non-decreasing by rank);
  ff-conflict fixture => `assert!(status == accepted)` with `last_error`
  containing `accepted but not integrated` and branch preserved.
- AUTO-006-T05 (stop when empty): empty ready queue => `assert_eq!(drive(), Stopped::NoReadyWork)`;
  `assert_eq!(workers_spawned(), 0)`; `assert_eq!(commits(), 0)`; exit code 0
  with blocked report; second `drive()` call identical (no spin, no state drift).

## TDD steps

1. Inspect: PLAN.md 5-6, 8, TDD.md 2-5, REQ-002, plan_model.py rank 6,
   AUTONOMOUS_EXECUTION.md 1-4, TOOL-016 format (done, see evidence).
2. Contract: defined above.
3. Author tests AUTO-006-T01..T05; establish compiling RED (fail: no driver module).
4. Freeze test hash + command manifest.
5. Implement minimum native Rust ready-queue + lease + gate-then-commit driver.
6. GREEN, refactor, rerun; negative tests (blocked deps, lease theft, STUB gate,
   ff-conflict, empty-queue stop).
7. Evidence + patch; verifier decides acceptance.

## Verification

```bash
git status --short -- tasks/AUTO-006.md
cargo test -p opencode-rk-autonomy
cargo check --workspace
```
