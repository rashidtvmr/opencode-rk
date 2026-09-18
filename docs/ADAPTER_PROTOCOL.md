# Native harness adapter protocol (COORD-001)

Binding contract for the completion harness's trusted native delegation path.
Base: HEAD `5af7884cf7637c0760d985da7a03f2f99ccd0c78`.
Status: **protocol defined; host bindings BLOCKED** (section 8). Nothing here
claims a running native worker, sandbox, provider execution, or acceptance.

Authority: `AGENTS.md` worker contract, `PLAN.md` ADR-007 + sections 5/6/8,
`docs/TDD.md`, `docs/SECURITY.md`, `prompts/COMPLETE_APP.md`,
`config/completion-controller.json`, card `COORD-001`
(`python3 tools/completion_plan.py --card COORD-001`).
Scheduler source of truth: `tools/completion_scheduler.py`.
Legacy prototype reference only: `tools/ralph_loop.py`
(`run_worker`, `run_verification`, `create_worktree`, `finalize_worktree`).
The old loop is NOT release acceptance authority until its audit findings are
repaired (`prompts/COMPLETE_APP.md`).

Conventions: `REV` = 40-hex git revision, `HASH` = 64-hex sha256
(`tools/completion_scheduler.py:20-21`). All bounds inclusive.
Evidence command after each section shows how to check the claim from stdlib.

## 1. Roles and exact API shapes

Four async roles. Signatures at `tools/completion_scheduler.py:79-94`.
No other entrypoint may produce candidates, proofs, or acceptance.

```python
class Task:                      # tools/completion_scheduler.py:33-39
    id: str                      # "<FAMILY>-<NNN>", e.g. "COORD-001"
    owned_path: str              # one explicit relative file, no globs/traversal
    frozen_tests_sha256: str     # HASH of frozen RED suite (controller-owned)
    test_obligations: tuple[str, ...]  # e.g. ("COORD-001-T01", ...), unique, nonempty
    dependencies: tuple[str, ...] = ()

class Candidate:                 # tools/completion_scheduler.py:42-46
    task_id: str
    revision: str                # REV, worker's candidate commit
    worker: str                  # nonempty implementer identity
    changed_paths: tuple[str, ...]  # MUST equal (task.owned_path,)

class Verification:              # tools/completion_scheduler.py:50-57
    task_id: str
    revision: str                # revision actually tested (candidate OR integrated)
    verifier: str                # nonempty, MUST != candidate.worker
    frozen_tests_sha256: str     # MUST equal task.frozen_tests_sha256
    test_count: int              # exact int type, >= len(test_obligations)
    test_obligations: tuple[str, ...]  # MUST cover task.test_obligations
    passed: bool                 # MUST be True (identity check, not truthiness)

class Integration:               # tools/completion_scheduler.py:61-64
    candidate_revision: str      # MUST equal candidate.revision
    integrated_revision: str     # REV of mainline after serialize
    integrated: bool             # MUST be True (identity check)

class TrustedAdapter:            # tools/completion_scheduler.py:79-94
    async def execute(self, task: Task) -> Candidate: ...
        # Use actual native subagent API with a frozen one-file OS grant.
    async def verify(self, task: Task, candidate: Candidate) -> Verification: ...
        # Independent verifier executes immutable tests outside worker authority.
    async def integrate(self, task: Task, candidate: Candidate) -> Integration: ...
        # Serialize VCS change on current mainline; prove actual ancestry/change.
    async def verify_integrated(self, task: Task, candidate: Candidate, revision: str) -> Verification: ...
        # Rerun frozen tests/regressions against the exact integrated revision.
```

Pipeline order enforced by `run_rolling` internals
(`tools/completion_scheduler.py:189-203` `finalize`):
`verify(candidate)` -> `integrate` -> `verify_integrated(integrated_revision)`.
Merge/rebase failure or post-merge test failure = NOT accepted, dependents stay
blocked (`tests/completion/test_scheduler.py:99-113`).

Evidence: `python3 -c "import ast;print([f'{n.name}:{n.lineno}' for n in
ast.walk(ast.parse(open('tools/completion_scheduler.py').read())) if
isinstance(n,(ast.ClassDef,ast.FunctionDef))])"`.

