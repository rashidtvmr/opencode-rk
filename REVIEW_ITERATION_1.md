# REVIEW_ITERATION_1 — Brutal Prod-Ready Audit
Date: 2026-09-19 | Repo: /home/rashid/projects/opencode-rk | Style: terse, factual, file:line cited

## 0. Verdict
App runs. Not prod. Not opencode-like. Loop never closes by design.
- One binary: `crates/cli/Cargo.toml:9-18`, sole `main()` `crates/cli/src/main.rs:191-202`
- `cargo run` works. Smoke passes: `--help`, `doctor`, `session create`, `serve`+curl, `tui --once`
- Core real: SQLite store `crates/sessions/src/store.rs:27-125`, `agent_loop.rs:98-175`, `turn_service.rs:233-479`, policy engine `crates/security/`, `rules_loader.rs:129-196`
- Rest: orphan logic, unwired lanes, stub `pub mod`, auth theater, ledger fiction
- Trust `RAW_FEATURE.md` + disk. Do NOT trust `FEATURES.md:9-124`, `prd.json`, `tasks/completion/claims.json`

## 1. Entry point — exists, miswired
| Path | Evidence |
|---|---|
| CLI parse/dispatch | `crates/cli/src/main.rs:45-76,203-272` |
| serve() | `main.rs:590-648` |
| web() | `main.rs:570-588` |
| chat TUI | `chat.rs:45-73 run()` |
| line TUI | `tui_entry.rs:464-504 run()` |
| CI headless | `ci_run.rs:246-368 run_ci()` |
| HTTP routes | `crates/server/src/lib.rs:109-175 router()/router_with_auth()` |
| auth middleware exists | `crates/server/src/daemon_auth.rs:151-176 require_bearer()` |
| daemon lock+descriptor | `crates/server/src/daemon.rs:299-344,257-280` |
| web bundle served | `crates/server/src/web_assets.rs:12-37` |

Gaps:
1. Auth theater. `serve()` mints token `main.rs:632-633`, serves unauthenticated `router()` `main.rs:643`. Live: no-auth curl => `200 /health`, `200 /api/sessions`, `201 create`. `router_with_auth` only in tests `crates/server/tests/daemon_auth_api.rs:22`. Fix: `router_with_auth(state, Some(credential))`.
2. Fix breaks clients. `chat.rs:434-503`, `tui_entry.rs:119-169`, `ci_run.rs:50-109` hand-roll TCP HTTP, never send `Authorization`. Bearer formatter `daemon_client.rs:639-648,722-735` unwired.
3. No-subcommand bypasses own launch plan. Never calls `app_start.rs:295 plan_default_launch`, `daemon_client.rs:694,722 discover/decide_lifecycle_authed`, `onboarding.rs:424-586`. No TTY detect, no owner election, no Setup view. 345 dead-code warnings.
4. `--native` fake. Dispatch `tui_entry.rs:365-415` line echo. Real `Renderer` `crates/opentui-bridge/src/lib.rs:8-12` unwired. `libopentui.so` absent. `native/` only python helper. `RAW_FEATURE.md:46` claim false.
5. `chat::run(_data_dir)` ignores data dir `chat.rs:45`. Spawned `serve` uses default dir.
6. Single provider. `lib.rs:761,890` rejects non-`openai`. Anthropic modules `providers/src/lib.rs:12-13,41` never called.
7. Tools deny-by-default `lib.rs:838-848`, agentic loop dead out of box.
8. Dead subcommands never dispatched: `run_headless.rs:167`, `native_app.rs:361`, `service_commands.rs:137`, `pair.rs:180`, `install_commands.rs:56`.

Minimal fix (~30 lines, `main.rs` + 3 client files):
- `serve()`: use `router_with_auth`
- `chat.rs`/`tui_entry.rs`/`ci_run.rs`: read `backend.json` via `daemon::read_backend_descriptor`, add `Authorization: Bearer` header
- no-subcommand arm: call `plan_default_launch` + `discover`; redirected stdio => headless error exit 2
- vendor `libopentui.so` or delete `--native` claim

## 2. Features — claimed vs disk
Real: registry `crates/tools/src/registry.rs:75-130`, agent_loop wiring `lib.rs:828,968,1050-1136`, turn_service, sync_log memory `sync_log.rs:78-192`, rules_loader, share ledger `sessions/src/share.rs:1-89`, `hook_bus_v2.rs:260L`, OAuth parse `responses.rs:736L`, `streaming.rs:161L`.

