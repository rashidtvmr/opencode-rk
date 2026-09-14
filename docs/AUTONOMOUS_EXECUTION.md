# Autonomous execution: loop contract, recovery, budgets, trust boundaries

Reference: PLAN.md sections 4, 5, 8; AGENTS.md; prompts/START_HERE.md.
Implementation: `tools/ralph_loop.py`, `tools/plan_model.py`, `config/controller.settings.json`.

## 1. What the loop is

The controller is a serial-or-leased, model-agnostic task pump. It does not
implement features itself. It repeatedly:

1. Loads the plan (`ralph.json` + `requirements/user-requirements.json` via
   `tools/plan_model.py`) and operator settings (`config/controller.settings.json`).
2. Computes the dependency-ready queue: a story is ready only when every
   dependency (authored `dependencyIds` plus synthesized milestone-rank edges
   from PLAN.md section 4, see `tools/plan_model.py`) is `accepted`.
   Blocked tasks never return to the queue; their dependents wait, reported as
   `waiting_on_blocker`, not as ready.
3. Claims ready tasks under ownership locks (`feature:<TASK-ID>`), creates one
   isolated git worktree per task under `.worktrees/<TASK-ID>` on branch
   `auto/<TASK-ID>`, and invokes a real worker through the worker adapter
   (`opencode run --agent <id> --format json <prompt>`).
4. Runs the configured verification commands in the task worktree. A task is
   accepted only when verification passes. Otherwise it is retried up to the
   attempt cap, then recorded `blocked` with the exact failure.
5. Records an append-only receipt per attempt in `state/receipts.jsonl` and
   keeps `state/controller.json` as the resumable task ledger.

Mandatory verification commands (operator settings may append, not replace):

- `python3 tools/validate_repository.py`
- `python3 tools/lane_gate.py --run`

## 2. Loop contract

- Only ready tasks run. A worker receives exactly one leased task and one owned
  worktree. It must not edit other slices, controller state, budgets, security
  policy, frozen tests, or verifier config (AGENTS.md authority section).
- Worker text is never evidence. Acceptance requires passing verification
  commands executed by the controller against the worktree, plus the lane gate.
  A worker cannot mark its own task accepted (`passes:true` in worker output
  means nothing; PLAN.md section 6).
- Integration is serial and mainline-only. On acceptance the controller commits
  the worktree branch and fast-forwards the mainline (`finalize_worktree`).
  If fast-forward fails (concurrent writer landed), the branch is preserved for
  the integrator per PLAN.md section 5; the task is still accepted but flagged
  `accepted but not integrated`.
- A lane crash must not kill the loop. Exceptions in a lane are captured as a
  failed result (`exitCode -1`, `verification FAIL`) and flow through the
  normal retry/block path (`ralph_loop.py` batch handling).
- `tasks/<ID>.md` is the worker task card when present; otherwise scope derives
  from the `ralph.json` userStory, requirementIds, and named test obligations.
  The worker prompt text is built by `worker_prompt()` in `tools/ralph_loop.py`.

## 3. State, receipts, recovery

- `state/controller.json`: resumable ledger. Maps every story id to
  `{status, attempts, worker, worktree, last_error, receipts}`. Status values:
  `not-started | running | blocked | accepted`.
- `state/receipts.jsonl`: append-only, one JSON object per attempt:
  `{id, task, worker, exitCode, verification, seconds, workerTail, verifyTail,
  attempt, [integrated, integration]}`. Receipts are never rewritten.
- `state/loop.lock`: singleton guard holding `{pid, started}`. A second
  controller refuses to start while the recorded pid is alive (exit code 3).
- Crash recovery: on load, any task left `running` is requeued to
  `not-started` with `last_error: requeued after unclean shutdown`. This is
  safe only because the singleton lock guarantees no live process owns those
  lanes. Atomic ledger writes (`write tmp + os.replace`) prevent torn ledgers.
- Operator inspection:
  - `python3 tools/ralph_loop.py --status` prints counts, ready size, and
    waiting-on-blocker size.
  - `python3 tools/ralph_loop.py --list-ready` prints the claimable queue.
  - `python3 tools/ralph_loop.py --once [--max-lanes N] [--dry-run]` processes
    one batch then exits.

## 4. Budgets and stop semantics (PLAN.md section 8)

Operator-set budgets live in `config/controller.settings.json` and are the only
authority for cost control. Current defaults:

| Setting | Default | Meaning |
|---|---|---|
| `maxConcurrentLanes` | 6 | max parallel worktrees per batch (small-machine default should stay 1-2; raise only within explicit CPU/RAM/provider budgets) |
| `perTaskTimeoutSeconds` | 3600 | worker wall-clock cap per attempt; expiry yields exit code 124, `worker timed out` |
| `maxAttemptsPerTask` | 3 | retries for repairable failures, then `blocked` |
| `totalBudgetUsd` | 0 | with `budgetMode: free-workers-only`, no paid spend is authorized |
| `budgetMode` | `free-workers-only` | paid model connections require an explicit operator decision |
| `stopWhenNoReadyWork` | true | exit 0 when the ready queue is empty |

Stop conditions (unattended never means unbounded):

- No safe ready work remains: exit 0, report blocked tasks and receipt path.
- Total budget exhausted, required credentials unavailable, a platform
  isolation test fails, or mandatory human authority is required: stop and
  record the exact blocker.
- Retry only classified repairable failures (verification FAIL or nonzero
  worker exit within the attempt cap). After the cap, status becomes `blocked`
  with `last_error` set to `verification failed` or `worker exit <code>`.
- A blocked state is not a completed project. Never bypass authority, reduce
  scope, fabricate credentials, suppress failures, weaken tests, or spin
  forever to force the loop to end.

Small-machine guidance: keep one or two implementation workers by default
(PLAN.md section 8). The shipped default of 6 is a ceiling, not a target;
the operator lowers `--max-lanes` or the settings file on a 2-core/4GiB box.

## 5. Trust boundaries

- Trusted: the controller process, the verifier (frozen tests, exact trees,
  verification commands), operator-owned config (`config/`,
  `sources/upstream.lock.json`), and state under `state/`. The system assumes
  the trusted controller and verifier are trustworthy; JSON validation does not
  make a malicious verifier safe (PLAN.md ADR-007).
- Untrusted: worker output, upstream repositories, issues, plugin text, model
  responses, task artifacts. Workers get an isolated worktree only; they never
  write `state/`. Test source, verifier configuration, resource limits, scope,
  and reference pins are not writable by the implementation sandbox.
- Human-only authority: actual OAuth consent, signing identities, production
  accounts, real grants, destructive operations on user data. Application HITL
  tests use fake trusted grant issuers in isolated fixtures; the loop must
  never ask the live user to approve hundreds of test deletions nor expose
  real `.env` files. Never touch the user's existing OpenCode database; use
  generated test datasets or an explicitly provided read-only copy.
- Sandbox: a prompt or regex is not a sandbox. Real OS isolation (Landlock or
  platform backend) must be configured and tested on the actual platform, with
  inherited capabilities closed, before unattended execution. Configure trusted
  worker, verifier, and sandbox adapters once before using the prototype loop;
  do not invent an adapter that prints success (prompts/START_HERE.md).
- Production hardening (later AUTO tasks): worktree leases, heartbeats,
  parallel ready-queue execution, append-only acceptance records, controlled
  discovery expansion. The current controller runs lanes in a thread pool with
  serial integration; AUTO-003 upgrades leases and heartbeats.
