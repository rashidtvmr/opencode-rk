# Main-agent handoff: deliver the actual opencode2 application

Read AGENTS.md, PLAN.md, docs/SECURITY.md, docs/TDD.md and
`docs/audits/2026-09-17-app-completion.md` before implementation. This addendum
preserves every original requirement and all existing safety/verification gates.
Do not substitute another plan or disconnected helpers for a usable application.

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