## 2. Scheduler binding and caps

```python
async def run_rolling(           # tools/completion_scheduler.py:158-160
    tasks: list[Task],
    adapter: TrustedAdapter,
    *,
    capacity: int = 20,          # 1..20
    max_unverified: int = 20,    # 1..20
    max_attempts: int = 3,       # 1..3
    stage_timeout: float = 3600, # finite, > 0
) -> Report
```

- `capacity`/`max_unverified`/`max_attempts`/`stage_timeout` validated at
  `tools/completion_scheduler.py:168-176`; violations raise `ValueError`.
- Refill on EACH worker completion; path locks held through post-merge proof
  (`tools/completion_scheduler.py:161-167,211-229`).
- One verification/integration pipeline; unverified capacity reserved before
  execution so simultaneous completions cannot overflow the queue (`:218`).
- Controller targets mirror these caps: `config/completion-controller.json:4-8`
  (`targetWorkers`/`maxWorkers`/`maxUnverifiedCandidates` 20,
  `maxHeavyValidations` 1, `maxAttemptsPerTask` 3); `allowBudgetIncrease:false`,
  `respectHarnessConcurrencyLimit:true`. Actual harness/provider limits may
  reduce occupancy; never exceed configured caps.
- `RetryableFailure` (`:24-25`) = adapter-classified repairable failure only;
  never for authority failures. Exceeding `max_attempts` -> `blocked`
  (`:205-208`, `tests/completion/test_scheduler.py:162-174`).
- Ambiguous integration outcome (unknown whether mainline changed) MUST raise
  `Rejected` for reconciliation, never blindly re-run implementation
  (`tools/completion_scheduler.py:200-203`,
  `tests/completion/test_scheduler.py:176-181`).

Evidence: `timeout 60 python3 -m unittest discover -s tests/completion -p
'test_*.py'` (scheduler primitive only; not native/host proof).

## 3. Capabilities: one-file grant and forbidden writes

- `execute` receives exactly one `Task` with one `owned_path`.
  `normalized_path` (`tools/completion_scheduler.py:97-102`) rejects empty,
  absolute, traversal (`.`,`..`), wildcard (`*?[`), backslash, trailing slash.
- `validate_candidate` (`:136-140`): `changed_paths != (task.owned_path,)`
  -> `Rejected`. Out-of-scope file rejected before verification
  (`tests/completion/test_scheduler.py:147-152`).
- Overlapping paths (`overlaps`, `:105-106`) serialize: `a==b`, `a` under
  `b/`, or `b` under `a/`. Same-path lanes run serially; unrelated work
  proceeds (`:224-226`, `tests/completion/test_scheduler.py:154-160`).
- Implementer MUST NOT write: frozen tests, controller state, pins
  (`sources/upstream.lock.json`), security policy, verifier code/config, cost
  ceilings, budgets, dependency acceptance, release criteria, another lane's
  path, shared routes/schemas/migrations/manifests/lockfiles (integrator-only
  per `ralph.completion.json` contract). Violation = `Rejected`/`blocked`,
  never silent acceptance (COORD-001-T03).
- Worker output (prompts, transcripts, model text) is untrusted data, never
  evidence (`AGENTS.md`; `docs/TDD.md` section 6).

Evidence: `python3 tools/completion_plan.py --card COORD-001` (T03 text);
`python3 -m unittest tests.completion.test_scheduler.RollingTests.test_out_of_scope_file_rejected`.

## 4. Distinct test-author/verifier identity and commands

- `validate_proof` (`tools/completion_scheduler.py:143-155`) rejects when:
  `passed is not True`; `task_id`/`revision` mismatch; frozen hash changed;
  `verifier` empty or `== candidate.worker`; `test_count` not exact `int` or
  `< len(obligations)`; obligations not covered. Zero-test, changed-hash,
  wrong-revision, self-verify, omitted-obligation cases in
  `tests/completion/test_scheduler.py:115-145`.
- Test author (RED + freeze) and verifier (GREEN + post-merge rerun) are
  disjoint from implementer. `proof_errors` additionally requires distinct
  `implementer`/`verifier` fields on release receipts
  (`tools/completion_plan.py:218`).