Stubs compiled as done (CRITICAL):
- `crates/sessions/src/snapshot.rs:1`, `filter.rs:1`, `cache.rs:1`, `meta.rs:1`, `import.rs:1`, `gc.rs:1`, `events.rs:1`, `export.rs:1` — 1-line each
- `crates/tools/src/permission.rs:1`, `builder.rs:1`, `search.rs:1`, `request_perms.rs:1`, `elicitation.rs:1` — zero items, exported `pub mod`
- `crates/providers/src/fallback.rs:1` — fallback routing missing
- `oauth_flow.rs:1-65` URL validator only, no HTTP/token/refresh
- `sandbox_real.rs:1-96` detection only, header admits no confinement. Labeled enforcement.
- `mcp_spawn.rs:1-50` spawn real, no JSON-RPC handshake. `mcp.rs:238-252` fake caps flagged in-module.
- `lsp_client.rs:1-40` no initialize, not consumed by turn path

Orphaned real logic, zero consumers (HIGH):
- `assemble_prompt` `rules_inject.rs:82` — 0 callers
- `LoopDriver` `loop_driver.rs:1-820` — mod decl only
- ROUTE table `route_table.rs:259L`, `model_route.rs:137L` — never consulted
- `context_report.rs` cited, ABSENT from disk
- `ci_run.rs`/`ci_output.rs`/`main.rs` modified unstaged. `opentui-bridge/` untracked. Cited evidence not on commit.
- Web `web/src/` real React, own caveats `FEATURES.md:87-95`: write-paths disabled, browser unexecuted, WEB-013/015 absent. Accepted anyway.
- Zero `todo!()/unimplemented!()` in `crates/`. Stubs use 1-line comments, evade lint.

Pattern: pure-state module tested isolation, marked accepted, never wired into turn/provider/TUI path.
`FEATURES.md` 258/258 accepted contradicted by 180 `TBD` rows, `prd.json:223,236` `not-started`, guard FAIL 122+134.

## 3. Ledger — FAIL, untrustworthy
- `tasks/completion/claims.json`: 121 rows = 96 completed / 24 in-progress / 1 blocked
- Plan union 109 stories: 61 completed / 6 in-progress / 1 blocked / 41 unclaimed not-started
- `ralph.json` 258 accepted, legacy flags non-evidence per `ralph.completion.json:28`
- No verifier receipts. `tests/release/` absent, `tests/e2e/` absent.

False completions:
1. 53 off-plan rows (LANE-* 35, RC-01/02/03, AUD-004-ENTRY, etc). Violates `tools/completion_claims.py:210-222`.
2. Isolation-GREEN as completed. Notes admit lib.rs untouched, needs integrator prewire (DISC-108/109/111-114, WEB-PINS-SEARCH, etc). Requires integrated-tree GREEN.
3. Illegal transitions. AUD-001 + 6 LANEs carry both blocked+completed. No blocked->completed in TRANSITIONS. Empty blockedNote violates non-empty rule `completion_claims.py:153-154`.
4. Post-freeze test edits. LANE-WEB-CANVAS: test fixes noted. WEB-TOOL-CHOOSER: afterEach cleanup post-freeze. Counted complete.
5. Audit-as-done. AUD-011/012/013/014/018/020: no acceptance claimed, status completed.
6. 87/121 rows point nonexistent scratchpads. `worklog/AUD-001.md`, `PAR-001.md` absent.

Critical path unstarted (41): APP-012, COORD-001..008, DISC-101,118,119, NET-001..015 entire gateway, MOB-001..006, SHIP-001..008. Trusted pipeline itself unbuilt, every GREEN self-reported.
`tools/lane_gate.py:31-44` only gates 12 storage-v2 lanes, not 121 claimed.

## 4. Integration — under-connected DAG
```
contracts leaf
security -> contracts; storage -> contracts; sessions -> contracts,storage; catalog -> contracts
providers -> contracts,security; tools -> contracts,security [edition 2024 vs workspace 2021]
agents -> contracts ONLY
foundation -> NOTHING, depended by NOTHING (orphan)
opentui-bridge -> NOTHING, depended by NOTHING (orphan)
server -> catalog,contracts,providers,sessions,tools (+storage dev-only)
cli -> catalog,contracts,sessions,server,storage,tools
```
No cycle. Problem opposite: under-connection. `foundation,opentui-bridge,agents` zero reverse deps in Cargo.lock.

