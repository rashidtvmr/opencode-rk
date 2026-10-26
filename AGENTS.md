# Agent contract — Recovery Mode / Convergence V2

This repository is in **product-convergence mode**. The goal is a working OpenCode-compatible Rust application, not a larger task graph.

## Current user operating instructions — 2026-10-26

These explicit user instructions take precedence over conflicting delegation,
parallelism and reference-selection guidance below. Persist them across restarts
and compaction; do not require the user to repeat them. They remain in effect
until the user changes them. Token allowances are today's user-provided budgets,
not verified provider balances; never assume that a new session resets them.

### Exclusive subagent allowlist

Only the following subagents are authorized. Do not use another agent or model
route, including an automatic fallback, without a new explicit user instruction.

| User-authorized subagent | Limit and availability rule |
| --- | --- |
| `@vyce-deepseek-v41` | At most **8 concurrent** assignments; unlimited cumulative use. |
| `@vyce-glm-5-3-flash` | **50 million tokens** total allowance; stop using this route when exhausted or on an error. |
| `@9router-vyce-deepseek-v4.1-free-combined` | No user-specified usage limit. |
| `@xkiro-gpt-6-luna` | Authorized for use; resolve the exact harness identifier before dispatch. |
| `@9router-xk-qwen38-max-free` | **8 million tokens** total allowance; stop when exhausted or on an error. |
| `@9router-luna-free` | Use if working; move work to another allowed route if unavailable. |
| `@9router-th-dsv41-flash-free` | Use while its free allowance remains; stop when exhausted/unavailable. |

Verify identifiers against the harness inventory. If a user spelling is an alias
or typo, establish its exact mapping rather than guessing or silently choosing
another model. In particular, resolve the spelling of `xkiro-gpt-6-luna` against
the actual harness inventory before use. Authorization of a route does not waive its quota.
Track reported token usage, route failures and exhaustion in the orchestrator
scratchpad across all assignments and sessions. If usage telemetry is absent,
record **UNKNOWN**, never invent a balance or claim a budget remains. Stop a
budgeted route when its reported usage reaches the allowance or the provider
reports exhaustion; respect the stricter stop-on-error rules above. Reassign its
unfinished task to another allowed, available route and preserve its evidence.

### Continuous delegation and prompt integration

- Maintain **20 or more concurrent, useful subagent assignments** while product
  tasks remain and independent work is available. Replenish completed assignments
  promptly; do not wait for unrelated workers or a whole batch to finish.
- Cover research, upstream comparison, implementation, independent review, audit,
  testing, integration preparation, merge preparation and blocker diagnosis.
  Reassign pending work as capacity becomes available. If dependencies, available
  routes or resources prevent 20 useful assignments, record the actual constraint
  and work to unblock it; do not manufacture duplicate tasks or claim idle agents
  are active. The eight-assignment limit for `vyce-deepseek-v41` still applies.
- Keep implementation path grants non-overlapping and heavy validation serialized.
  Parallel research/review capacity does not authorize concurrent Cargo/PTY runs,
  uncontrolled writers, new branch families or weakened semantic assertions.
- Integrate each independently reviewed, verified, ready feature into `main-v2`
  promptly through the **single integration writer**. Integration/merge workers
  prepare scoped patches, exact hashes, reviews and checks; they do not race to
  mutate canonical history. Do not hold a ready feature for unrelated work.
- Investigate blockers, repair them, and continue. Redirect other workers to
  independent work while the blocker is being resolved. Preserve negative review
  verdicts until the corrected exact candidate is reviewed again.

### Explicit roles, goals and success criteria

Every delegation prompt must assign the worker's **role first**, followed by its
goal and observable success criteria. Then instruct it to read the applicable
worker best practices. Reading a rules file does not assign a role.

Include the exact base SHA, absolute source/reference paths and upstream pin,
allowed write paths or read-only scope, private artifact directory, dependencies,
resource limits, required evidence and expected handoff. Distinguish research
findings, proposed patches, executed verification and integrated acceptance.
Use `docs/AGENT_WORKER_PRACTICES.md` for the shared best practices and handoff
template; do not substitute a generic "read worker rules" prompt for an assignment.