- Verifier runs the frozen command manifest outside worker write authority
  (`docs/TDD.md` sections 1/4; `PLAN.md` ADR-007). Actual test commands are
  recorded per task; worker self-report text never substitutes
  (COORD-001-T04).
- Legacy mapping (non-authoritative): prototype `run_verification(commands,
  workdir)` runs each command with `cwd`=worktree, first nonzero exit stops
  with FAIL (`tools/ralph_loop.py:508-528`); mandatory prepended gates were
  `python3 tools/validate_repository.py` + `python3 tools/lane_gate.py --run`
  (`docs/AUTONOMOUS_EXECUTION.md` section 1). Native binding must re-declare
  its frozen command list; inheriting this list silently is forbidden.

Evidence: `python3 -m unittest
tests.completion.test_scheduler.RollingTests.test_self_verification_rejected`;
`python3 -m unittest
tests.completion.test_scheduler.RollingTests.test_zero_tests_rejected_before_merge`.

## 5. Cancellation and shutdown

- Adapter MUST honor cancellation: on `run_rolling` cancellation, scheduler
  cancels every in-flight `execute`/`finalize` future and joins them
  (`tools/completion_scheduler.py:253-260`). Test adapter proves 20/20 owned
  calls observe `CancelledError` and `active` returns to 0
  (`tests/completion/test_scheduler.py:183-193`).
- `stage_timeout` wraps each stage via `asyncio.timeout`
  (`:184-187,190-198`); expiry surfaces as failure, retried only if
  adapter-classified `RetryableFailure` within cap, else `blocked`
  (`tests/completion/test_scheduler.py:195-199`).
- Shutdown MUST cancel/join all owned native/remote work: no detached tasks,
  no orphan process trees, no lease held past owner death (COORD-001-T05;
  lease/fencing detail belongs to COORD-006, which this protocol does not
  preempt).
- Never invent credentials or extra budget during shutdown/recovery; missing
  authority = `blocked` with exact reason (`docs/SECURITY.md` section 5;
  `ralph.completion.json` contract `externalAuthority`).

Evidence: `python3 -m unittest
tests.completion.test_scheduler.RollingTests.test_cancellation_joins_owned_adapter_calls`.

## 6. No simulated workers; bounded everything

- BANNED substitutes: echo/sleep/PASS adapters, tests that assert on worker
  stdout text, 20 heavyweight CLI processes standing in for native delegation,
  fabricated `passes:true`, zero-test success, edited status flags
  (`prompts/COMPLETE_APP.md`; `config/completion-controller.json:23` note).
  Unavailable native concurrency is reported and bounded, never replaced
  (COORD-001-T02).
- Scheduler-side bounds (trusted, always on): task list 1..10000
  (`:110-111`); `validate_tasks` requires identity + HASH + unique nonempty
  obligations + known acyclic deps (`:116-133`); `Report.errors[tid]` stores
  only exception type name, never adapter log bodies (`:205-208`); retained
  tails (prototype: last 12 lines `workerTail`/`verifyTail`) stay small; full
  logs live in lane scratch, never in controller state.
- Resource envelope from `config/completion-controller.json:12-16`:
  8 GiB host, 2 GiB reserve, stop admitting below 1 GiB available,
  `CARGO_BUILD_JOBS=2`, `RUST_TEST_THREADS=2`, one heavy validation at a
  time. Twenty logical workers != twenty parallel builds
  (`prompts/COMPLETE_APP.md`).
- Provider budget source `config/controller.settings.json`
  (`config/completion-controller.json:19`); a name containing `free` is not
  proof of free authorized access; budget increase requires explicit operator
  decision (`allowBudgetIncrease:false`).

Evidence: `python3 -c "import json;print(json.load(open(
'config/completion-controller.json'))['note'])"`;
`timeout 30 python3 tools/completion_plan.py --check`.

## 7. Legacy prototype mapping (explicitly non-authoritative)

