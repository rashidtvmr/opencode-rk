# Application completion audit - 2026-09-17

## Scope and evidence boundary

Inspected project revision: `165b076c91ac6f2fd71e23f01d9fd7d3054c121d`.
OpenCode V2 baseline: `95daf90670b7c039c436c85537da5fbfe2205b41`.
Observed OpenCode dev head: `5a8335857b0ebec44ef6aa1d52b339cf25c329ca`.
Owned OpenTUI fork: `rashidtvmr/opentui` at
`c01292fd0837bafd07ce458c74416b2b375a41ab`.

This is a source-backed integration audit, NOT a release certificate, exhaustive
review of every upstream file, or evidence that Rust/platform/device tests passed.
All original requirements remain mandatory. Existing accepted flags must be
reconciled with independently verified behavior at the installed entrypoint.

## Confirmed gaps

1. **No default installed-app journey.** `crates/cli/src/main.rs`, `Cli.command`
   and `run`, require a `Command` subcommand. The binary identifies as
   `opencode-rk`. There is no default `opencode2` -> discover/start authenticated
   daemon -> create/select project/session -> launch native TUI contract.
   Upstream `packages/cli/src/commands/handlers/default.ts` obtains
   `daemon.transport()` and invokes `runTui(transport)`.
2. **Line UI is not OpenTUI.** `crates/cli/src/tui_entry.rs`, `interactive_loop`,
   reads newline-delimited stdin and prints text frames. `persist_submit` POSTs
   messages; it does not itself start the provider/tool execution loop. Without
   `--origin`, submissions are explicitly not persisted. `fetch_snapshot` asks
   users to create a session manually when none exists. The follow loop polls.
3. **Existing subsystems are not one engine.** `crates/server/src/lib.rs`,
   `web_capabilities`, explicitly says the web Responses adapter cannot execute
   native tools, run execution-owned approvals, host plugins, or send persisted
   draft attachments to the provider. Workspace metadata is exposed but session
   membership and memory/context authority are not implemented. These are
   integration requirements, not permission to remove the features.
4. **Accepted-before-integrated bug.** `tools/ralph_loop.py`, `record_result`,
   sets `state.status = "accepted"` before `finalize_worktree`. Failed integration
   leaves the task accepted with an error string; dependencies can be unlocked
   even though their code is absent from mainline. No post-integration rerun is
   required there. In `finalize_worktree`, a clean working tree also returns
   success without proving precommitted branch changes are on mainline.
5. **Batch barriers prevent rolling 20-worker execution.** The controller's
   `main` waits for the entire ThreadPoolExecutor batch, then integrates results.
   Slots are not refilled after individual completions. A completed early lane
   can also stop heartbeating while another batch member continues. Raising
   `maxConcurrentLanes` from 6 to 20 alone does not fix this.
6. **Task identity is not file ownership.** `ownership_lock` returns
   `feature:<task-id>`; distinct tasks can still modify overlapping files. Isolated
   worktrees do not prevent integration conflicts or unauthorized shared edits.
7. **Plan reconstruction lost detail.** `ralph.json` contains accepted stories
   with `TBD - see source audit` and empty dependencies. `tools/plan_model.py`
   synthesizes milestone barriers only when NO story has authored dependencies;
   adding a single explicit dependency can therefore disable inferred protection
   for the rest. Its prefix table also omits families already listed in
   FEATURES.md (SYNC, RUN, ACP, WSX, SDK, HEAD). The plan and controller require
   reconciliation, not another bulk acceptance update.
8. **Source presence is not end-to-end proof.** Numerous modules are exported,
   but the actual CLI/router determines reachability. A task must trace its
   entrypoint, service, authorization, durable state, event and UI response.
   Named test obligations are specifications, not tests that necessarily execute.

## OpenTUI decision

Prefer a bounded Rust wrapper around the owned fork's native Zig renderer over
an immediate wholesale Rust port. This is an architecture decision to validate
with a native build/ABI/PTY spike, not a proven performance claim.

