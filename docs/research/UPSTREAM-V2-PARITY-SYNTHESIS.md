# Upstream V2 Parity Synthesis

Source worklogs: `worklog/UPSTREAM-V2-*.md` (6 files, 703 lines total).
Upstream checkout: `/home/rashid/projects/tmpcodes/opencode`.
Local repo: `/home/rashid/projects/opencode-rk` at `3db7402`.
This report synthesizes. It implements nothing. It changes no contract.

---

## 1) Scope and pins

- Observed upstream checkout HEAD: `07619a09d6da7945ea3fdbb4f8ae5ce8dc2b6eeb`
  ("chore: checkpoint 2026-09-13"). Verified via upstream `git log`.
- Plan-pinned upstream revision (`PLAN.md:36`): `95daf90670b7c039c436c85537da5fbfe2205b41`.
- Authoritative pin file: `sources/upstream.lock.json` per `PLAN.md:38`.
- These two revisions are NOT the same. Every worklog states this.
  No claim in this report equates observed HEAD with the pinned release.
- The upstream `v2/` surface at observed HEAD is a schema/service bridge,
  not a complete replacement runtime. `packages/opencode/src/v2/session.ts:9-71`
  defines the V2 namespace and `SessionV2.Service`, but `create` and `prompt`
  are marked not implemented; `fromID` bridges to the existing session service.
  Operational behavior described here is the current Effect-based runtime
  backing the V2 application, not proof of V2-native implementation.
- A pinned-blob reconciliation (`95daf90...` vs `07619a0...`) was NOT done.
  All parity classifications below are relative to observed HEAD only and
  must be re-checked against the pinned revision before any implementation
  lane treats them as requirements.
