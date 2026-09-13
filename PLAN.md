# Implementation plan: Lean Harness

## 1. Product contract

Build a local-first coding-agent application that preserves the audited,
externally observable OpenCode V2 product behavior, closes explicitly selected V2
compatibility gaps, and adds native multi-account routing and stronger resource
and safety controls. Use a Rust core with Tokio for asynchronous coordination and
bounded CPU workers for expensive computation. End users install an application,
not PostgreSQL, MongoDB, Redis or a development toolchain.

The optimization objective is low **whole-application** memory, CPU, disk growth,
startup cost and idle wakeups under realistic agent workloads. It is not a claim
that Rust automatically beats every Go or JavaScript implementation. Benchmarks
must include child processes and optional plugin runtimes.

### Fixed requirements

The full requested scope includes sessions, sharing, forking, resume, custom and
built-in agents, foreground/background delegation, cross-provider child models,
independent effort settings, persistent child history, singleton multi-client
operation, skills, plugins, commands, themes, status and timestamps. Routing is a
native subsystem, inspired by the pinned 9router behavior, not a mandatory
Next.js service. Model metadata originates from models.dev and is served through
our own versioned query API. Safety remains deterministic and capability-based
when ordinary permissions are set to `*`.

`requirements/user-requirements.json` is the explicit requirement checklist.
`FEATURES.md` and `ralph.json` are the implementation backlog. Every current story
is mandatory to the declared full release even when its feature is off by default
at runtime. A lean release milestone is not an excuse to quietly delete remaining
work.

## 2. What is frozen, and what is not known yet

OpenCode is pinned to `95daf90670b7c039c436c85537da5fbfe2205b41`; 9router is pinned
to `17c4cc76877bd1755030a8414f8d0083f48dcccf`. No worker may silently use a newer
`dev` or `master`. `sources/upstream.lock.json` is authoritative.

Inspected evidence includes the V2 session and catalog specs, actual V2 runner,
core tool architecture, model ingestion, plugin API, TUI extraction boundaries,
and 9router's routing entrypoint and development architecture. These are not a
review of every file. In particular, V2 sources mark some plugin, context,
structured-output and recovery behaviors as partial, deferred or unfinished.

Classify every discovered behavior as:

- implemented in native V2;
- implemented only in a shared/legacy compatibility path;
- partial or planned upstream;
- new user requirement;
- deliberate safer/resource-bounded deviation, with a recorded contract.

A design document, unchecked TODO or issue is not evidence that a feature works.
The seed contains 193 work items, but the final number may grow when the full
source audit discovers additional surfaces. No completeness certificate may be
issued while that discovery gate is open.

## 3. Architecture decisions

### ADR-001: one native domain runtime

Use one Tokio runtime per daemon. Model APIs, sessions, subagents, routing,
permissions and persistence orchestration stay in Rust. Use a fixed bounded CPU
pool only where profiling shows CPU-heavy work. Do not create a process per
subagent or a language boundary per feature. Blocking SQLite and native tooling
run outside async executor workers.

### ADR-002: one daemon, many clients, isolated locations

The singleton is per operating-system user and data directory, not a
machine-global privileged service. CLI, native TUI, optional web and optional
desktop clients attach through authenticated versioned transport. Locations own
workspace services; sessions own history and execution; parents own child tasks.
Use lazy location activation and an LRU cap on open stores/watchers/LSP services.
Remote multi-user mode is a separate opt-in security and resource profile.

### ADR-003: behavior parity, not file translation

Inspect every file but implement native behavior slices. Do not preserve an
Effect abstraction solely because it existed in TypeScript. Preserve error,
ordering, cancellation, cleanup, persistence and protocol semantics. One feature
may span many source files and many target modules. Every slice must include a
usable entrypoint, policy checks, state transitions, presentation where
applicable, and independent tests.

### ADR-004: embedded relational state plus bounded blob storage

Keep SQLite for compact canonical state. Use a small global catalog and lazily
opened workspace databases. Store large payloads as content-addressed, optionally
compressed blobs with a crash-safe write protocol. Separate live deltas, durable
history, replay windows, audit records and disposable caches. Give every growing
category a retention/archival policy or an explicit quota behavior. Never silently
delete precious user history just to satisfy a byte target.

### ADR-005: optional compatibility is visible and measured

