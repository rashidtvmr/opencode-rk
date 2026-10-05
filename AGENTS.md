# Agent contract — Recovery Mode / Convergence V2

This repository is in **product-convergence mode**. The goal is a working OpenCode-compatible Rust application, not a larger task graph.

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
5. Rejection/needs-fix returns to the same failure and same lane; no repair/review branch is created.
6. After optimizer changes, reviewers evaluate the new candidate SHA again; old approvals do not carry forward.
7. Disagreement goes to a third independent evaluator or reopens the failure.
8. The optimizer/author may never mark its own candidate accepted.

Use `tools/recovery_state.py claim/candidate/review/preverify/ready/begin-integration/accept` for lifecycle transitions. The tool owns state locks, path leases, independent-review counts and the single integration lock.

### Activation / restart safety

- Old merge-wave/controller sessions must be stopped or treated as hostile writers before Recovery Mode starts.
- Unexpected local branches or persistent worktrees are a hard guard failure.
- Recovery Mode contracts must already be present on canonical `main-v2` before product work starts.
- On agent restart/crash, do not infer state from prose/worklogs. Run `tools/recovery_state.py status` and `tools/recovery_guard.py`; inspect any integration lock before doing Git work.
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
3. observable behavior of the pinned OpenCode revision in `sources/upstream.lock.json`;
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

Parallelism is a ceiling, not a fill target. Many agents may reason, investigate and verify in parallel, but writing packages must be independently useful and non-overlapping.

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

The pinned OpenCode commit is recorded in `sources/upstream.lock.json`. Use the full read-only local checkout at that exact pin when available and record upstream file/symbol evidence. Do not claim parity from selected excerpts when the relevant upstream source was not inspected.

## Non-destructive salvage

Pre-V2 branches/worktrees are valuable evidence. Never delete, force-push, reset or prune them during convergence.

Legacy branches receive dispositions only when a current product failure requires inspecting that lineage. Do not create hundreds of disposition tasks merely to complete an inventory. Allowed dispositions include `ALREADY_PRESENT`, `SALVAGED_PRODUCT`, `SALVAGED_TEST`, `SUPERSEDED_BY <ref>`, `DUPLICATE`, `PROCESS_ONLY`, `EVIDENCE_ONLY`, `CONFLICTING_IMPL_PRESERVED`, and `PENDING_REVIEW`.

Useful code/tests are integrated selectively. Superseded/process-only branches stay preserved in Git ancestry/archive refs without reintroducing obsolete file contents.

## Security and resources

No unrestricted secret access, shell-string concatenation, automatic replay of ambiguous side effects, unbounded queues/output, fake sandbox claims, or modification of the user's existing OpenCode database during tests. Use disposable fixtures and explicit capabilities. Permission `*` cannot bypass mandatory protection or human-only authority.

Run heavy commands serially unless measured headroom proves otherwise. Default `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2`; reduce under pressure. Prefer focused test -> target -> crate -> workspace.

## Completion report

Every implementation handoff states package/gate ID, base SHA, candidate SHA, exact changed paths, upstream/current-requirement evidence, exact verification commands/results, remaining observed failure, and whether the result is PREVERIFIED or ACCEPTED on an integrated SHA.

Never describe planning, registration, a handoff document or a compiling module as product completion.