Orphans:
- `crates/foundation`: `OwnedTaskScope:112`, `ByteBudget:197`, `LazyService:307`, `HeartbeatGuard:421`. Zero `use` outside crate.
- `crates/opentui-bridge`: `CellBuffer/Rgba/BridgeHandle/Renderer`. Zero importers. No `opentui-sys` crate despite arch doc mandating audited FFI.
- `crates/agents`: 7 mods declared `lib.rs:5-11`, 8 files on disk undeclared (`agent_files.rs`, `command_templates.rs`, `delegation_live.rs`, `ultra_codegen.rs`, etc). Only via `#[path]` test hacks. Nothing in server/cli imports it.
- `server`: `context_accounting.rs`, `rules_inject.rs` no `pub mod` in `lib.rs:1-75`. `turn_service.rs:747L`, `loop_driver.rs:820L`, `app_runtime.rs:1177L`, `runtime_wiring.rs:861L`, `web_turn_adapter.rs:516L`, `remote_approvals.rs:553L` declared never called. `runtime_wiring.rs:1-16` admits serve-path wiring integrator-owned — never happened. Routes use loose `AppState`, engine composition rots.
- `tools/lib.rs:4-72` missing 9 on-disk files. `providers/lib.rs:1-59` missing 9. `security/sandbox_real.rs` not declared.
- Stub `pub mod`s lie about capability (tools 5, sessions 8).