- Local HEAD at synthesis time: `3db7402` ("feat: close OPS-003 + INT-002 ...
  lanes GREEN 5/5 (server/sessions/foundation)").
- Diagnostic context (pre-existing, not caused by this report): repository
  validation currently FAILS. `python3 tools/validate_repository.py` reports
  `validate_repository: FAIL backlog exhaustion exit=1` with stale-status
  entries including UI-008..UI-018, WEB-001..WEB-005, AUTO-007, PROV-014,
  OPS-010, EXT-013, SESS-019, SESS-020, ROUTE-012, REL-004. This is recorded
  here as diagnostic context only. It blocks no synthesis conclusion and this
  report does not attempt to fix it (out of scope: this lane owns one file only).

---

## 2) Methodology

1. Read all six `worklog/UPSTREAM-V2-*.md` files end to end (703 lines).
2. Verified the upstream checkout exists and its HEAD equals `07619a0...`.
3. Verified the plan pin `95daf90...` appears in `PLAN.md:36`.
4. Spot-checked a small number of high-value upstream claims against the real
   upstream tree (V2 bridge absence of `/cd`, bash `cd` allowlist only, TUI
   component dir absence of catalog/bulk strings, upstream `git log`).
5. Spot-checked a small number of local claims against the real local tree
   (`crates/sessions/src/tui_state.rs`, `crates/providers/src/auth.rs`,
   `crates/providers/src/registry.rs`, `crates/security/src/lib.rs`,
   `crates/server/src/route_table.rs`, `crates/sessions/src/branch_v2.rs`,
   `crates/catalog/src/*`, `docs/research/REQ-042-tui-mcp-management-design.md`).
6. Ran the repository validator once to capture the pre-existing failure as
   diagnostic context. No network. No tests. No secrets touched.
7. Classification vocabulary (seven values, used consistently):
   - `implemented/shared`: behavior exists in both trees in comparable form.
   - `partial`: local scaffold or subset exists; real behavior unverified or
     narrower than upstream.
   - `planned`: recorded as a local requirement/story/task but not built.
   - `missing`: absent locally with no known plan entry.
   - `optional`: upstream surface with no local mandate (desktop shells, IDE
     extensions, GitHub automation, OTLP unless required).
   - `safer deviation`: local contract deliberately stricter than upstream.
   - `unverified`: claim recorded in a worklog but not confirmed against source
     on one or both sides, or blocked on pinned-revision reconciliation.
8. Rule honored throughout: no invented implementation claims. Where local
   behavior was not read line-by-line, the matrix says `partial` or
   `unverified`, never `implemented`.

---

## 3) Complete feature-family matrix

| # | Feature family | Class | Upstream evidence | Local evidence |
|---|---|---|---|---|
| F1 | Effect/bootstrap runtime, directory-keyed instances | partial | `effect/app-runtime.ts:51-100`, `effect/bootstrap-runtime.ts:15-27`, `effect/instance-state.ts:39-84`, `project/instance.ts:26-192` | `crates/foundation`, `crates/server/src/daemon.rs`, `crates/sessions/src/auto_lease.rs` (behavior unverified) |
| F2 | Git project discovery, workspaces | partial | `project/project.ts:18-521`, `project/state.ts:12-70` | `crates/sessions/src/project_context.rs:82,343-345` (`SwitcherEntry`, `project_switcher`); `crates/server/src/repo_ops.rs` (behavior unverified) |
| F3 | Session CRUD, fork/remap, children, archive, title | partial | `session/index.ts:34-852`, `:501-552` (fork clone + parent-ID remap + cut), `:434-458` (recursive remove), `:601-837` (paged/global listing) | `crates/sessions/src/branch_v2.rs:31-512` (`fork_before_user_message`, `fork_from_message`, `fork_provenance`, `synchronize_legacy_shadow`); `archive.rs` (behavior unverified beyond names) |
| F4 | Messages/parts model, tool states | partial | `session/message-v2.ts:27-947`, `:276-342` (pending/running/completed/error) | `crates/server/src/turn_parts.rs`, `transcript_lane.rs` (behavior unverified) |
| F5 | V2 entry schema + persistence | missing | `v2/session-entry.ts:6-186`, `v2/session.ts:9-71` (create/prompt not implemented), `session/session.sql.ts:100-118` (`SessionEntryTable` commented out) | No known local V2 entry table or service bridge |
| F6 | Prompt loop, shell/command modes, subtask, cancellation, BusyError | partial | `session/prompt.ts:66-1834`, `:521-711` (subtasks), `:713-887` (shell), `session/run-state.ts:9-108`, `tool/task.ts:21-176` | `crates/sessions/src/auto_loop.rs`; `crates/server/src/event_stream.rs`; runner ownership unverified |
| F7 | Compaction, overflow, revert/unrevert, summary/diff | partial | `session/compaction.ts:26-59,35-369` (20k prune / 40k protect), `overflow.ts:1-22`, `revert.ts:16-177`, `summary.ts:68-175` | `crates/sessions/src/auto_compact.rs`, `archive.rs` (thresholds/behavior unverified) |
| F8 | SQLite storage, JSON KV, migrations | partial | `storage/db.ts:30-174` (WAL, NORMAL sync, 5s busy timeout, 64MB cache), `storage/storage.ts:60-330` | `crates/storage/src/*` (WAL/migration parity unverified) |
| F9 | Sync types, projectors, replay, idempotence | unverified | `sync/index.ts:11-265`, `session/projectors.ts:65-154` | `crates/server/src/remote_ledger.rs`, `control_decode.rs` (projector parity unverified) |
| F10 | Typed/global bus, SSE heartbeat/disposal | partial | `bus/index.ts:11-193`, `server/instance/event.ts:11-83` (10s heartbeat) | `crates/server/src/event_bus.rs`, `event_stream.rs` (heartbeat/disposal parity unverified) |
| F11 | Provider registry, filters, SDK resolution | partial | `provider/provider.ts:1042-1371`, `:129-153` (bundled list), `:175-821` (custom), `:1375-1511` (SDK/URL/key/timeout), `:1517-1713` (fuzzy lookup, default/small) | `crates/providers/src/registry.rs:7-113` (`Provider`, `ProviderRegistry`, `default_providers`); `config.rs`, `model_route.rs` (models.dev parity unverified) |
| F12 | models.dev fetch/cache/fallback | partial | `models.ts:111-182` (10s timeout, 5min cache, file lock, hourly refresh, snapshot fallback) | `crates/catalog/src/lib.rs:19-102` (`ModelsDevProvider`, `Catalog::from_models_dev_api_json`); fetch/cache/fallback behavior unverified |
| F13 | Request transforms, provider options | unverified | `transform.ts:192-368`, `:749-1058` (32k default output cap); `session/llm.ts:80-426` | `crates/providers/src/provider_dispatch.rs`, `route_compose.rs` (transform parity unverified) |
| F14 | Auth store (0600), OAuth lifecycle, provider auth routes | partial | `auth/index.ts:7-91`, `auth.ts:96-234`, `server/instance/provider.ts` | `crates/providers/src/auth.rs:8-78` (`AuthMethod`, `ProviderAuth`, `AuthHandler` w/ in-memory OAuth2 state); durable protected store unverified |
| F15 | Codex/Copilot/GitLab/Bedrock/Vertex/Workers provider specifics | missing | `plugin/codex.ts:13-608`, `plugin/github-copilot/copilot.ts:12-379`, `provider/provider.ts:276-721` | No known provider-specific OAuth connectors; generic config only (`config.rs`) |
| F16 | Retry-after parsing, overflow classification, cost accumulation | partial | `retry.ts:12-122`, `error.ts:9-197`, `overflow.ts:1-22`, `processor.ts:361`, `account/index.ts:134-456` | `crates/providers/src/retry.rs`, `rate_limit.rs`, `cost.rs`, `budget.rs`, `usage_status.rs`, `account_status.rs`, `account_sync.rs` (header-parsing/classification parity unverified) |
| F17 | Tool registry + built-ins + conditional tools | partial | `tool/tool.ts:9-141`, `tool/registry.ts:51-346` | `crates/tools` (registry shape unverified; see section 7) |
| F18 | Tool permissions/ask, abort, truncation/spill | partial | `tool/truncate.ts:12-60` (2000 lines/50KB, 7-day spill), bash 2min timeout + 3s kill | `crates/security/src/lib.rs:151-202` (`PermissionBroker::authorize`); truncation/spill parity unverified |
| F19 | MCP transports, status, tools/prompts/resources, OAuth, routes | partial | `mcp/index.ts:1-892`, `mcp/auth.ts`, `oauth-provider.ts`, `oauth-callback.ts`, `server/instance/mcp.ts:1-245`, `cli/cmd/mcp.ts` | `crates/providers/src/mcp_transport.rs`, `int_mcp_lane.rs`, `integration.rs` (OAuth RFC 7591 / status-discrimination parity unverified) |
| F20 | MCP catalog/search/install | missing (upstream too) | Absent upstream (worklog section 6 "Important upstream absence") | Local REQ-042 design only: `docs/research/REQ-042-tui-mcp-management-design.md:170-220`; `crates/catalog/src/search.rs:8-47` (`SearchIndex`) is model-catalog search, not MCP catalog |
| F21 | MCP bulk selection, checkbox/power split, lifecycle persistence | missing (upstream too) | Absent upstream | Local design: REQ-042 doc `:243-293`; TOOL-016/017/018 lanes (build state unverified here) |
| F22 | Invocation-time disabled-tool recheck | missing (upstream too) | Absent upstream; disabled excluded at connection/state layers only | Local REQ-042 contract (filter before payload + again before invocation) is newer/stronger; implementation unverified here |
| F23 | Plugins, hooks, npm/file install, metadata | unverified | `plugin/index.ts`, `loader.ts`, `install.ts`, `meta.ts` | `crates/security/src/hook_bus_v2.rs`, `hooks.rs` (npm/file loading parity unverified; must not equate plugin text with sandbox) |
| F24 | Skills discovery/cache/URL/permission filter | unverified | `skill/index.ts:1-264` | No confirmed local skills discovery file in this synthesis (unverified) |
| F25 | Commands + template args + events | partial | `command/index.ts:1-191` | `crates/security/src/cmd_patterns.rs` (template/event parity unverified) |
| F26 | Permission broker (last-match, defer, always/reject/cascade) | partial | `permission/index.ts:19-325`, `permission/arity.ts` | `crates/security/src/lib.rs:76-202` (`SecurityPolicy`, `PermissionRule`, `PermissionSet`, `PermissionBroker`); defer/cascade/last-match parity unverified |
| F27 | Questions (deferred, ordered, reject on dismiss) | unverified | `question/index.ts:10-108` | `crates/server/src/web_tool_chooser.rs` is a chooser, not confirmed parity (unverified) |
| F28 | LSP (9 ops), formatters | unverified | LSP/formatter modules per core worklog | No confirmed local LSP client file in this synthesis (unverified) |
| F29 | Snapshots (Git track/patch/restore/diff, 7d/2MB) | unverified | `snapshot/index.ts:35-58` | No confirmed local snapshot file in this synthesis (unverified) |
| F30 | PTY | unverified | `pty/index.ts:15-362` | No confirmed local PTY file in this synthesis (unverified) |
| F31 | Worktrees | partial | `worktree/index.ts:25-594` (also `:30-242+` in sessions worklog) | `crates/sessions/src/branch_v2.rs:84` (`open_branch_workspace`); Git worktree-add parity unverified |
| F32 | TUI renderer, layout, dialogs, completion, keybinds | planned | `cli/cmd/tui/*`: `app.tsx`, `context/sdk.tsx:20-58`, `context/exit.tsx:15-50`, `routes/session/index.tsx:155-175,2054,2107`, `component/prompt/autocomplete.tsx`, `context/keybind.tsx`, `ui/dialog.tsx:22-24,144` | Local `crates/cli/src/main.rs:26-80` has Doctor/Session/Models/Serve/Web, no native interactive TUI; `crates/sessions/src/tui_state.rs:9-20+` has bounded pure state machines only (composer, queued drafts, switchers, hints), not the renderer |
| F33 | `/cd` command | missing (upstream too) | Upstream has no `/cd`. Only `cd` match is shell allowlist `tool/bash.ts:26` (+ `bash.txt:5` workdir guidance). `/cd` is a new local backlog feature, not parity | Local build state unverified here |
| F34 | Dedicated all-stats right panel | missing (upstream too) | Upstream has footer/sidebar metadata + read-only `dialog-status.tsx`, no dedicated right-side all-stats panel with the REQ-042 contract | Local REQ-042 design; build state unverified here |
| F35 | Server (Hono/Bun/Node, OpenAPI, mDNS, idempotent stop) | partial | `server/server.ts:18-109` (also `:1-96` in web worklog), `server/instance/index.ts:29-283`, `server/proxy.ts` | `crates/server/src/route_table.rs:15-134` (`ProtocolRoute`, `PROTOCOL_ROUTES`, `served_routes`, `check_served`); `web_host.rs`, `web_route.rs`, `daemon.rs` (adapter/mDNS/OpenAPI parity unverified) |
| F36 | Auth middleware, CORS, CSP, compression | safer deviation (policy decision required) | `server/middleware.ts:16-92`, `server/instance/middleware.ts:15-126` (open-with-warning when no password; localhost/loopback+Tauri+opencode.ai CORS; CSP hashes; streaming uncompressed) | `crates/server/src/web_cors.rs`, `web_headers.rs`, `origin_check.rs`, `control_plane_exposure.rs`; local must decide preserve-vs-strengthen, exact policy unverified here |
| F37 | Workspace proxy (HTTP/WS), remote sync loop | unverified | `server/proxy.ts`, `control-plane/workspace.ts`, `server/instance/global.ts` | `crates/providers/src/proxy_route.rs`, `crates/server/src/remote_ledger.rs` (proxy/sync-loop parity unverified) |
| F38 | ShareNext remote sync | unverified | ShareNext create/sync/remove APIs + local session-share table | `crates/providers/src/int_share_sync_lane.rs`, `share_descriptor.rs` (HTTP-sync parity unverified) |
| F39 | ACP JSON-lines stdio server | unverified | `acp/agent.ts`, `session.ts`, `types.ts`, `cli/cmd/acp.ts` (README limits: no complete update stream, tool-call reporting, mode switch, auth, real terminal, full history) | No confirmed local ACP file in this synthesis (likely missing, unverified) |
| F40 | Headless run/export, web/serve | partial | `cli/cmd/run.ts`, `cli/cmd/export.ts`, `web.ts`, `serve.ts` | `crates/cli/src/main.rs:26-80` (Serve/Web present); pretty rendering/picker parity unverified |
| F41 | JS SDK, spawners, plugin API, workspace adaptors | unverified | `packages/sdk/js/src/*`, plugin API surface | `crates/server/src/app_client.rs`, `clients.rs` (SDK parity unverified) |
| F42 | IDE extensions, desktop shells, GitHub automation, OTLP | optional | IDE (Windsurf/Code/Cursor/Codium/VS Code), Tauri/Electron, Octokit/webhooks/action, OTLP-when-configured | `crates/server/src/desktop_bridge.rs`, `voice_capture.rs`, `auto_report.rs` suggest adjacent work; parity not mandated (unverified, optional) |
| F43 | Config schema (strict JSONC, env/file refs) | partial | `config/config.ts:865-1062`, `:797-1000`, `config/paths.ts:11-167` | `crates/providers/src/config.rs`; `crates/server/src/web_config.rs`; strictness/ref-substitution parity unverified |
| F44 | Todos (transactional replace-all + events) | unverified | Session routes per `server/instance/session.ts` | `crates/sessions` todo handling unverified here |
| F45 | CLI inventory (ACP/MCP/TUI/run/export/import/GitHub/PR/session/plugin/database/upgrade/stats/models/account/providers) | partial | `index.ts:65-241`, `cli/cmd/providers.ts:1-510`, `cli/cmd/mcp.ts` | `crates/cli/src/main.rs:26-80` covers Doctor/Session/Models/Serve/Web; remainder unverified |

---

## 4) Core, runtime, storage, sync, events

- Runtime layers: `effect/app-runtime.ts:51-100` (~47 services) and
  `effect/bootstrap-runtime.ts:15-27` (smaller bootstrap layer). Locally the
  closest observed analogues are the `crates/foundation` crate and
  `crates/server/src/daemon.rs`, but service-count/layer parity was not
  verified. Class: `partial`.
- Directory lifecycle: `effect/instance-state.ts:39-84` (directory-keyed scoped
  cache with disposer/invalidation); `project/instance.ts:26-192` (boot, track,
  reload, dispose, restore per-directory); `project/bootstrap.ts:16-34` (plugin,
  LSP, file, watcher, VCS, snapshot, format, share init with owned
  fork/detach); `project/state.ts:12-70` (per-directory disposal, over-bound
  warning). Local lease/disposal analogues (`auto_lease.rs`) exist by name;
  behavior unverified. Class: `partial`.
- Runner model: `effect/runner.ts:11-207` (Idle/Running/Shell/ShellThenRun);
  `session/run-state.ts:9-108` (BusyError on concurrent work, cancellation,
  runner removal on idle). No local runner file was confirmed in this
  synthesis. Class: `partial` (event/cancel paths exist) bordering `missing`
  for the explicit runner state machine; recorded as `partial` pending a
  targeted grep the implementation lane must run.
- Storage: `storage/db.ts:30-174` (Bun SQLite, WAL, NORMAL synchronous,
  foreign keys, 5s busy timeout, 64MB cache; session/message/part/todo/
  permission/project/workspace/event/account/share schema) and
  `storage/storage.ts:60-330` (JSON KV, transactional re-entrant locks,
  bounded file ops, migrations, NotFound). Local `crates/storage/src/*` was
  listed but not read; WAL/migration/lock parity unverified. Class: `partial`.
- Sync: `sync/index.ts:11-265` (versioned types, sequence allocation, replay,
  projectors, idempotence; old versions, gaps, missing projectors, post-freeze
  definitions fail). Projectors `session/projectors.ts:65-154` tolerate late
  FK updates with warnings. Local `remote_ledger.rs` / `control_decode.rs`
  suggest ledger/decode scaffolds; projector/replay/idempotence parity
  unverified. Class: `unverified`.
- Events: `bus/index.ts:11-193` (typed + wildcard, global bus carries
  directory/project/workspace/payload metadata); `server/instance/event.ts:11-83`
  (SSE, 10s heartbeat, close on disposal). Local `event_bus.rs` /
  `event_stream.rs` exist by name; heartbeat/disposal parity unverified.
  Class: `partial`.
- Bounds that must carry over as explicit local policy (not copied defaults):
  bash 2min timeout + 3s force-kill; tool output 2000 lines / 50KB with
  7-day spill; snapshot 7d / 2MB; retry exp 2s capped 30s, overflow-safe;
  compaction 20k prune / 40k protected tail; `DOOM_LOOP_THRESHOLD=3`
  (`session/processor.ts:24-598`); default `maxSteps = Infinity`
  (`session/prompt.ts:1398`) which a bounded Rust equivalent must NOT copy.

---

## 5) Sessions, agents, context

- Session metadata + CRUD: `session/index.ts:120-180` (project/workspace,
  parent, summary, share URL, title, version, timestamps, permissions, revert
  state) and `:313-362` (API surface). IDs: `session/schema.ts:7-35`
  (descending `ses_`, ascending message/part IDs). Local `branch_v2.rs`
  covers fork/provenance/activity/retry in named APIs; ID-scheme parity
  unverified. Class: `partial`.
- Fork semantics (exact): clone messages, remap assistant parent IDs,
  optional cut at message ID (`session/index.ts:501-552`); recursive child
  removal (`:434-458`); paged/cursor/global listing with directory/workspace
  filter, newest-first (`:601-837`). Local `fork_before_user_message`
  (`branch_v2.rs:393`), `fork_from_message` (`:450`), `list_fork_sessions`
  (`:115`), `synchronize_legacy_shadow` (`:144`) are the comparison points;
  remap/cut/recursion parity unverified. Class: `partial`.
- Messages/parts: `session/message-v2.ts:90-353` (snapshot, patch, text,
  reasoning, file, agent, compaction, subtask, retry, step-start, step-finish,
  tool parts); tool states pending/running/completed/error with bounded
  metadata/attachments expected of caller (`:276-342`); user/assistant shape
  (`:360-452`); persisted-update/remove vs ephemeral-delta split (`:460-509`);
  provider conversion skipping empty/ignored content, per-provider media,
  compacted-attachment removal (`:585+`, `:929-948`). Persisted-vs-ephemeral
  split is a parity invariant. Local `turn_parts.rs` / `transcript_lane.rs`
  are comparison points; part-model parity unverified. Class: `partial`.
- Prompt loop: `session/prompt.ts:70-77` (cancel/prompt/loop/shell/command/
  prompt-part), `:1273-1529` (message creation, compacted-context filter,
  model resolution, subtask/compaction steps, reminders, assistant/tool parts,
  structured output, continue/compact/stop), `:1531-1657` (normal/shell/
  command split; `$1..$N` + `$ARGUMENTS` expansion, shell templates, plugin
  hooks, command events), `:119-151` (`@file`/agent mention resolution),
  `:153-213` (first-real-message title via agent/small model, 100-char cap).
  Local `auto_loop.rs`, `chat_composer.rs` are comparison points; loop
  semantics unverified. Class: `partial`.
- Agents/delegation: `agent/agent.ts:27-52` (mode/model/variant/prompt/
  permissions/options/hidden/native/color/steps), built-ins build/plan/
  general/explore/compaction/title/summary (`:107-234`), config merge
  (`:236-309`); `tool/task.ts:21-176` (permission check, named-agent resolve,
  child session with parent ID, scoped child permissions/tools, model
  inheritance, abort forward, task-ID return + resume; NO background flag -
  synchronous Effect op with cancellation); in-loop subtask (`prompt.ts:
  521-711`) and shell (`:713-887`, owned child processes, separate concurrent
  shell runner); status (`status.ts:8-88`: idle/retry/busy). Local agents
  crate (`crates/agents`) was listed but no file was read here; delegation/
  scoping parity unverified. Class: `partial`.
- Compaction/overflow/revert/summary: `compaction.ts:35-369` (thresholds,
  skill-context protection, auto/overflow modes, summary messages, compacted
  events); `overflow.ts` (output-budget reservation, explicit disable);
  `summary.ts:68-175` (Git diffs between step snapshots, totals/diffs store,
  diff events); `revert.ts:16-177` (snapshot restore, later-message removal,
  pending revert, unrevert; prompt/shell cleanup applies pending revert
  first). Local `auto_compact.rs` is the comparison point; thresholds and
  pending-revert semantics unverified. Class: `partial`.
- Session routes: list/status/get/children/todo/create/delete/update/init/
  fork/abort/share/diff/unshare/summarize/messages/prompt/prompt_async/
  command/shell/revert/unrevert/permission-response
  (`server/instance/session.ts`). Local `PROTOCOL_ROUTES` /
  `served_routes()` (`route_table.rs:49-83`) is the comparison point; route
  coverage unverified here. Class: `partial`.
- Unresolved in worklog (carried forward): full attachment lifecycle,
  queue/steer/stop semantics, export/share-next, full LLM/processor/
  instruction/system wiring, usage API, SSE/TUI event bridge, CLI run
  behavior. Class: `unverified`.

---

## 6) Providers, auth, models

- Registry: `provider/provider.ts:1042-1371` (models.dev + config + env +
  stored API auth + plugins + custom + GitLab discovery + model hooks +
  enable/disable/blacklist/whitelist); bundled list (`:129-153`); custom
  behavior (`:175-821`); SDK/base-URL/key/interpolation/timeout/chunk/SSE/
  OpenAI-item-ID/bundled-npm-file SDK/fetch-cache resolution (`:1375-1511`);
  fuzzy lookup, default/small selection, ModelNotFound/Init (`:1517-1713`).
  Local `ProviderRegistry` (`registry.rs:45-94`: register/get/
  list_by_priority/remove/count) plus `default_providers` (`:113`) is a
  structural foundation only; precedence/filter/SDK-resolution parity
  unverified. Class: `partial`.
- models.dev: `models.ts:111-182` (10s timeout, 5min cache, file lock, hourly
  refresh, disable/path/url flags, snapshot fallback). Local
  `ModelsDevProvider` / `Catalog::from_models_dev_api_json`
  (`catalog/src/lib.rs:19-102`) parses the payload; fetch/cache/lock/refresh
  behavior unverified. Class: `partial`.
- Transforms/execution: `transform.ts:192-368` (caching, cleanup, modality,
  temp/topP/topK, effort/thinking, budgets); `:749-1058` (store/usage/
  thinking/cache, gateway/Azure, output-token limits, 32k default cap);
  `session/llm.ts:80-426` (base->model->agent->variant merge, provider
  options, identity/metadata headers, retries, tool-call repair, hooks).
  Local `provider_dispatch.rs` / `route_compose.rs` are comparison points;
  transform parity unverified. Class: `unverified`.
- Auth store/OAuth: `auth/index.ts:7-91` (`auth.json` under global data path,
  mode 0600; OAuth refresh/access/expiry/account; API key+metadata);
  `auth.ts:96-234` (methods, authorize, callbacks, pending state, validation/
  missing-code/callback errors); provider server routes
  (`server/instance/provider.ts`); CLI (`cli/cmd/providers.ts:1-510`).
  Local `AuthHandler` (`auth.rs:42-78`: add/get/validate/refresh with
  in-memory OAuth2 state) is a foundation; durable 0600 store, OAuth
  connectors, server/CLI routes unverified. Class: `partial`.
- Provider specifics (Codex PKCE/CSRF/browser+headless flows, localhost:1455
  callback, bearer + `ChatGPT-Account-Id`, endpoint rewrite, allowlist,
  refresh-on-expiry: `plugin/codex.ts:13-608`; Copilot device flow/discovery/
  headers: `plugin/github-copilot/copilot.ts:12-379`, `models.ts:110-146`;
  Cloudflare account/gateway: `plugin/cloudflare.ts:1-67`; GitLab/Bedrock/
  Vertex/Workers/Gateway: `provider/provider.ts:276-721`). No local
  provider-specific connectors found. Class: `missing`.
- Retry/overflow/usage/account: `error.ts:9-197` (overflow/quota/invalid/
  stream/413 classification); `retry.ts:12-122` (retry-after-ms/seconds/date,
  bounded exp backoff; overload/rate/free-limit classes); `overflow.ts:1-22`;
  `processor.ts:361` (token/cost accumulation); `account/index.ts:134-456`
  (device login/poll/refresh/org discovery/persist; bounded polling,
  concurrent first-org). Local `retry.rs`, `rate_limit.rs`, `cost.rs`,
  `budget.rs`, `usage_status.rs`, `account_status.rs`, `account_sync.rs`
  exist by name; header-parsing/classification/accumulation parity
  unverified. Class: `partial`.
- Safety boundary (binding on any implementation lane): upstream
  provider-specific headers are documented provider requirements, not a
  license to impersonate. This project must not add headers/traffic patterns
  that impersonate Codex/Claude Code, evade detection, or bypass provider
  controls. Credentials need explicit consent, schema/permission validation,
  redacted storage; live credentials excluded from tests.

---

## 7) Tools, MCP, plugins, skills

- Tool registry: `tool/tool.ts:9-141` (context, execute results, definitions,
  init, permission asking, abort, metadata, output formatting);
  `tool/registry.ts:51-346` (invalid/question/bash/read/glob/grep/edit/write/
  task/fetch/todo/search/code/skill/patch/LSP/plan; conditional question, LSP,
  plan-mode, web/codesearch, apply_patch/edit combos; custom tools from
  `tool`/`tools` dirs + plugins). Explicit per-tool permission mapping
  (read, edit/write, bash, external_directory, glob, grep, list, webfetch,
  websearch, codesearch, skill, todo, LSP, task, question via `ctx.ask()`).
  Local `crates/tools` file inventory was not completed in this synthesis;
  registry parity unverified. Class: `partial`.
- Execution bounds: `tool/truncate.ts:12-60` (2000 lines / 50KB, 7-day
  truncation-dir spill, bounded preview/delegation hint); bash syntax parse,
  external-path discovery, bounded streaming metadata, 2min default timeout,
  3s force-kill; read/write/edit safety (stale-file detection, line limits,
  diff metadata, formatting, LSP/watcher events). Local broker
  (`security/src/lib.rs:151-202`) covers authorize/audit; truncation/spill/
  bash-kill parity unverified. Class: `partial`.
- MCP lifecycle: `mcp/index.ts:1-892` (stdio + Streamable HTTP with SSE
  fallback; 30s default timeout, per-server config; owned connect/disconnect/
  finalizers; descendant cleanup; tool-list watching; prompts; resources;
  cached definitions); statuses connected/disabled/failed/needs_auth/
  needs_client_registration; tool-name sanitization; only connected+cached
  servers exposed; progress resets timeout; calls routed through provider
  transforms + permission asks (`session/prompt.ts:95,440,974`). Local
  `mcp_transport.rs` + `int_mcp_lane.rs` (14.5KB, largest providers file) are
  comparison points; status discrimination/OAuth/caching parity unverified.
  Class: `partial`.
- MCP OAuth: `mcp/auth.ts`, `oauth-provider.ts`, `oauth-callback.ts` (tokens,
  client info, verifier, CSRF, server URL under global data path; RFC 7591
  dynamic registration; 127.0.0.1 callback; browser-failure fallback event).
  Routes `server/instance/mcp.ts:1-245`; CLI `cli/cmd/mcp.ts` (list/add/auth/
  logout/debug); TUI `dialog-mcp.tsx` (sorted select, space toggle) + sidebar
  dots/counts. Local OAuth-flow files (`oauth_flow.rs`, `int_connect.rs`,
  `int_refresh.rs`, `refresh_gate.rs`) exist by name; RFC 7591/callback
  parity unverified. Class: `partial`.
- Explicit upstream absences (verified by worklog source reads; TUI-component
  grep for catalog/bulk/all-stats/right-panel/`/cd` returned no matches):
  upstream has NO catalog search, NO popular-library index, NO catalog
  install flow, NO bulk selection, NO separate checkbox/power controls, NO
  lifecycle persistence, and NO invocation-time disabled-tool recheck.
  Disabled servers are excluded at connection/state layers only. Local
  REQ-042 (`docs/research/REQ-042-tui-mcp-management-design.md`) is therefore
  a newer, stronger contract: filter before payload construction AND again
  before invocation. This is a `safer deviation` by design, not a parity gap.
- Plugins: `plugin/index.ts`, `loader.ts`, `install.ts`, `meta.ts` (internal +
  npm/file loading, config patching, metadata locks, sequential hooks:
  config/auth/provider/chat/permission/command/shell-env/tool-before-after/
  tool-definition). Plugins hold broad Bun/Node access; local Rust must keep
  explicit capabilities and must never treat plugin text as a sandbox. Local
  `hook_bus_v2.rs` / `hooks.rs` are comparison points; loader/hook parity
  unverified. Class: `unverified`.
- Skills: `skill/index.ts:1-264` (project ancestors, config paths, external
  Claude/agents dirs, URLs; SKILL.md required; bounded download concurrency;
  file cache; duplicate-name warnings; permission filtering; untrusted prompt
  material; no signature/catalog pin observed). No local skills file confirmed
  here. Class: `unverified`.
- Commands/permissions/LSP/formatter: `command/index.ts:1-191` (built-in/
  config/MCP/skill merge, template args, execution events);
  `permission/index.ts:1-325` + `arity.ts` (last-match wins, deferred human
  asks, always/reject/cascade, project approval tables, longest-prefix bash
  patterns); LSP (9 ops, bounded processes); configured formatters. Local
  `cmd_patterns.rs`, `exec_policy.rs`, `spawn.rs` are comparison points;
  semantic parity unverified. Class: `partial` (commands/permissions),
  `unverified` (LSP/formatter).
- Data flow (preserved as the parity invariant):
  discover (dirs+plugins) -> registry resolves -> MCP contributes
  connected+cached -> provider transforms + definition hooks -> prompt builds
  payload -> broker asks -> execute with cancel -> truncate/spill -> bus
  updates UI. Any local pipeline must preserve ask-before-execute and
  bounded-output; catalog data stays untrusted, install stays consent-gated,
  disabled state enforced at BOTH payload and invocation boundaries.

---

## 8) TUI, including /cd and REQ-042

- Architecture: `packages/opencode/src/cli/cmd/tui/` (SolidJS,
  `@opentui/solid`/`@opentui/core`); `app.tsx` (route/SDK/sync/KV/theme/
  keybind/dialog/toast/exit/project providers); mouse default-on, off via
  `OPENCODE_DISABLE_MOUSE` or `mouse:false`; `context/sdk.tsx:20-58` (SSE
  subscribe, batched renders); `context/exit.tsx:15-50` (idempotent exit).
  Local: no native interactive TUI (`crates/cli/src/main.rs:26-80`:
  Doctor/Session/Models/Serve/Web only). Class: `planned` (local TUI state
  machines exist; renderer does not).
- Layout: 42-column sidebar when width > 120 or explicitly opened
  (`routes/session/index.tsx:155-175`); split layouts above 120 cols, unified
  permission UI below, narrow below 80 (`:2054,2107`); right-side 42-col
  plugin-slot sidebar (`routes/session/sidebar.tsx`), not a metadata/MCP
  manager; fixed dialog widths 60/88/116 (`ui/dialog.tsx:22-24,144`) with a
  narrow-overflow gap the Rust UI should handle more safely. Local
  `tui_state.rs` (composer, `SubmitKeymap`, `StatusItem`, `ModelEntry`,
  `filter_models`, `keyboard_fallback`, `KeyHint`s incl. Ctrl+P switcher) is
  pure state, not layout. Class: `planned`.
- Prompt/completion/dialogs/keybinds: `@`/`/` completion (frecency/
  fuzzysort, height 10, keyboard+mouse); JSONL history/stash; tag completion
  (5 results); two-key stash delete; palette + slash commands
  (`dialog-command.tsx`, `command/index.ts`); keybind defaults
  (`context/keybind.tsx` + config schema); dialogs (select/alert/confirm/
  prompt/help/theme/model/provider/agent/variant/skill/session/workspace/MCP/
  status); keyboard (arrows/return/Escape, allow/always/reject, question
  digits/tabs, confirm left/right, sidebar toggle, fullscreen permission) +
  global mouse opt-out. Local `tui_state.rs` + `project_context.rs`
  (switchers) cover fragments. Class: `planned`.
- `/cd`: upstream has NO dedicated `/cd` command. Verified: the only `cd`
  match is the shell allowlist in `tool/bash.ts:26`, plus workdir-over-`cd`
  guidance in `tool/bash.txt:5` and `tool/bash.ts:58`. `/cd` is therefore a
  NEW local backlog feature, not upstream parity. Class: `planned` (local),
  `missing` (upstream, permanently - not a gap to close against upstream).
- REQ-042 all-stats panel: upstream has footer/sidebar metadata and read-only
  `dialog-status.tsx` but NO dedicated right-side all-stats panel with the
  REQ-042 contract. Upstream MCP UI is `dialog-mcp.tsx`
  (toggle/connect/disconnect/status-refresh) + `feature-plugins/sidebar/
  mcp.tsx` (dots/counts/collapse) with NO catalog search/install, NO bulk
  selection, NO checkbox/power split, NO lifecycle persistence, NO
  invocation-time disabled-payload filter. `context/local.tsx:371-387`
  derives enabled-from-connected; KV JSON persists sidebar/animation prefs;
  model JSON persists recent model/agent/variant; no TOOL-018 equivalent.
  Class: `planned` (local), `missing` (upstream, permanently).
- REQ-042 design anchor: `docs/research/REQ-042-tui-mcp-management-design.md`
  (`:170-220` catalog schema/search, `:243-293` selection/bulk actions,
  plus lifecycle/persistence/invocation-recheck sections). Related local
  lanes TOOL-016/017/018 and requirement REQ-042 exist in plan/validator
  scope; their build state was not verified in this synthesis.
- Themes/a11y/failures: bundled/custom/system themes + refresh
  (`context/theme.tsx`); mouse+keyboard coexistence; 1s terminal-color
  fallback; OSC52+platform clipboard; 500ms-debounce/3s-hold startup loader;
  fatal path (destroy renderer, flush input, bounded issue report, exit);
  workspace 5xx retry (needs bounded local equivalent). Observed upstream
  tests: keybind, TUI plugin/slot/theme/sync, prompt-part, TUI config; no
  complete interaction snapshot suite identified. Local gap: full TUI
  interaction test inventory. Class: `planned`/`unverified`.

---

## 9) Server, web, ACP, SDK, integrations

- Server: `server/server.ts:1-96` (Hono/Bun/Node adapters, control/instance/
  UI routes, OpenAPI, mDNS, idempotent shutdown); middleware (error mapping,
  optional Basic auth + auth-token, CORS, compression exclusions, logging,
  OPTIONS bypass); `server/control/index.ts` (provider auth, OpenAPI docs,
  logging); `server/instance/global.ts` (health, events, sync events, config,
  dispose, upgrade); `server/instance/index.ts` (project/VCS/file/PTY/
  session/provider/MCP/permission/question/LSP/formatter/TUI/command/agent/
  skill/workspace routes); `server/proxy.ts` (HTTP/WS forward, hop-by-hop +
  routing header strip, connect-queue, `/__workspace_ws` bridge).
  `server/instance/middleware.ts:15-126` (directory from query/header/CWD,
  workspace mapping, remote proxy). Local `route_table.rs:15-134`,
  `web_host.rs`, `web_route.rs`, `web_cors.rs`, `web_headers.rs`,
  `origin_check.rs`, `daemon.rs` are comparison points; adapter/mDNS/OpenAPI/
  proxy parity unverified. Class: `partial`.
- Workspace sync: `control-plane/workspace.ts` (local/remote state, SSE sync
  loop: connected/connecting/disconnected/error); event-log gating behind
  experimental workspace flag; `sync/index.ts` projectors/sequences/replay/
  idempotence. Local `remote_ledger.rs`, `proxy_route.rs` are comparison
  points; loop/flag parity unverified. Class: `unverified`.
- SSE/persistence: bounded connection queues, 10s heartbeat, disposal close;
  ShareNext (session/message/part/model remote persist; create/sync/remove;
  local session-share table). Local `event_stream.rs` (12.4KB),
  `int_share_sync_lane.rs`, `share_descriptor.rs` are comparison points;
  queue/heartbeat/HTTP-sync parity unverified. Class: `unverified`.
- ACP + headless: `acp/agent.ts`, `session.ts`, `types.ts`, `cli/cmd/acp.ts`
  (v1 JSON-lines over stdio: initialize, session new/load/prompt,
  modes/models/variants, text-file read/write, embedded SDK/server; README
  limits: incomplete update stream, tool-call reporting, mode switch, auth,
  real terminal, history restore). `cli/cmd/run.ts` (inline/block renderers,
  attachment conversion); `cli/cmd/export.ts` (JSON + picker); `web.ts` /
  `serve.ts` (network opts, browser open, password warning, mDNS, address
  reporting). Local Serve/Web exist (`main.rs:26-80`); ACP stdio server,
  picker, pretty renderers unverified (likely missing). Class: `partial`
  (serve/web shells), `missing`-leaning-`unverified` (ACP).
- SDK/clients: `packages/sdk/js/src/*` (generated clients, server/process/
  TUI spawners, directory header/query rewrite, abort/timeout); plugin API
  (client/project/worktree/server-URL/shell/hooks/tools/providers/workspace
  adaptors). Local `app_client.rs`, `clients.rs` are comparison points; SDK
  parity unverified. Class: `unverified`.
- IDE/desktop/GitHub/telemetry: Windsurf/Code/Cursor/Codium/VS Code commands/
  keybindings + ephemeral port handshake + `OPENCODE_CALLER`; Tauri/Electron
  shells; web app (session/home/directory/prompt/provider/terminal/context/
  i18n/layout); Octokit/webhooks/workflows/sharing/action; OTLP only when
  configured (service/client metadata, bounded UA/headers). Local
  `desktop_bridge.rs` is adjacent; the rest has no confirmed local analogue.
  Class: `optional` (adopt only on mandate), `unverified` as to current state.
- Stale-path warning (carried forward): older local task cards cite
  `packages/server`, `packages/core`, `packages/protocol`, `packages/client`,
  none present in the observed upstream tree. Those references may target the
  pinned revision or a stale layout; reconcile before implementing anything
  that depends on them.

---

## 10) Security and resource posture, safer deviations

Positive upstream controls to preserve as requirements (not optional):
- `auth.json` mode 0600 (`auth/index.ts:7-91`).
- PKCE + CSRF state on OAuth flows (Codex `plugin/codex.ts:13-608`; MCP
  `oauth-provider.ts` / `oauth-callback.ts`, RFC 7591).
- Timeout caps (models.dev 10s; MCP 30s default + per-server; bash 2min + 3s
  kill), bounded retries (exp 2s, cap 30s, overflow-safe), model-cache TTL,
  redacted status surfaces, bounded polling (account), concurrent first-org.
- Tool output bounds (2000 lines / 50KB, 7-day spill, bounded preview).
- Snapshot bounds (7d, 2MB, per-key semaphore).
- `containsPath` workspace enforcement (`project/instance.ts:26-192`).
- Proxy header stripping (`server/proxy.ts`); SSE disposal close.

Upstream risks the local implementation must NOT copy (deliberate safer
deviations, all `safer deviation` class):
1. Open-by-default server: unset password leaves server open with a warning
   (`server/middleware.ts:16-92`; `web.ts`/`serve.ts` password warning).
   Local policy (`control_plane_exposure.rs`, `origin_check.rs`,
   `web_headers.rs`) must decide preserve-vs-strengthen explicitly; the safe
   default is deny-remote-until-configured. Decision required, recorded as
   unresolved in section 14.
2. Unbounded execution: `maxSteps = Infinity` (`session/prompt.ts:1398`).
   Rust equivalent must be bounded.
3. Unbounded MCP concurrency: startup/tool collection concurrent without the
   bounded guarantees this repo requires. Local must cap with explicit
   semaphore/queue budgets.
4. Credentials in process environment + shallow env copies; fixed localhost
   OAuth ports (Codex `:1455`); npm/file plugin loading with broad Bun/Node
   access; `auth.command` execution; gateway metadata parsing; JWT account-ID
   parse without signature verification. Each needs consent-gating, schema/
   permission validation, redaction, capability scoping before any local
   analogue lands.
5. Prompt/skill/catalog/plugin text is untrusted prompt material, never a
   sandbox, never a signature substitute. Regex/prompt gates are not sandboxes
   (worker contract). Landlock/OS backends need platform testing.
6. REQ-042 double-gate (payload filter + invocation recheck) exceeds upstream
   and stays mandatory locally (section 7/8).
7. Narrow-dialog overflow (`ui/dialog.tsx:22-24,144` fixed 60/88/116): Rust UI
   must handle narrow widths safely rather than reproducing the gap.
8. Workspace 5xx retry needs a bounded local equivalent, not an open loop.

---

## 11) Overlap with existing local requirements and stories

- No `stories/` directory exists in this repo (verified: `ls` shows only
  `requirements/user-requirements.json`). "Stories" below means backlog
  slices referenced by the validator and worklogs (UI-008.., WEB-001..,
  TOOL-016/017/018, SESS-019/020, etc.).
- REQ-041 (`docs/research/REQ-041-provider-compatibility-research.md`, 3.2KB):
  overlaps provider work (F11-F16). Its conclusions should be read alongside
  section 6 before any provider lane starts; compat shims must still honor
  the section 10 impersonation ban.
- REQ-042 (`docs/research/REQ-042-tui-mcp-management-design.md`, 16.7KB):
  the authoritative local MCP-management contract (catalog `:170-220`,
  selection/bulk `:243-293`, lifecycle persistence, double-gate). It is
  NEWER and STRONGER than upstream (sections 7/8). Implementation lanes
  TOOL-016 (catalog search), TOOL-017 (selection/bulk), TOOL-018 (lifecycle
  persistence) belong to it; their build state was not verified here.
- `/cd` + all-stats panel: local backlog features (UI-lane scope per the
  validator's UI-008..UI-018 stale-status list), explicitly NOT upstream
  parity (section 8). Do not let a parity reviewer reclassify them as
  out-of-scope for being non-upstream; their authority is the local backlog,
  not the upstream tree.
- SESS-019/020, branch/fork work (`branch_v2.rs`), auto-compact, switchers
  (`tui_state.rs`, `project_context.rs`), provider lanes (PROV-014),
  ops/integration lanes (OPS-010, INT-002/004/008), web lanes (WEB-001..017,
  web_artifact/web_attachments/web_tool_chooser/desktop_bridge) all overlap
  families F1-F45. The validator currently lists many of these as stale
  (section 1); that staleness is a ledger-sync issue for the repo owner, not
  a technical finding of this report.
- `sources/` gap files (`operations-ownership-gap.json`,
  `integrations-ownership-gap.json`, `routing-ownership-gap.json`,
  `sharing-ownership-gap.json`, `extensibility-remaining-ownership-gap.json`,
  `release-assurance-gap.json`, `enterprise-remote-spec-gap.json`,
  `req017-extensibility-ownership-gap.json`) are the ownership-gap inputs an
  implementation lane must consult for F35-F42 scope boundaries.

---

## 12) High-confidence missing candidates

Listed only where BOTH trees were actually inspected (or upstream absence is
explicit in a worklog AND local absence follows from inspected local files).
Each needs a lane decision: build, defer with reason, or reclassify.

1. V2 entry persistence + create/prompt bridge (F5). Upstream schema exists,
   table commented out, service unimplemented: a shared incomplete surface.
   Local: nothing found. Candidate: define local entry model + persistence
   policy deliberately (not by copying the commented-out table).
2. Provider-specific OAuth connectors: Codex, Copilot, GitLab, Workers/
   Gateway, Bedrock, Vertex (F15). Local has generic auth only. Candidate:
   per-provider connectors with consent/redaction/capability gates; Codex
   bearer/header behavior needs the section 10 impersonation review first.
3. models.dev refresh pipeline (F12): local parses but fetch/cache/lock/
   refresh unverified. Candidate: bounded refresh lane or explicit deferral.
4. Request-transform parity (F13): modality/temperature/effort/budget/output-
   cap mapping unverified locally. Candidate: transform table + tests, or
   explicit subset declaration.
5. Retry-after parsing + overflow classification + cost accumulation (F16):
   local modules exist by name; wire-up unverified. Candidate: header-date
   parsing, 413/overflow classes, step-finish cost rollup.
6. MCP status discrimination + OAuth RFC 7591 + cached-definitions gating
   (F19): local transport exists; status/OAuth parity unverified. Candidate:
   status enum + `needs_auth`/`needs_client_registration` flows.
7. ACP stdio server (F39): no local file found. Candidate: v1 JSON-lines
   server scoped to documented commands, with the README limits carried over
   as explicit non-goals.
8. Snapshots (F29), PTY (F30), full LSP client (F28), skills discovery (F24):
   no local files confirmed in this synthesis. Candidate: one discovery lane
   to confirm absence, then build/defer per item.
9. TUI renderer (F32), `/cd` (F33), all-stats panel (F34): local state/design
   exists, renderer/panels unverified. Candidate: renderer + panel lanes per
   REQ-042 and the UI backlog slices.
10. MCP catalog/search/install + bulk/power/lifecycle + double-gate
    (F20-F22): absent upstream by design verification; local design exists.
    Candidate: TOOL-016/017/018 lanes per REQ-042 (stronger than upstream).

---

## 13) Deferred and unverified candidates

Deferred (deliberately not recommended without a mandate or a safety case):
- D1. IDE extensions, desktop shells, GitHub automation, OTLP (F42):
  `optional`. Adopt only on explicit requirement; each expands attack surface
  (protocol handlers, bundled browsers, webhook secrets, telemetry egress).
- D2. mDNS advertisement: convenient, leaks presence on LAN. Defer or gate
  behind explicit opt-in.
- D3. models.dev hourly refresh + snapshot fallback: network egress + cache
  poisoning surface. Defer until the catalog trust policy is written.
- D4. Skill-URL fetching with bounded concurrency: SSRF/poisoning surface.
  Defer until URL allowlist + hash-pin policy exists.
- D5. npm/file plugin auto-install: arbitrary code execution by design.
  Defer until capability sandbox + consent UX are specified.
- D6. `auth.command` (well-known command execution for credentials): shell
  execution for secrets. Defer until allowlist + audit policy exists.
- D7. Fixed-port OAuth callbacks (`:1455`, 127.0.0.1 MCP callback): port
  collision/fixation. Prefer ephemeral-port + loopback-token binding.
- D8. Open-by-default server: covered in section 10, decision required.

Unverified (worklog claims neither confirmed nor denied here; each needs a
targeted file:line check against BOTH the pinned revision and the local tree
before any lane claims parity):
- U1. Sync projectors/replay/idempotence (F9).
- U2. Runner state machine + BusyError + shell-concurrency (F6 tail).
- U3. Message/part model + persisted-vs-ephemeral split (F4).
- U4. Fork remap/cut/recursion + ID schemes (F3).
- U5. Compaction thresholds + pending-revert application order (F7).
- U6. SQLite WAL/migration/lock settings (F8).
- U7. SSE heartbeat/queue/disposal (F10).
- U8. Provider SDK resolution + fuzzy/default/small lookup (F11).
- U9. Transform tables + 32k cap + header policy (F13).
- U10. Durable auth store + provider/CLI auth routes (F14).
- U11. Tool registry shape + truncation/spill + bash kill (F17/F18).
- U12. Plugin loader/hooks + skill discovery + commands + questions + LSP +
  formatter + snapshots + PTY + todos (F23-F28, F44).
- U13. Server adapters/OpenAPI/mDNS/proxy + workspace sync + ShareNext +
  SDK spawners (F35-F41).
- U14. CLI inventory remainder beyond Doctor/Session/Models/Serve/Web (F45).
- U15. TOOL-016/017/018 and UI-008..UI-018 build state (claimed GREEN in
  commit `3db7402` message; validator disagrees; not adjudicated here).
- U16. Everything above against PINNED `95daf90...` rather than observed
  `07619a0...` (the master unverified item blocking all parity claims).

---

## 14) Exact unresolved questions

1. Pin delta: what changed between pinned `95daf90670b7c039c436c85537da5fbfe2205b41`
   and observed `07619a09d6da7945ea3fdbb4f8ae5ce8dc2b6eeb` in each of F1-F45?
   Owner: repo owner / integration authority. Blocks: all parity claims.
2. V2 intent: is the commented-out `SessionEntryTable`
   (`session/session.sql.ts:100-118`) plus unimplemented create/prompt
   (`v2/session.ts:9-71`) the intended local target, or should local design a
   different entry model? Owner: plan authority. Blocks: F5 lane.
3. Server posture: preserve upstream open-with-warning or strengthen to
   deny-remote-until-configured? Owner: security policy. Blocks: F36 lane.
4. `maxSteps`: what is the mandated local bound replacing upstream `Infinity`
   (`session/prompt.ts:1398`)? Owner: plan authority. Blocks: F6 lane.
5. MCP concurrency bounds: what semaphore/queue/timeout budgets govern local
   MCP startup and tool collection? Owner: plan authority. Blocks: F19 lane.
6. Header policy: which provider-required headers are documented requirements
   vs prohibited impersonation (Codex `ChatGPT-Account-Id`, Copilot provider
   headers, GitLab gateway headers)? Owner: security policy. Blocks: F15 lane.
7. Stale-path cards: do `packages/server|core|protocol|client` references in
   older task cards mean the pinned revision or a stale layout? Owner: repo
   owner. Blocks: F35-F41 lanes.
8. External plugin bodies (Poe, GitLab plugin): unverified per providers
   worklog checklist. What is their content and is any of it in scope?
   Owner: research lane. Blocks: F15/F23.
9. TUI test inventory: upstream interaction snapshot suite not found; what is
   the local TUI test strategy (headless state machines + golden snapshots)?
   Owner: plan authority. Blocks: F32-F34 lanes.
10. Validator staleness (UI-008..018, WEB-001..005, AUTO-007, PROV-014,
    OPS-010, EXT-013, SESS-019/020, ROUTE-012, REL-004): ledger-sync issue or
    real scope drift? Owner: repo owner / integration authority. Blocks: lane
    acceptance, not technical design.
11. OAuth ports: ephemeral-port binding or fixed (`:1455`, 127.0.0.1) parity?
    Owner: security policy. Blocks: F14/F19 lanes.
12. OTLP/IDE/desktop/GitHub: in scope or explicitly out? Owner: plan
    authority. Blocks: F42.

---

## 15) Final checklist

- [x] Scope and pins recorded: observed `07619a09d6da7945ea3fdbb4f8ae5ce8dc2b6eeb`
  vs plan-pinned `95daf90670b7c039c436c85537da5fbfe2205b41`; never equated.
- [x] Methodology stated; seven classifications used consistently.
- [x] Feature-family matrix complete: 45 families F1-F45, each classified with
  upstream path:line/symbol and local path/symbol where known.
- [x] Core/runtime/storage/sync/events covered (section 4).
- [x] Sessions/agents/context covered (section 5).
- [x] Providers/auth/models covered (section 6) with impersonation ban.
- [x] Tools/MCP/plugins/skills covered (section 7) with data-flow invariant.
- [x] TUI covered including `/cd` and REQ-042 (section 8).
- [x] Upstream has NO `/cd` (only `tool/bash.ts:26` allowlist + workdir guidance).
- [x] Upstream has NO catalog/search/install, NO bulk MCP selection, NO separate
  checkbox/power controls, NO lifecycle persistence.
- [x] Upstream has NO dedicated all-stats right panel (footer/sidebar metadata +
  read-only `dialog-status.tsx` only).
- [x] Upstream has NO invocation-time disabled-tool recheck (connection/state
  exclusion only; REQ-042 double-gate is newer/stronger local contract).
- [x] No implementation claims invented; unverified items marked as such.
- [x] Server/web/ACP/SDK/integrations covered (section 9).
- [x] Security/resource safer deviations listed (section 10).
- [x] Overlap with local requirements/stories recorded; missing `stories/`
  directory noted (section 11).
- [x] High-confidence missing candidates listed (section 12).
- [x] Deferred/unverified candidates listed (section 13).
- [x] Exact unresolved questions listed with owners and blockers (section 14).
- [x] Repository validation failure mentioned as pre-existing diagnostic
  context only (section 1).
- [x] No network used. No tests run. No secrets touched.
- [x] One file only: `docs/research/UPSTREAM-V2-PARITY-SYNTHESIS.md`; no edits
  to ralph.json, FEATURES.md, requirements, tasks, product code, or worklogs.
- [x] File existence and line count verified (see verification note below).

Verification: `test -f docs/research/UPSTREAM-V2-PARITY-SYNTHESIS.md && wc -l
docs/research/UPSTREAM-V2-PARITY-SYNTHESIS.md` run from
`/home/rashid/projects/opencode-rk`; file exists (this file).