### OpenCode V2 is the architectural reference

Build a Rust version of **all OpenCode V2 behavior as-is, plus the explicitly
requested additional features**. Resolve ordinary architectural/API/UI decisions
by inspecting the actual V2 source in `/Users/mymac/Projects/tmp/opencode`, rather
than asking the user to decide what upstream already specifies. The current
user-selected reference is **`450e7b92412c780a7b5a75226bb8d49aaca24e22`**;
verify the checkout SHA and cite file/symbol evidence. This selection supersedes
the older OpenCode pin in `sources/upstream.lock.json` for current parity work;
do not silently change that lock or compare against a different revision.
Preserve approved extensions and identify their behavior separately from upstream
parity. Ask only for a genuine unresolved product choice or unavailable human
authority. Completing Phase 1 remains the immediate release priority, not a claim
that wider OpenCode parity is already complete.

### Durable scratchpads are mandatory

- Every research subagent maintains a unique scratchpad containing its question,
  exact source pins/paths/symbols, findings, decision rationale, rejected options,
  uncertainties, evidence/artifact hashes and next steps. Read relevant existing
  scratchpads before repeating research; validate old findings against the current
  source. Update the scratchpad before returning or approaching context limits.
- The main orchestrator maintains `docs/worklogs/ORCHESTRATOR_SCRATCHPAD.md` and
  updates it at meaningful changes and before compaction/handoff. Record current
  SHA, active assignments and session IDs, route usage/errors, decisions, reviewed
  candidates, actual test results, blockers, integration status and next actions.
- Research workers write only their granted unique scratchpad/artifact paths.
  Prefer an **absolute private directory outside canonical** and return its path
  and hash; the orchestrator preserves useful durable notes under
  `docs/worklogs/research/`. A relative `.private-tmp/` or `tests/e2e/T/` inside
  canonical is not an external private directory. Never overwrite another worker's
  notes, user work or shared scratchpad.
- Scratchpads preserve reasoning and point to raw evidence; they are not acceptance
  records or replacements for actual source/test verification. Keep secrets out.

## Temporary user override — finish Phase 1 first (2026-10-21)

The user explicitly directed: "lets ignore the guards and running python script
and focus on compelte the phase-1 task, we can hardne the rules afterward" and
asked that this direction be recorded here. This section takes precedence over
conflicting Recovery Mode process requirements below while completing Phase 1.

- Do not block product implementation, integration or installed verification on
  recovery guard scripts, queue-state transitions, controller migration,
  capability/receipt ceremonies or recovery bookkeeping. Defer process hardening.
- Apply the already independently reviewed observer corrections and repair actual
  failing product behavior. Preserve semantic acceptance assertions; never turn
  an unexecuted or failing product check into a claimed PASS.
- Scoped direct commits on `main-v2` and command-local bypass of the Recovery Mode
  Git hook are authorized for this work when that hook blocks the required edits.
  Do not persistently disable Git hooks or change global Git configuration.
- Reuse the existing fixed lanes and one integration writer. Preserve legacy refs,
  user work, history, databases and preferences; do not force-push or bulk-delete.
- Product permission/security enforcement remains required. This override concerns
  repository orchestration, not the running application's permission broker.
- Run focused tests and the real installed Phase-1 journeys on the exact final
  release SHA. Report procedural checks waived by this override separately from
  product PASS/FAIL, and retain evidence and independent review of meaningful fixes.
- Do not create a replacement orchestration framework or another controller
  migration project. Finish the working local native application first.

## Recovery Mode is active

Read these first, in this order, before changing product code:

1. `docs/RECOVERY_MODE.md`;
2. `docs/PHASE1_ACCEPTANCE.md`;
3. `config/recovery-mode.json`;
4. `tools/recovery_state.py` and `recovery/failures.json` (the tracked file is only the immutable seed);
5. `docs/CONVERGENCE.md` and `docs/AGENT_STRATEGY_V2.md`;
6. `docs/TDD.md` and `docs/SECURITY.md`.

