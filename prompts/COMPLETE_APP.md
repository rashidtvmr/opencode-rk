# Main-agent handoff: deliver the actual opencode2 application

Read AGENTS.md, PLAN.md, docs/SECURITY.md, docs/TDD.md and
`docs/audits/2026-09-17-app-completion.md` before implementation. This addendum
preserves every original requirement and all existing safety/verification gates.
Do not substitute another plan or disconnected helpers for a usable application.

## CONVERGENCE MODE — mandatory before breadth

Read `docs/CONVERGENCE.md` and run `python3 tools/convergence_gate.py` FIRST.
The repository has already demonstrated a failure mode where isolated state-machine
or helper modules pass unit tests while the installed application remains unwired.

While that gate fails, prioritize its executable-spine findings over new leaf tasks.
Do not mark a parent complete when its own evidence says repair child, follow-up,
unwired, unproven, state-only, partial, missing, or no acceptance. A parent remains
open until its full integrated journey passes.

Use worker capacity by role: reserve at least four safe lanes for integration-spine
implementation and two for independent RED/verifier work; use the remaining safe
capacity for parallel feature/audit work. With a 20-worker harness this means up to
14 breadth lanes, not twenty disconnected leaf lanes. With fewer workers, preserve
integration and verification capacity first.

The first hard boundary is the installed local journey:
`opencode2` -> authenticated daemon start/reuse -> in-app provider setup -> real
OpenTUI-backed UI -> provider/tool turn through the security broker -> persistence ->
restart/resume -> second-client consistency. This boundary must become GREEN before
the project spends the majority of capacity on web/mobile breadth.

`tasks/completion/claims.json` is coordination state only. A `completed` row does
not make a dependency or parent accepted unless independent verification,
integration, and post-integration GREEN on the exact revision exist. Off-plan lane
IDs never satisfy plan dependencies.

## Scope and initial actions

The full completion scope is the UNION of legacy `ralph.json`, its requirements,
and `ralph.completion.json` with every included file and subsequently discovered
mandatory child. Old accepted flags and the flat `prd.json` export are historical
records, not release evidence. Do not bulk flip statuses or erase old work.

Run:

```sh
python3 tools/validate_repository.py
python3 -m unittest discover -s tests/bootstrap -p 'test_*.py' -v
python3 tools/completion_plan.py --check
python3 -m unittest discover -s tests/completion -p 'test_*.py' -v
python3 tools/completion_plan.py --audit-legacy
python3 tools/completion_plan.py --ready
python3 tools/completion_plan.py --card APP-001
```

Use the repository's RTK convention when available. Preserve actual baseline
failures and repair them through authorized tasks; never suppress or relabel them.
`--check` and the completion unit tests validate planning/controller primitives,
NOT a working Rust product. `--ready` lists initial audit roots, not live leases.

Start AUD-001..AUD-020 concurrently where supported. Each auditor owns exactly
its `sources/completion/audits/AUD-xxx.json` report. AUD-020 must account for every
legacy ID, including unknown prefixes; AUD-019 must inspect the complete pinned
source inventory, dynamic registrations and separately observed dev delta.
Selectors in cards are discovery hints, not proof that a path/function exists.
Record exact commit/path/symbol, actual command/count/output and classify each
behavior as verified, unwired, missing or unverified. Turn gaps into mandatory
repair children with explicit tests. Parent completion requires every child.

## Rolling delegation: target 20, never fabricate occupancy

Use the harness's ACTUAL native subagent API and completion notifications. Load
`config/completion-controller.json`: target/max 20 workers, one heavy validation,
three attempts maximum, bounded unverified candidates. Actual harness/provider
limits, dependencies, path conflicts and memory may reduce the count. Never
start twenty heavyweight CLI processes as a substitute or evade account limits.

On EACH worker completion, check scope and frozen-test integrity, queue independent
verification/integration, release the execution slot and admit the next eligible,
nonconflicting child immediately. Do not wait for an entire batch. Retain file
ownership until integration finishes. Bounded verification backlog is valid
backpressure; continuous twenty-way occupancy is not more important than safety.