Broker bypass (blocker #1):
- Zero `use opencode_rk_security` in `server/src/`, `cli/src/`.
- Turn path `server/src/lib.rs:1132,1153-1160` calls `ToolExecutor::new().execute()` directly. No broker, no `ToolAuthorizer:95`, no audit.
- `registry_dispatch.rs:67-79` defaults `AllowAll`. Turn path bypasses even that.
- `shell_tool.rs`, `file_ops.rs`: zero authorize refs.
- Three local `trait PermissionBroker` shadows (`ext_scoped_exec_lane.rs:119`, `plugin_scoped_exec.rs:119`, `lsp_client.rs:61`).
- Triple policy: `EnginePolicy app_runtime.rs:130`, `turn_tool_config() lib.rs:838`, `security::Decision`. Triple permits: `TURN_PERMITS lib.rs:103` live vs `TurnPermits app_runtime.rs:549` idle. Triple sessions: format-1 `storage`, format-2 `sessions/SessionManager`, `SessionService:332` bridge. `main.rs:466` vs `:471` fork by subcommand.
- Auth: `main.rs:~630` comment admits legacy unauthenticated `router()`. Token discoverable, never checked. `chat.rs:36-60` raw TcpStream, ignores Unix socket `daemon.rs:131` + bearer.
- Provider gate `lib.rs:761,890` single-provider despite catalog advertising many.
- `web_capabilities lib.rs:179-249` hardcodes false for plugins/approvals/search/voice. `web_assets.rs:10` embeds stub `index.html 558B`, no verified `web/`=>`web_dist/` copy.
- `headless_engine.rs` pure string helpers, zero engine imports.

## 5. Build/test — why never ends
- CI `.github/workflows/ci.yml:29-51`: no `CARGO_BUILD_JOBS` cap, no threads cap, `planning`+`rust` independent, fail-closed false.
- `tools/auto_drive.py:200,229`: `--lanes 15`, 15 parallel workspace builds vs 8GB budget. OOM guaranteed. `ralph_loop.py:65`: 6 lanes, same violation.
- Bottom heavy: 100+ unit tests, 63 files `server/tests/`. Top absent. No product e2e gate in CI.
- Mutually exclusive acceptance: `ralph.json` all-accepted vs `validate_backlog_exhaustion.py:173-198`, `reconcile_surfaces.py:56`, `PLAN.md:250-252` demand not-done + missing inventory. One side always fails.
- Hash fragility: `sources/disc-003-reconciliation.manifest.json` byte-match. Legit edit => drift => FAIL => retry => drift.
- Verifiers exist unwired: `check_tdd_pipeline.py`, `check_release_tdd.py`, `completion_verification.py:122` rejects `passes:true` self-report. Zero CI callers. `validate_repository.py:21-28` checks no cargo test, no lane_gate. `ralph_loop.py:58-61` vs `auto_drive.py:167-169` disagree on mandatory checks.
- Evidence in `/tmp/opencode/*.log`, ephemeral, not in repo.
- `docs/SECURITY.md:33-34` postgres truncate stale, repo uses rusqlite. Host-destructive if followed.
- `PLAN.md:57,249`: no certificate while discovery open + all accepted + 2-core/4GiB measurements + SBOM. None measured. Bar unreachable, `REL-005` forbids lowering.

Prove health (bounded):
```
python3 tools/validate_repository.py
python3 -m unittest discover -s tests/bootstrap -p 'test_*.py'
python3 tools/lane_gate.py --json
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 120 cargo test -p opencode-rk-storage --test integration_v2 -- --test-threads=1
python3 tools/coverage_gate.py
python3 tools/validate_backlog_exhaustion.py
```
Expect exhaustion/DISC drift FAIL, gate UNRUN without `--run`, coverage FAIL.

## 6. Prod-ready guide — in order, no skipping, no parallel heavy jobs
One resource-heavy cmd at a time. `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2`. 8GB budget, 2GB reserve.

Phase 0 — freeze fraud:
1. Freeze `claims.json`. No new `completed` until ledger rebuilt via `tools/completion_claims.py` only. Delete hand edits.
2. Revert false 53 off-plan rows to unclaimed or add to plan with deps/acceptance. Fix illegal blocked+completed, empty notes.
3. Land or drop dirty tree: `ci_run.rs`, `ci_output.rs`, `main.rs`, `opentui-bridge/`, `install_commands.rs`. No evidence on uncommitted code.
4. Stub policy: implement or remove exports for 5 tools + 8 sessions stub `pub mod`s. Empty public API must die.

Phase 1 — runnable + honest (~30 lines):
1. Serve `router_with_auth(state, Some(credential))` in `main.rs:serve`. Update frozen tests.
2. Add `Authorization: Bearer` from `backend.json` via `daemon::read_backend_descriptor` in `chat.rs`, `tui_entry.rs`, `ci_run.rs`.
3. No-subcommand: call `plan_default_launch` + `discover`, headless refusal exit 2, no raw mode.
4. Vendor `libopentui.so` or delete `--native` claim + `RAW_FEATURE.md:46`.
5. Fix `chat::run` data-dir passthrough. One session opener for all subcommands.

Phase 2 — wire or delete:
1. Turn path: call `PermissionBroker::authorize` before `ToolExecutor::execute` `lib.rs:1132-1178`, persist denial, emit event. Delete `AllowAll` or `cfg(test)`-gate. Remove 3 shadow broker traits.
2. Unify policy/permits/sessions: one `EngineHandles` in `serve()`, delete `TURN_PERMITS` static, adapt `EnginePolicy`->`security::Decision`, pick format-1 or format-2.
3. Pre-wire `lib.rs` or delete: agents 7, tools 9, providers 9, server 2, security 1. Add reverse deps for `foundation/agents/opentui-bridge` or delete crates.
4. Wire `assemble_prompt`, `LoopDriver`, ROUTE table, LSP/MCP-to-turn, rules->prompt, graph->paint. Or mark correctly unclaimed.

Phase 3 — ledger rebuild:
1. Re-claim only with real scratchpad `worklog/<TASK-ID>.md`, integrated-tree GREEN, frozen hash, exact rev.
2. Build spine first: `COORD-001..008`, `APP-012`, `DISC-101,118,119`. Then NET-, MOB-, SHIP-.
3. Run `python3 tools/lane_gate.py --run` per lane. Extend gate to all crates.

Phase 4 — verifier + CI:
1. Wire `completion_verification.py`, `check_release_tdd.py`, `check_tdd_pipeline.py` into CI. Add `needs: planning` to rust job.
2. Cap jobs: `CARGO_BUILD_JOBS=2`, lanes 1-2 serial. Fix `auto_drive.py:200` lanes 15, `ralph_loop.py:65` lanes 6.
3. Pin exact toolchain, not `stable`. Fix `SECURITY.md` postgres instruction. Disposable test DB only.
4. Resolve acceptance contradiction: either `ralph.json` not all-accepted or update validators + `PLAN.md:250-252` + `coverage_gate.py:54` to reachable bar with SBOM + 2-core/4GiB measurements.

Phase 5 — release:
1. `SHIP-001..008`: artifacts, clean-machine matrix, canaries, soak, docs, completion decision.
2. Multi-provider turns or honest single-provider label. Tool default that does something or honest dead-loop label.
3. Web write-paths enabled + browser executed, or keep caveats and stop claiming parity.

Skip any phase and loop restarts.