At the pinned fork, native code is under `packages/native`, NOT
`packages/core/src/zig`. `packages/native/src/lib.zig` exports native handle-based
functions and uses `extern struct` layouts. `packages/native/build.zig` defines
the build. Preserve the required renderer, text/grapheme and layout behavior;
audit the actual dependency graph before disabling audio, image, embedded-terminal
or other optional capabilities. Do not assume removing imports is sufficient.

`packages/core/src/renderer.ts` also owns input parsing, render scheduling,
selection, key handling, palette/capability handling and higher-level renderables.
Removing TypeScript means implementing the necessary orchestration in Rust; it
is not simply binding a handful of draw calls. Native mode must launch no
Node/Bun/TS runtime. Arbitrary Solid/TS UI plugins remain an explicitly separate
compatibility product, not silently claimed native support.

Use an audited narrow FFI crate, opaque handles, explicit ABI version, ownership
and lifetime rules, validated pointer/length arguments, error codes, one renderer
owner thread and bounded events into Tokio. Do not weaken `forbid(unsafe_code)`
across domain crates. Isolate the necessary unsafe boundary. Runtime users install
packaged artifacts, not a Zig/Rust/Bun build toolchain. Keep fork changes and
artifact hashes pinned and license notices intact. A Rust-only replacement needs
an explicit comparison ADR and the same terminal/Unicode/lifecycle tests.

## Required product contract

`opencode2` with no subcommand starts or reuses one authenticated daemon per OS
user/data directory, then opens the real terminal UI. First-run provider setup
happens in the application. No manual serve, session creation, browser launch or
database setup is required. TUI, headless, web and phone use the same daemon-owned
execution, permission, persistence and event services. Add installed-artifact
acceptance scenarios rather than accepting more isolated utility modules.

The hosted mode is opt-in and keeps local use account-free/offline. A self-hosted
account/device gateway is published through a named Cloudflare Tunnel. A paired
PC maintains an authenticated outbound connection to that gateway; phone clients
use the same account and gateway, with per-device/workspace/session authority.
Tunnel transport is NOT authentication or authorization. Cloudflare terminates
transport TLS unless application-layer encryption is separately implemented;
do not claim end-to-end encryption by default. Never publish an unauthenticated
local agent/PTY API. Real iOS and Android clients, secure credentials, explicit
pairing, tabs, replay/reconnect, approvals, revocation, audit and deploy/restore
work must all be separate testable vertical slices.

## Completion and parallel execution rules

- Keep every original task and requirement. Re-audit accepted stories; do not
  silently demote historical records or invent retrospective RED/GREEN receipts.
- Parent slices own complete user outcomes. Under AGENTS.md, each delegated child
  owns one file; the main agent prewires shared contracts and integrates all child
  files. Path/subtree locks, dependency revisions and frozen tests are mandatory.
- Target 20 native subagent workers; refill on EACH completion, not batch exit.
  Actual harness concurrency, safe readiness, memory and provider budgets may
  reduce occupancy. Never bypass provider limits or claim unavailable agents run.
- Keep the 8 GiB development envelope and 2 GiB reserve, one heavy build/test
  command, and serialized integration. Twenty workers do not mean twenty Cargo
  builds or twenty heavyweight CLI processes.
- Accept only independently verified code after successful integration and a
  passing rerun on the exact integrated revision. Failures unlock no dependents.
  Zero discovered tests, edited frozen tests, mock-only success and worker reports
  are not proof. Guard code in a writable worker tree is not trusted enforcement.
- Missing account consent, network, signing identities, devices, sandbox support
  or budget remain explicit blockers. No ready work is not the same as completion.

## Reproduction commands for a trusted checkout

Run the existing canonical guard and bootstrap tests before changes; preserve
baseline failures. Run the packaged CLI with no arguments under a real PTY in a
fresh disposable home. Inspect its server ownership and provider/tool/approval
journey. Do not use or mutate the user's existing OpenCode history database.

The authoring container cannot resolve github.com and has no Cargo executable.
This audit therefore claims source inspection only. Any subsequent Python plan
or scheduling tests must be reported separately from full Rust, native OpenTUI,
installed-product, Cloudflare and mobile-device verification.
