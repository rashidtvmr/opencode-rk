# Agent contract — Convergence V2

This repository is in **product-convergence mode**. The goal is a working OpenCode-compatible Rust application, not a larger task graph.

Read `PLAN.md`, `docs/CONVERGENCE.md`, `docs/AGENT_STRATEGY_V2.md`, `docs/TDD.md`, and `docs/SECURITY.md` before changing product code.

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

Use: `READY -> LEASED -> CANDIDATE -> PREVERIFIED -> INTEGRATION_READY -> INTEGRATING -> INTEGRATED -> ACCEPTED`.

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

Every legacy branch gets one disposition: `INTEGRATED_ANCESTOR`, `SALVAGE_PRODUCT`, `SALVAGE_TEST`, `SUPERSEDED_BY <ref>`, `PROCESS_ONLY`, `EVIDENCE_ONLY`, `CONFLICTING_IMPL_PRESERVED`, or `PENDING_REVIEW`.

Useful code/tests are integrated selectively. Superseded/process-only branches stay preserved in Git ancestry/archive refs without reintroducing obsolete file contents.

## Security and resources

No unrestricted secret access, shell-string concatenation, automatic replay of ambiguous side effects, unbounded queues/output, fake sandbox claims, or modification of the user's existing OpenCode database during tests. Use disposable fixtures and explicit capabilities. Permission `*` cannot bypass mandatory protection or human-only authority.

Run heavy commands serially unless measured headroom proves otherwise. Default `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2`; reduce under pressure. Prefer focused test -> target -> crate -> workspace.

## Completion report

Every implementation handoff states package/gate ID, base SHA, candidate SHA, exact changed paths, upstream/current-requirement evidence, exact verification commands/results, remaining observed failure, and whether the result is PREVERIFIED or ACCEPTED on an integrated SHA.

Never describe planning, registration, a handoff document or a compiling module as product completion.
