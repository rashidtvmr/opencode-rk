# Adapter protocol: worker, verifier, and sandbox adapters

Reference: PLAN.md sections 5, 6, 8; AGENTS.md; prompts/START_HERE.md.
Implementation: `tools/ralph_loop.py` (`run_worker`, `run_verification`,
`worker_prompt`, `create_worktree`); `docs/AUTONOMOUS_EXECUTION.md`.

## 1. Roles

- Worker adapter: connects the controller to a real coding CLI/API that can
  attempt exactly one leased task in exactly one worktree. The shipped
  prototype uses `opencode run --agent <id> --format json <prompt>`.
- Verifier adapter: runs frozen, operator-owned verification commands in the
  task worktree and reports pass/fail. Only the verifier result accepts work.
- Sandbox adapter: confines the worker to its worktree with closed inherited
  capabilities (OS-enforced, e.g. Landlock on Linux). A prompt is not a
  sandbox. This adapter is operator-configured; there is no default fake.

Do not invent an adapter that prints success. The prototype loop must not run
until real adapters are configured (prompts/START_HERE.md).

## 2. Worker adapter contract

Invocation (see `run_worker`):

```
opencode run --agent <worker-id> --format json <prompt>
```

- `cwd` is the task worktree (`.worktrees/<TASK-ID>` on branch `auto/<TASK-ID>`).
- `<prompt>` is built by `worker_prompt()` and always contains: role, scope
  (task id, one worktree, forbidden targets), goal, task card pointer
  (`tasks/<ID>.md` or fallback to `ralph.json` story fields), declared user
  story, requirement ids, named test obligations (minimum taxonomy), required
  deliverable (code plus captured command evidence), constraints (stdlib/native
  first, no new deps without a proposal, no emojis, no em dashes), verification
  commands to run, and the blocked-instead-of-fake rule.
- Bounded execution: the adapter call is subject to `perTaskTimeoutSeconds`.
  On expiry the lane records exit code 124 and `worker timed out`; the worker
  process must be reaped so no orphan holds the worktree.
- Capture: stdout plus stderr are captured; the last 12 lines are stored as
  `workerTail` in the receipt. Full logs stay in the worktree or lane scratch
  area, never in `state/` beyond the tail.
- Environment: the worker inherits a restricted environment. No secrets, no
  production credentials, no live user files. Network only through the
  configured worker backend; the controller itself makes no network calls
  except through this adapter.
- Worker pool: `workerPool` in `config/controller.settings.json` (round-robin
  across lanes). Workers are model/backend labels, not trust levels; all
  worker output is untrusted data.

Worker prohibitions (AGENTS.md; repeated in every prompt):

- Only the leased task and its approved paths. No edits to controller state,
  accepted task flags, immutable references, frozen tests, verifier
  code/config, cost ceilings, security policy, dependency acceptance, or
  release criteria.
- No shared-contract edits (schema, migration numbering, manifests, Cargo
  lockfiles, central route registries): propose an additive registration
  fragment for the integration lane instead (PLAN.md section 5).
- Never claim a feature exists because a module compiles or a test was mocked.
- On genuinely unprocessable tasks (missing credentials, network, source pin):
  stop and report `blocked` with exact reason and reproduction.

## 3. Verifier adapter contract

Invocation (see `run_verification`): each command in `verificationCommands`
runs with `cwd` set to the task worktree, sequentially, with stdout plus
stderr captured. First nonzero exit stops the sequence with `FAIL`. The last
12 lines of the combined transcript are stored as `verifyTail`.

Default commands:

```
python3 tools/validate_plan.py
python3 tools/lane_gate.py --run
```

Rules:

- The verifier runs on the controller side, not inside the worker sandbox,
  against the frozen test set and the exact integrated tree.
- The implementer cannot edit the test set, `passes` flags, global scope,
  policies, or budgets. A task-specific RED suite must compile and fail for
  the missing behavior before implementation; the controller freezes its hash
  (PLAN.md section 6, docs TDD contract).
- Compile failures, missing imports, never-executed or disabled tests,
  `#[ignore]`, fabricated logs, and worker-edited `passes:true` are not
  evidence (PLAN.md section 6).
- A purely declarative or discovery task still needs executable validators and
  a captured failing fixture; product tests must not be invented for it.
- Verifier worker pool (`verifierWorkerPool` in settings) names the trusted
  backends allowed to author or run verification; implementation workers are
  never in this trust set for their own tasks (PLAN.md ADR-007: the
  implementer cannot mark its own story accepted).

## 4. Sandbox adapter contract

Required properties before unattended runs:

- One task, one worktree, one branch. Fresh candidate workspaces prevent an
  attempt from changing another slice's state (PLAN.md section 8).
- Explicit capabilities only: filesystem scope limited to the worktree plus
  explicitly granted read paths; no direct secret file access; no
  unrestricted inherited environment; no shell-string concatenation of
  untrusted input; no automatic replay of ambiguous side effects.
- OS enforcement, tested on the actual platform: the backend (Landlock or
  equivalent) must close inherited capabilities. Never grant broad filesystem
  access and assume a prompt protects `.env`. Permission `*` cannot bypass a
  human-only grant or mandatory system protection (AGENTS.md).
- No per-agent OS process for orchestration, no hidden JS runtime in native
  mode, no secret logging. Bounded queues with byte budgets, scoped
  cancellation, lazy services.
- Destructive and host-global commands are forbidden in the loop; tests use
  disposable restricted fixtures. The user's existing OpenCode database is
  never modified (generated test datasets or an explicit read-only copy).
- Failure mode: if the platform isolation test fails, the loop stops
  (PLAN.md section 8). Missing platform support is a `blocked` reason, never
  a cue to retry unsandboxed.

## 5. Adding or replacing an adapter

1. Keep the function signatures (`run_worker(worker, prompt, workdir,
   timeout_s) -> (exit_code, output)` and `run_verification(commands,
   workdir) -> (ok, transcript)`); the ledger and receipt format
   (`state/receipts.jsonl` fields) stay unchanged.
2. Record the adapter choice, version, and platform test result in the
   operator log; adapter config is operator-owned and not worker-writable.
3. Add the adapter's own self-test (worker timeout reaps the child; verifier
   failure stops the sequence; sandbox denies a probe outside the worktree)
   and run it before enabling the loop.
4. Production upgrades (leases, heartbeats, parallel ready-queue, VCS
   integration) belong to the later AUTO tasks, not to silent local edits of
   this prototype.
