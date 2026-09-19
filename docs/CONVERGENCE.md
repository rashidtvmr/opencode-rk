# Convergence mode: build the product, not more islands

This repository now has substantial implementation code, but isolated GREEN lanes are
not the same thing as a working application. Convergence mode exists to force the
project toward one executable end-to-end product before breadth work can be called
complete.

## Hard product boundary

The local application is not "built" until one clean installed `opencode2` journey
passes on the exact integrated revision:

1. run `opencode2` with no subcommand from a fresh disposable HOME;
2. detect an interactive terminal, discover or start exactly one authenticated daemon,
   and attach without manual `serve`, browser, database, or session setup;
3. if provider credentials are absent, open in-app setup;
4. render the real native OpenTUI-backed interface rather than the line fallback;
5. submit a prompt through the same daemon-owned execution engine used by web/headless;
6. make a real provider fixture request, stream output, request a tool, authorize it
   through the real security broker, execute it, and feed its result back to the model;
7. persist the session/tool result, exit, restart, and resume the same history;
8. attach a second client and observe the same durable session without duplicate work;
9. exercise denial, interruption, daemon restart, and terminal restoration;
10. rerun the frozen end-to-end test on the exact integrated commit.

A parent task cannot be complete while its own note says repair child, follow-up,
unwired, unproven, state-only, missing, partial, or no acceptance. Such language is
evidence that the parent remains open.

## Convergence-before-breadth rule

Until the hard local boundary above is GREEN, do not spend the majority of workers on
new leaf/state-machine modules. Use at least four concurrent lanes on the integration
spine when the harness can safely provide them:

- entrypoint/daemon/auth client wiring;
- shared runtime + provider + agent + security-broker wiring;
- native OpenTUI renderer/input integration;
- installed end-to-end test and independent verifier.

The remaining safe capacity can continue independent feature work, but every wave must
land at least one integration/wiring improvement. A wave that only adds leaf modules is
not progress toward product completion.

After the local boundary is GREEN, expand in this order:

1. TUI feature parity on the shared engine;
2. web parity, including graph/canvas, tools, approvals, sessions, artifacts, files,
   terminal, providers, agents, settings, research/voice where required;
3. remote gateway/pairing/control;
4. native iOS and Android parity;
5. full four-client parity and release evidence.

## Current integration blockers to verify first

The gate intentionally checks structural facts that have repeatedly caused false
completion:

- no-subcommand must actually call the launch/daemon decision path;
- the serve path must enforce bearer auth, not merely mint a token;
- the native renderer bridge must have a real caller;
- server must depend on and call the real agent and security crates;
- the live turn path must not directly bypass authorization with bare ToolExecutor;
- completed claims must belong to the plan and must not admit missing follow-up work.

These are necessary wiring checks, not sufficient release evidence. Passing the static
gate never substitutes for behavioral RED/GREEN, installed E2E, real OS isolation,
provider canaries, remote deployment, or device tests.

## Orchestrator-neutral parallelism

Any main orchestrator may be used if it can provide these capabilities: native worker
spawn, completion notifications, cancellation/join, isolated ownership, independent
verification, and serialized integration. Target 20 workers, keep at least 15 useful
workers when the harness/provider/resource budget supports it, and refill on individual
completion rather than batch barriers.

Reserve capacity by role rather than letting all twenty workers create leaves:

- 4 integration-spine implementation lanes;
- 2 independent test/verifier lanes;
- up to 14 independent feature/audit lanes.

If fewer than 20 workers are available, preserve the integration/test lanes first.
Twenty parallel planners with zero integration writer is a failure mode.

## Immutable-test rule

Existing and frozen tests are read-only for implementers and orchestrators. They may not
be edited, deleted, renamed, moved, skipped, ignored, snapshot-regenerated, assertion-
weakened, selector-narrowed, or replaced to achieve GREEN. A genuinely wrong frozen test
is a blocked contract review requiring independent authorization; it is never fixed by
the implementation worker.

New tests may be authored only by the independent test-author role before freeze. After
freeze, only product code changes until GREEN. Candidate GREEN, integration, and
post-integration GREEN must all use the same frozen test hash.

## Acceptance

`tasks/completion/claims.json` is coordination state, not acceptance authority.
"completed" in that ledger cannot unlock a release claim by itself. Trusted acceptance
requires independent verification, successful integration, and a rerun on the exact
integrated revision. Off-plan lane IDs and self-reported logs never count as parent
completion.

Run `python3 tools/convergence_gate.py` before choosing new breadth work and after every
integration wave. While it fails, fix its product-spine findings before declaring parent
tasks complete.