`tools/completion_scheduler.py` supplies a tested async rolling primitive and a
`TrustedAdapter` protocol. It launches no agents by itself. Bind real native
execute/verify/integrate/post-integrate APIs and complete COORD-001..008. The old
`tools/ralph_loop.py` is NOT approved as a release acceptance authority until its
audit findings are repaired. Increasing its old concurrency number alone is not
a solution. Do not create echo/sleep/PASS adapters or claim unit fixtures are real
subagents, real phones, real OS isolation or live provider executions.

## Task-claim ledger and scratchpads (mandatory before fan-out)

Coordination state lives OUTSIDE the plan files: `tasks/completion/claims.json`,
owned by `tools/completion_claims.py` (stdlib, write-through, fail-closed on
collision). `completion_plan.py --check` deliberately forces story statuses back
to `not-started` at load, so never record progress in `ralph.json`,
`ralph.completion.json` or story rows. Worker prompts instruct subagents to read
`.agents/WORKER.md` first; it carries the same protocol in worker-facing form.

The session pick policy is `before-each`: the persistent agent re-evaluates the
backlog before every delegation. For each candidate child:

1. Compute the pickable set: tasks whose plan dependencies are completed in the
   ledger and that no session currently holds (`cc.ready_tasks(root,
   cc.plan_stories(root))` — see `tools/completion_claims.py` `__main__` for a
   ready-made status summary).
2. **Claim atomically, before any file is created or edited:**
   `cc.claim(root, 'TASK-ID', '<session-id>', 'worklog/<TASK-ID>.md')`.
   `claim` writes through immediately, fails closed on an existing
   `in-progress`/`blocked` claim, and only permits legal transitions
   (`not-started -> in-progress -> completed`, plus `-> blocked` from either,
   with `blocked` recoverable). Two workers racing for one task cannot both
   win; the loser MUST pick a different task.
3. Create the worker's scratchpad `worklog/<TASK-ID>.md` only after the claim
   succeeded, and pass its path to the subagent.

A subagent MUST NOT touch any task file until its claim succeeded, and MUST NOT
edit any file outside its one owned file plus its scratchpad. On completion the
worker sets `cc.update(root, 'TASK-ID', '<session-id>', 'completed')` — which is
only legitimate when its frozen tests are green with zero test edits — or
`update(..., 'blocked', note)` with the exact blocker in the bounded note.
Never set `completed` to dodge verification; the independent verifier still
re-runs the frozen tests, and a false `completed` is a failed lane. In its
completion message the worker reports its scratchpad path; the orchestrator
collects scratchpad paths with `cc.scratchpad_report(document, session)` and
consults them before re-delegating or integrating that lane. After a worker
stops, the orchestrator `cc.release(...)`s the claim when integrating or
abandoning; recovery of a foreign task requires proof the prior owner stopped
plus re-claim under a fresh session id (orchestrator-only
`cc.reclaim(root, tid, session, evidence)` records the proof).

## Landing work: commit and push per feature

A completed feature must be LANDED, not just left green in a worktree:

- Small lanes commit their lane files (owned file, scratchpad,
  `tasks/completion/claims.json`, authored RED tests) and push to `main`
  immediately; if `main` advanced, rebase, re-run frozen tests on the
  integrated tree, then push. Never force-push.
- Larger or race-prone lanes work in a dedicated git worktree on a named
  branch `lane/<TASK-ID>`. The worker pushes the BRANCH to the remote first so
  it exists as a permanent reference, then merges into `main` (rebase onto
  fresh `main`, merge, re-run frozen tests on the merged tree, push).
- After the merge is confirmed on the remote, the WORKTREE is deleted
  (`git worktree remove`), but the remote branch is NEVER deleted — it remains
  as the lane's history for future reference. Branch deletion on the remote is
  an orchestrator/human decision only, never part of lane completion.