Recovery Mode overrides older merge-wave, Ralph-queue, branch-inventory and heritage-merge execution plans when they conflict. Historical plans, worklogs, claims, branches and worktrees are evidence only.

### Hard recovery invariants

- The unit of work is an **observed failing product behavior**, never a branch, worktree, file count, task-card count or legacy claim.
- Do **not** bulk-merge legacy branches. Read them only to salvage exact useful code/tests into an active failure repair.
- Do **not** create review, verification, custody, repair, candidate or test-owner branches. Those are queue states, not Git topology.
- Only the fixed mutable branches listed in `config/recovery-mode.json` may be created or advanced during Recovery Mode.
- A worker or subagent normally returns evidence, a patch/commit candidate and verification results to its lane owner. It does not create a persistent worktree.
- `main-v2` is the canonical product line. Exactly one integration writer lands there.
- Recovery activation installs a Git `reference-transaction` guard. New local branches, stashes/tags/custom refs, movement/deletion of frozen legacy refs, and post-activation changes to frozen evaluator files are rejected rather than merely detected later.
- A candidate that fails after rebasing/preparing against current `main-v2` is ejected from the integration queue; reopen the failure instead of creating a descendant repair branch.
- Legacy refs are frozen by `recovery/legacy-snapshot.json`. Never reset, force-push, delete or advance them.
- `ralph.json`, completion claims and legacy task inventories are requirement/evidence databases, not schedulers.
- Product progress means the count of failing/unproven Phase-1 behaviors decreases on the exact integrated SHA.
- Mutable queue/gate/salvage state lives under the shared Git common directory (`recovery-state/`), **outside tracked history**. Never hand-edit operational state or commit acceptance bookkeeping; use `tools/recovery_state.py`.
- Before any product claim or implementation delegation, `python3 tools/recovery_guard.py` must be GREEN from the activated canonical Recovery Mode. A bootstrap/controller branch is never a production execution lane.
- A fixed recovery lane owns at most one active failure, starts from the current `main-v2`, has an explicit non-overlapping path grant, and is reset/fast-forwarded after disposition. If `main-v2` moves, stale candidates are reopened/reapplied rather than turned into a branch family.

### Evaluator-optimizer loop (mandatory)

1. The implementer/optimizer produces a candidate on the assigned fixed lane.
2. Independent evaluator(s) review the diff **without inheriting the author's justification first**.
3. P0/P1 require two independent approvals; P2/P3 require one.
4. A security/permission-path change additionally requires an approving security evaluator.
5. **Any** `REJECT`/`NEEDS_FIX` blocks preverification. Reopen the same failure, keep the same fixed lane, produce a new candidate SHA, and review that SHA again; no repair/review branch is created.
6. After optimizer changes, reviewers evaluate the new candidate SHA again; old approvals do not carry forward.
7. A third evaluator may diagnose a disagreement, but a negative verdict can never be outvoted on the same candidate SHA.
8. Recovery controller files, the Phase-1 contract/gate definitions, the judge engine and existing semantic installed-product tests are frozen at activation. The explicitly listed evaluator-sensitive extension points may change only with at least two independent approvals including a `test` evaluator.
9. The optimizer/author may never mark its own candidate accepted.
10. After three failed candidate cycles for the same failure, claiming is blocked until an independent evaluator records a `strategy-review` diagnosis and a materially different approach.

Use `tools/recovery_state.py` for lifecycle transitions. The normal product path is `claim -> candidate -> review -> preverify -> ready -> integrate`, followed by an installed judge and `observe-judge`/acceptance. `integrate` is fast-forward-only and owns the single integration lock. Use `sync-lane` after canonical advances; use `recover-integration` after a crash instead of inventing a current-base/repair branch. `recover-state` restores the last consistent runtime revision after an interrupted multi-file state write.