| Native role | Prototype analogue | Gap to real binding |
|---|---|---|
| `execute` | `run_worker(worker,prompt,workdir,timeout_s)` shells `opencode run --agent <id> --format json <prompt>` (`tools/ralph_loop.py:489-505`), prompt built by `worker_prompt` (`:375-393`), isolation by `create_worktree` (`:396-418`) | Spawns one OS process per lane; NOT the native subagent API; forbidden as 20-way substitute (section 6) |
| `verify` | `run_verification(commands,workdir)` (`:508-528`) + heartbeat helper `_run_process_with_heartbeat` (`:451-486`, exit 124 timeout / 125 lease-heartbeat failure) | Runs shell commands, not an identity-separated verifier service; command list operator-owned |
| `integrate` | `finalize_worktree` commit + `--ff-only` merge (`:421-448`); diverged branch kept for integrator | Task-level helper, not the serialized native integrator lane (COORD-005 owns it) |
| `verify_integrated` | None in old loop (accepted-before-integration gap) | Must exist in native binding; post-merge rerun is mandatory (`:197-198`) |

Do not present prototype behavior as native-protocol compliance.

## 8. BLOCKED host APIs (not passed, not worked around)

Each row is a missing host surface. Owner: future COORD/hardware/operator
tasks, NOT this doc. No credential, device, or budget is fabricated to clear
any row.

| # | Missing surface | Why blocked | Unblock condition |
|---|---|---|---|
| B1 | Native subagent spawn/collect API behind `execute` | No native delegate endpoint vendored; only `TrustedAdapter` Protocol shape + synthetic test adapter exist (`FEATURES-COMPLETION.md:36-43`) | Operator supplies API + auth + concurrency limit; COORD-002 proves real occupancy |
| B2 | One-file OS grant enforcement (Landlock/platform backend) | Prompt/worktree isolation is not a sandbox (`docs/SECURITY.md` sections 3-4); backend must close inherited caps and pass a platform test | Platform backend configured + denial probe test green on target OS |
| B3 | Independent test-author service + frozen-hash store | No trusted freeze service in tree; `frozen_tests_sha256` is a scheduler-checked string, not a hosted attestation | Controller-owned freeze pipeline (COORD-004) with hash store outside worker authority |
| B4 | Independent verifier service + identity registry | `verifier != worker` is string inequality; no PKI/token registry binds names to principals | Verifier identity issuer + `verify`/`verify_integrated` execution sandbox (COORD-004) |
| B5 | Serialized VCS integrator lane + mainline credentials | `finalize_worktree` is a local helper; no fencing/receipt atomicity here (COORD-005/006 own it) | Integrator lane with atomic accept+integrate receipts |
| B6 | Lease/heartbeat provider (`leaseTtlSeconds` 180 / `heartbeatSeconds` 30) | Values are config targets; no durable lease store in this protocol | COORD-006 durable leases/fencing/recovery |
| B7 | Provider concurrency/rate/cost source values | `config/controller.settings.json` is operator-owned and not inspected by this lane; limits must be honored, never evaded | Operator file present + COORD-007 budget enforcement |
| B8 | Signing/consent/device authority for release paths | Actual OAuth/signing/production grants cannot be fabricated (`docs/SECURITY.md` section 5) | Human authority events recorded; absent = `blocked` |

## 9. Conformance checklist (per adapter implementation)

1. Implements all four `TrustedAdapter` methods with exact dataclass shapes
   (section 1); `mypy`/AST signature check green.
2. COORD-001-T01: real authorized harness delegates and returns a
   one-file `Candidate` passing `validate_candidate`.
3. COORD-001-T02: missing native concurrency degrades to a reported bound;
   no CLI-process substitution (config note evidence, section 6).
4. COORD-001-T03: cross-lane/frozen-state/policy writes impossible by
   capability, plus `validate_candidate`/`validate_proof` rejection tests.
5. COORD-001-T04: author/verifier identities distinct from implementer;
   actual frozen commands recorded with counts and hashes.
6. COORD-001-T05: shutdown cancels/joins all owned work (cancellation test,
   section 5); no invented credentials/budget in any path.
7. All section evidence commands runnable from stdlib; B1..B8 either cleared
   by their owning tasks or still marked BLOCKED. No row silently dropped.