Native plugin manifests, tools and declarative UI contributions use a stable
capability/RPC boundary. Arbitrary JS/TS plugins may need a restricted optional
JavaScript host. Arbitrary Solid/OpenTUI presentation plugins cannot simply run
inside a Rust renderer; an opt-in compatibility frontend is a separately tested
feature. Native mode starts neither host. A plugin must declare its supported
contract version and granted capabilities.

### ADR-006: authorization is outside the model

All agent-originated operations use a trusted policy broker. Generated executables
run in an OS-restricted environment. Prompts and pre/post hooks improve behavior
but cannot grant access, disable isolation or undo completed damage. Explicit
human grants have resource, operation, identity, expiration and policy-version
bounds. Project config is not a source of human authority.

### ADR-007: tests and acceptance have a separate owner

The implementer cannot mark its own story accepted. A trusted controller owns
state and invokes an independent verifier against frozen tests and exact trees.
Test source, verifier configuration, resource limits, scope and reference pins
are not writable by the implementation sandbox. The system assumes the trusted
controller and verifier are trustworthy; it does not claim to make a malicious
verifier safe through JSON validation.

## 4. Delivery sequence and useful milestones

Dependencies in `ralph.json` decide eligibility. The phases below are delivery
milestones, not instructions to run every task in a group serially.

| Milestone | Main tasks | Demonstrable result |
|---|---|---|
| M0 - reference and verification | DISC-001..010, AUTO-001..002 | Pinned sources, exhaustive inventory, deterministic fixtures, trusted RED/GREEN pipeline |
| M1 - safe local foundation | BASE, SEC core, DB foundations | Two clients attach to one daemon; create and reopen a session; unsafe file/process operations are denied |
| M2 - one complete coding turn | CAT, PROV core, TOOL core, SESS-001..010 | Select a real or fixture model, stream, authorize tool, settle and continue; interrupt/restart works |
| M3 - delegation and routing | AGENT, ROUTE, relevant UI | Mixed-provider parent/children, foreground/background lifecycle, account limits and small-machine tests |
| M4 - complete user workflows | remaining SESS, SHARE, EXT, INT, UI | Fork, rewind, skills, plugins, commands, LSP/MCP/ACP, storage settings and full history workflows |
| M5 - optional compatibility | WEB, UI-012, plugin fixtures | Optional web/desktop/legacy TUI contracts pass with cost attribution |
| M6 - hardening and completion | OPS, AUTO hardening, REL | Full scope reconciliation, crash/security/resource suites and reproducible artifacts |

A first native vertical thread should be: create session -> list it -> admit
prompt -> fixture provider response -> timestamped transcript -> close/reopen.
A second thread should be: fixture provider requests a file read -> secret denied
or safe content returned -> outcome persisted -> UI renders it. These prove the
cross-layer contracts before hundreds of parallel implementations accumulate.

## 5. Slice template and independence rules

Every task card contains the outcome, source selectors, dependencies, suggested
module/test boundary, public entrypoints, negative cases and resource requirement.
The module path is a proposed owner, not a mandate for one-file architecture.

A slice is ready only when its dependency revisions and required contracts are
accepted. Siblings can work independently in isolated worktrees if they do not
hold the same ownership lock. Public schema, migration numbering, manifests,
Cargo lockfiles and central route registries use a serialized integration lane.
Do not ask every agent to edit the same `lib.rs`, `Cargo.toml` or database schema.
Have workers propose additive registration fragments; the integrator assembles
them and reruns all affected tests.

Large stories discovered to require unrelated contracts must be split into
smaller complete user-visible outcomes. New children inherit the original
requirements, evidence and non-negotiable tests. Parent acceptance requires all
children, so splitting cannot hide unfinished work.

## 6. Mandatory test lifecycle

For each slice: inspect sources and runtime evidence -> define behavior contract
and intentional deviations -> author tests -> establish a compiling **RED** that
fails for the missing behavior -> freeze tests and command manifest -> implement
minimum behavior -> run **GREEN** -> refactor -> run independent regressions,
negative/security tests, lifecycle/resource tests and differential fixtures ->
merge through the integrator -> rerun on the exact integrated revision.

The five named tests per task are a minimum taxonomy, not five sufficient tests
for an arbitrary feature. Expand them into concrete cases. A denied permission
must assert absence of side effects, not merely an error string. Cancellation
must assert that tasks/processes/FDs are actually reclaimed. Partial streams,
provider errors and process crashes need fixtures.