Legacy search is also state-machine controlled: `salvage-query`, `salvage-status`, and `salvage-dispose`. A query is tied to one current failure, scans the entire frozen snapshot read-only, materializes at most eight distinct tip candidates, and blocks integration until every materialized candidate has a terminal disposition. The old convergence salvage inventory is evidence only and cannot be imported as a new queue.

Every judge result that is meant to drive work must be ingested with `recovery_state.py observe-judge` (or `phase1_judge.py --observe-state`). Never translate judge output into ad-hoc tasks by hand.

Direct canonical writes are forbidden after activation. The Git reference hook requires a runtime capability held by `recovery_state.py integrate`; do not use raw Git to advance `main-v2`.

### Activation / restart safety

- Old merge-wave/controller sessions must be stopped or treated as hostile writers before Recovery Mode starts.
- Unexpected local branches or persistent worktrees are a hard guard failure.
- Recovery Mode contracts must already be present on canonical `main-v2` before product work starts.
- On agent restart/crash, do not infer state from prose/worklogs. Run `tools/recovery_state.py status` and `tools/recovery_guard.py`; inspect any integration lock before doing Git work.
- If runtime JSON revisions disagree, run `tools/recovery_state.py recover-state`. If an integration lock is present, run `recover-integration`; it completes an already-landed candidate, returns an untouched canonical to READY, or fails closed on ambiguity.
- Phase-1 judge evidence is written under the shared Git common directory (`recovery-evidence/<sha>/`), not committed. This prevents acceptance bookkeeping from changing the SHA it certifies.

### Subagents are allowed and encouraged

Use subagents aggressively for independent investigation, upstream comparison, legacy-salvage search, test diagnosis, adversarial review and competing implementation ideas. Subagents may run in parallel up to the configured logical ceiling, but mutable repository state stays bounded:

- read-only/research/review subagents: parallelize freely within resource limits;
- implementation subagents: only for currently OPEN/CLAIMED failures with non-overlapping path grants;
- reviewers/verifiers: return verdicts/evidence; do not create branches;
- only a lane owner may mutate one of the fixed recovery lanes;
- only the integration writer may advance `main-v2`.

When the verified-candidate high-water mark is reached, stop spawning implementers and move subagents to review, salvage lookup, integration prep and failure diagnosis.

## Authority order

Resolve decisions in this order:

1. current explicit user requirement;
2. explicitly approved opencode-rk extension;
3. observable behavior of the current user-selected OpenCode revision recorded
   above, falling back to `sources/upstream.lock.json` only when no user-selected
   revision is in effect;
4. simplest safe Rust implementation preserving that behavior;
5. historical local plans/tests/assumptions.

For ordinary OpenCode parity, **upstream is the product specification**. Inspect pinned upstream before inventing APIs, state machines, provider behavior, routes, TUI behavior or server abstractions.

## HITL policy

Ask the user only for a genuine product/scope choice or unavailable human authority: an intentional user-visible deviation, a new extension with materially different product choices, credentials/OAuth/signing/device/production authority, or an irreversible external action not already authorized.

Do not ask for routine implementation choices, internal API shape, branch/rebase order, fixture maintenance, mechanical test repair, retries, dependency-compatible Rust design, or conflicts whose answer is already determined by the authority order.

## Work packages, not file tasks

The schedulable unit is a coherent observable behavior. A package may own multiple explicit paths when required for one gate. Active packages must have non-overlapping path grants. Shared high-collision files such as `crates/server/src/lib.rs`, `Cargo.lock`, and major CLI entrypoints are serialized by the integrator.

Do not split a behavior into one task per file. Create a child only when independently testable or independently executable.

## Failure-driven loop

1. Run the smallest relevant gate on current canonical SHA.
2. Capture the exact failure.
3. Inspect pinned upstream/current requirement.
4. Assign one coherent repair package.
5. Allow a second competing implementation only when uncertainty warrants it.
6. Independently preverify.
7. Integrate through the single trusted integration writer.
8. Rerun the same gate on the exact integrated SHA.
9. Only then accept and unlock dependencies.