- The orchestrator verifies each landing by hash (commit/merge present on
  `origin/main`, frozen tests re-run on the integrated revision) before
  treating the lane as integrated. The independent verifier repeats the
  frozen tests on the exact integrated revision per the ownership pipeline.

Worker-facing instructions for all of the above live in `.agents/WORKER.md`;
every delegated subagent reads it first.

## Ownership and trusted implementation pipeline

Parent tasks are vertical outcomes and can span UI, API, storage and packaging.
Their `paths` are conservative parent locks, NOT permission for a worker to edit
multiple files. Reconcile proposed target modules with actual architecture, then
prewire shared contracts and create durable one-file child tasks under AGENTS.md.
Do not create a second unwired application. The single integrator owns shared
routes/modules, schemas/migrations, Cargo manifests/lockfiles and plan changes.

Before implementation, an independent test author establishes executable
behavioral RED and freezes test-source/command hashes. Missing imports, compile
failure or zero discovered tests are not behavioral RED. Give implementers an
OS-enforced one-file grant and no write authority over tests, verifier/controller
state, source pins, policy, budget or other lanes. Prompts and worktrees alone
are not a sandbox. Audit/report tasks also need executable negative fixtures.

The independent verifier runs real tests, checks actual counts and asserts absent
side effects on denial plus reclaimed resources on cancellation. Verify the
candidate, integrate against CURRENT mainline, then rerun frozen tests and affected
regressions on the exact integrated revision. Failed merge or post-merge test
means NOT accepted and unlocks no dependencies. A clean worktree with precommitted
unintegrated code is not a no-op success. Never blindly retry ambiguous integration
or external side effects. Preserve candidates for safe reconciliation.

## Leases, resources and stop behavior

Implement durable fencing/heartbeats through execution, verification AND integration.
Only the trusted controller publishes acceptance/receipts atomically. Recover work
only after proving the prior owner stopped. Preserve diagnostics and cancellation
ownership; no detached tasks or orphan process trees.

Respect the existing provider budget. A model/worker name containing 'free' is not
proof of free authorized access. Keep the 8 GiB host envelope and 2 GiB reserve,
CARGO_BUILD_JOBS=2, RUST_TEST_THREADS=2, and one heavy Cargo/browser/full-suite
validation at a time. Measure whole process trees; twenty workers are not twenty
parallel builds. Stop admitting work under memory pressure or missing authority.

Retry only classified safely repairable failures up to three attempts. Record
credential, consent, signing, security, device, network and budget blockers without
inventing access. Continue unrelated ready work. No ready work with unresolved
scope is an incomplete/blocked project, not completion. Do not weaken tests or
security controls to maintain throughput.

## Required product and final evidence

Deliver installed `opencode2` -> authenticated daemon start/reuse -> real native
OpenTUI/Rust UI -> in-app provider setup -> real coding turn/tools/approvals ->
persist/restart/resume. No manual serve, browser, database/session initialization
or developer runtime on an end-user machine. All clients use the same Rust/Tokio
engine. Keep optional compatibility visible and measured.

Follow `docs/architecture/COMPLETION_NATIVE_TUI.md` and
`docs/architecture/COMPLETION_REMOTE.md`. Remote mode is opt-in: owned account and
device gateway behind a named Cloudflare Tunnel, outbound PC connector, explicit
pairing/revocation and real installed iOS/Android UI. Local use remains offline and
account-free. Do not expose an unauthenticated agent API or call a PWA a native
mobile acceptance result. Credentials and app signing require actual authority.

Independent final verification must cover EVERY legacy/additive/discovered task
and installed/platform/source/security/resource/hosted/device journey. Store proof
outside worker-write authority. `python3 tools/completion_plan.py --release
--evidence-root /trusted/release-proof` checks receipt completeness/integrity for
HEAD; it does not replace test execution, OS separation or a trustworthy verifier.
It is intentionally expected to fail until real evidence exists.

Report exact integrated commit, verified vs blocked work, actual commands/counts,
source and installed/device evidence, unintegrated branches and operator
prerequisites. Never declare all features/E2E complete while any mandatory scope,
independent proof or usable app wiring is missing.