Compile failures, missing imports, tests that never execute, disabled tests,
`#[ignore]`, a fabricated successful log, and a worker-edited `passes:true` are not
valid evidence. A purely declarative or discovery task still needs executable
validators and a captured failing fixture; do not invent product tests for it.

## 7. Source exhaustiveness procedure

`tools/freeze_sources.py` fetches exact references. `tools/inventory.py` enumerates
tracked blobs, symlinks and gitlinks; it never silently skips generated code,
assets, tests, docs, packaging or infrastructure. Candidate glob matches are only
hints. Review every file into behavior ownership or a justified non-product
category. Extract public routes, config keys, CLI actions, hook names, schemas,
tool registrations, event types, UI commands, platform branches and hidden flags.

Then examine call paths and dynamic registration, compare upstream tests, and run
representative reference scenarios. A file-level inventory alone is insufficient:
one file can contain multiple behaviors and a behavior can cross many files.
Maintain a second surface ledger and map surfaces to concrete test IDs.

Hosted billing/marketing/infrastructure in the monorepo still needs an explicit
scope disposition. Do not pretend a local binary owns upstream cloud
infrastructure or a vendor account. Any product-facing behavior intentionally not
reproduced must be visible in the completion report. The implementer cannot
silently approve an exclusion.

## 8. Autonomous operation and stop semantics

The prototype controller is serial and model-agnostic. Trusted adapters connect
it to the chosen coding CLI/API, VCS integration and real sandbox. It repeatedly
selects a dependency-ready task and executes the TDD pipeline. Fresh candidate
workspaces prevent an implementation attempt from changing another slice's
working state. State, receipts and budgets live outside the worker sandbox.

Production AUTO tasks add worktree leases, parallel ready-queue execution,
heartbeats, append-only acceptance records and controlled discovery expansion.
Keep the small-machine default at one or two implementation workers; increase
only within explicit CPU/RAM/provider budgets.

Unattended does not mean unbounded. Retry only classified repairable failures.
After the retry cap, record exact blockers and continue independent tasks. Stop
when no safe ready work remains, total budget is exhausted, required credentials
are unavailable, a platform isolation test fails, or mandatory human authority is
required. A blocked state is not a completed project. The initial budget values
are examples requiring an explicit operator decision.

Application HITL tests use fake trusted grants in isolated fixtures. The builder
must not ask the live user to approve hundreds of test deletions or expose their
real `.env` files. Actual OAuth consent, signing identities, production accounts
and grants cannot be fabricated by subagents.

## 9. Resource acceptance

`config/resource-targets.json` contains proposed acceptance targets, not measured
performance. Measure on a documented two-core, 4 GiB machine with local SSD, then
record baselines and justified adjustments before claiming results. Include idle,
active streaming, many parked subagents, large histories, huge outputs, slow
clients, optional JS plugins and shared LSP processes. Measure the whole process
tree; do not report only the native daemon while excluding a large sidecar.

A bounded queue count is insufficient when messages are huge: bound bytes too.
Record memory reservations before admitting work. A parent awaiting children must
not retain the only permit needed by its children. WALs, caches, diagnostics,
archives and blob garbage collection also have explicit policies and telemetry.

## 10. Release completion certificate

The trusted release verifier must produce a report containing exact upstream
pins, exact product revision, every mandatory task state, source inventory/tree
hashes, reviewed surface counts, intentional deviations, test/log hashes,
platform capabilities, resource measurements, license/SBOM information and
unresolved blockers. Completion requires all mandatory tasks accepted, all
source/surface audits resolved and every applicable release gate passed.

`REL-005` does not override those rules. A milestone demo, passing unit suite,
clean compiler or large percentage coverage is not a full completion certificate.
Do not weaken acceptance to force the loop to end.

## 11. Updating after completion

Freeze a new upstream commit intentionally, diff exact trees, invalidate affected
file/surface reviews, regenerate candidate ownership, add new stories, rerun
changed characterization fixtures and all affected release gates. Preserve old
source locks and evidence for reproducibility. No periodic `git pull` in the
reference oracle during a migration.

## 12. Current kit verification and limitations

The local Python bootstrap utilities have recorded tests and structural
validation under `validation/`. The Rust application, real OS sandbox, actual
provider integrations, full source inventory, user database importer and product
benchmarks have not been built or executed here. The kit is deliberately honest
about these boundaries so later agents cannot inherit a false declaration of
completion.