Do not recursively turn every implementation detail into planning/review/registration tasks.

## Parallelism and backpressure

The current user instruction above requires 20+ useful concurrent assignments
when independent product work is available. The configured ceiling and resource
limits still apply; writing packages must be independently useful and non-overlapping.

- candidate high-water: **4** verified/unintegrated candidates;
- at high-water, stop new implementation and move capacity to verification, rebase/integration prep, conflict resolution and upstream analysis;
- exactly one trusted integration writer mutates the canonical branch at a time;
- heavy local validation stays within `config/convergence-v2.json`.

Forty agents may reason in parallel; forty Cargo builds or forty independent writing branches are forbidden.

## Acceptance lifecycle

Use runtime lifecycle: `OPEN/UNPROVEN -> CLAIMED -> CANDIDATE -> PREVERIFIED -> INTEGRATION_READY -> INTEGRATING -> ACCEPTED`.

Only `ACCEPTED` unlocks dependencies. `ACCEPTED` means the required gate passed on the **exact integrated revision**. Worker self-report, isolated GREEN, `claims.json completed`, worklog text, source presence or candidate GREEN is not acceptance.

Historical `ralph.json`, `tasks/completion/*`, claims, worklogs and old controller receipts are requirements/evidence inventory only unless explicitly imported into V2 state.

## Test policy

Implementation workers may not weaken semantic tests to manufacture GREEN.

Semantic frozen contracts include externally visible behavior, security guarantees, protocol semantics and assertion intent. Implementers cannot change these to pass.

Mechanical maintenance includes unused imports, readiness waits, PTY draining, fixture-path repairs, accepted helper renames, formatting/lint/compiler maintenance that does not change asserted behavior. An independent test owner may repair these without HITL.

When an old semantic test conflicts with a higher authority, an independent contract evaluator records the supersession and freezes the corrected contract while preserving Git history. Ask the user only if a real product choice remains.

## Upstream reference

The current user-selected OpenCode reference and absolute checkout path are
recorded in the operating instructions above; they supersede the older OpenCode
pin in `sources/upstream.lock.json` for current parity work. Use the full read-only
checkout at that exact selected pin and record upstream file/symbol evidence.
Do not claim parity from selected excerpts when the relevant upstream source was
not inspected.

## Non-destructive salvage

Pre-V2 branches/worktrees are valuable evidence. Never delete, force-push, reset or prune them during convergence.

Legacy branches receive dispositions only when a current product failure requires inspecting that lineage. Do not create hundreds of disposition tasks merely to complete an inventory. Terminal dispositions are `ALREADY_PRESENT`, `SALVAGED_PRODUCT`, `SALVAGED_TEST`, `SUPERSEDED_BY <ref>`, `DUPLICATE`, `PROCESS_ONLY`, `EVIDENCE_ONLY`, and `CONFLICTING_IMPL_PRESERVED`. There is no durable `PENDING_REVIEW` salvage queue.

Useful code/tests are integrated selectively. Superseded/process-only branches stay preserved in Git ancestry/archive refs without reintroducing obsolete file contents.

## Security and resources

No unrestricted secret access, shell-string concatenation, automatic replay of ambiguous side effects, unbounded queues/output, fake sandbox claims, or modification of the user's existing OpenCode database during tests. Use disposable fixtures and explicit capabilities. Permission `*` cannot bypass mandatory protection or human-only authority.

Run heavy commands serially unless measured headroom proves otherwise. Default `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2`; reduce under pressure. Prefer focused test -> target -> crate -> workspace.

## Completion report

Every implementation handoff states package/gate ID, base SHA, candidate SHA, exact changed paths, upstream/current-requirement evidence, exact verification commands/results, remaining observed failure, and whether the result is PREVERIFIED or ACCEPTED on an integrated SHA.

Never describe planning, registration, a handoff document or a compiling module as product completion.
