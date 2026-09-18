# RAW_FEATURE — Feature map vs OpenCode (raw inventory, not acceptance)

Status: RAW INVENTORY. This file lists features honestly by implementation
state. **Nothing here is a release claim** — "verified" means real code +
frozen tests were observed in this repo; anything less is called out as such.
Baseline: OpenCode (sst/opencode) public feature surface as of 2026-09-18
(docs: opencode.ai/docs — intro, agents, commands, config, plugins, tools,
permissions, themes, keybinds, LSP, MCP, share, desktop/IDE).
This repo: `opencode-rk` (Rust/Tokio native rewrite) @ `17b15e6`.

---

## SECTION 1 — OpenCode feature parity ("we start with OpenCode → we should have ALL the features OpenCode has")

The bar: every feature below is something OpenCode ships today. Status legend:

| Status | Meaning |
|---|---|
| ✅ VERIFIED | Real implementation + passing tests observed in this repo (cite given) |
| 🟡 PARTIAL | Core code exists and is wired, but end-to-end journey or some surface unverified |
| 🔴 LANDED-UNWIRED / GAP | Code or tests exist but not reachable end-to-end, or genuinely missing |
| ⬜ NOT TOUCHED | No implementation in this repo |

### 1.1 Core app & session lifecycle

| OpenCode feature | Status | Evidence / gap |
|---|---|---|
| Bare `opencode` launch opens the TUI (no subcommand) | 🟡 PARTIAL | Bare launch → chat TUI with auto-daemon spawn proven by 4/4 `crates/cli/tests/default_tui.rs` E2E tests + release smoke; but full golden installed journey (APP-012) and real-provider turn in the TUI are not yet evidenced |
| Client/server architecture (daemon owns execution; TUI/CLI/web are clients) | ✅ VERIFIED | `crates/server` daemon + `crates/cli` client; `web_capabilities` shared-service contract; APP-003/008 stories |
| Session create / list / resume / persistent history | ✅ VERIFIED | `crates/sessions` (300 tests), `crates/storage` (177 tests) |
| Session fork / revert / rollback split | ✅ VERIFIED (state machines) / 🔴 in live path unproven | SESS-011/018/019 typed fork boundary + revert-vs-rollback modules |
| /undo, /redo message reversion | 🔴 GAP | revert machinery exists (SESS-019); slash-command surface not wired to it in TUI |
| Session share links (/share) | 🟡 PARTIAL | SHARE-001..005 lanes landed (publish service, revocation, redaction — `worklog/SHARE-*`), browser-verified journey not evidenced |
| Session titles/summaries auto-generation | 🔴 GAP | Hidden `title`/`summary` agent pattern from OpenCode not implemented |
| Auto-compact long context + summarizer agent | 🟡 PARTIAL | SESS-020 thresholds + breaker module; compaction agent (AGENT-022/024) state machines exist; live wiring unproven |
| Timestamps on each response | ✅ VERIFIED | SESS-002/UI-003 |
| Export session / headless run (`opencode run`) | ✅ VERIFIED | HEAD-001/002: headless execution with renderers, typed exit codes, redacted export |
| /init project analysis → AGENTS.md | ⬜ NOT TOUCHED | No equivalent command |

### 1.2 TUI (native terminal UI)

| OpenCode feature | Status | Evidence / gap |
|---|---|---|
| Native, responsive, themeable TUI (OpenTUI-grade: real renderer, not line-echo) | 🟡 PARTIAL | `crates/opentui-sys` + `native/opentui-fork` wrapper (TUI-001/002 spike modules), 13 native_* pure-state modules (palette, navigation, approvals, status, layout, composer, console, host, input, keys, mouse, shell, caps) all wired + tested; **full Zig-renderer-backed shell render (TUI-003) and platform acceptance (TUI-010) not complete** |
| Streaming transcript, markdown/code/diff rendering | 🟡 PARTIAL | TUI-005 lane + `tui_entry` line-mode renders; native renderer integration incomplete |
| Composer with @file fuzzy mention, paste, images drag-drop | 🟡 PARTIAL | Composer state + input modules (UI-014, `native_composer.rs`); image paste/drop not evidenced |
| Plan mode ↔ Build mode switch (Tab) | 🟡 PARTIAL | AGENT-031 plan-mode + structured review child module; TUI toggle not wired end-to-end |
| Approval prompts / interruption / retry UI | 🟡 PARTIAL | `native_approvals.rs` (TUI-008) state machine + approvals in turn path (turn_service); live interruption journey unproven |
| Status bar: model, context, cost, tokens | ✅ VERIFIED (state) | UI-015/016 + `native_status.rs` + REL-004 per-turn cost counters |
| /memory viewer, context detail | ✅ VERIFIED (state) | UI-017, UI-016 modules |
| Keybindings help (/keys) + custom keybinds | ✅ VERIFIED (state) | UI-018 `footer_hints`/`keybinding_help`; custom keybind config file parsing not evidenced |
| Command palette (/ commands) | ✅ VERIFIED (state) | `native_palette.rs` (TUI-006) |
| Session tabs / child-session navigation (subagent inspection) | 🟡 PARTIAL | `native_navigation.rs` (TUI-007) + session tree in storage; live subagent tab flow unproven |
| Themes | 🔴 GAP | Theme system (UI-009 story) — no theme engine wired into native shell |

### 1.3 Agents

| OpenCode feature | Status | Evidence / gap |
|---|---|---|
| Primary agents (build/plan) with per-agent model, prompt, temperature | 🟡 PARTIAL | `crates/agents` registry + AGENT-002/006 config lanes (39 tests); markdown/JSON agent-file loading unverified |
| Subagents (general/explore/scout equivalents) + Task tool delegation | 🟡 PARTIAL | AGENT-003/004/005 delegation modules + `app_delegation.rs`; live parent→child round-trip through the daemon not evidenced |
| Foreground AND background subagents | 🔴 LANDED-UNWIRED | AGENT-003/004 + `agent_executor.rs` (PAR-004) + headless bridge — background execution loop not integrated into live turns |
| Custom agents from config/markdown | 🔴 GAP | No agent-file discovery/loading surface |
| Agent permission profiles (edit/bash deny for plan) | ✅ VERIFIED (policy) | SEC-001/003 policy engine + execpolicy prefix rules (SEC-018/019, 492 security/tools tests) |
| Agent steps cap (max agentic iterations) | ✅ VERIFIED | `LoopController` MAX_STEPS=64 + `should_stop` cap semantics + `max_steps` stop reason (agent_loop.rs, 13 lib tests) |
| Subagent lifecycle: context fork, fresh spawn, resume failed, retry w/ backoff, mid-session model swap, context compression, structured handoff, pools, output aggregation, cancellation | 🟡 PARTIAL | AGENT-016..030 modules exist as typed state machines w/ tests; several are state-only, not all live-wired |
| Navigate into child sessions (cycle keys) | 🟡 PARTIAL | navigation module + REQ-013 stories |
| Per-agent permission.task (which subagents an agent may invoke, glob rules) | ⬜ NOT TOUCHED | Not in plan |

### 1.4 Tools

| OpenCode feature | Status | Evidence / gap |
|---|---|---|
| Built-in tool set: bash, read, write/edit, glob, grep, list, webfetch, todowrite/read | ✅ VERIFIED | `crates/tools` registry (bash/shell/echo executable, 492 tests suite share), typed tool contract (TOOL-006) |
| Agentic loop: tool calls execute, results feed back, model continues, terminates cleanly | ✅ VERIFIED | Server bounded agentic tool loop (`crates/server/src/agent_loop.rs` + live turn stream wiring, E2E `crates/server/tests/agent_loop_turns.rs` E1: real bash exec, `function_call_output` fed back, final answer) — committed `c67b6c0` |
| Stop reasons / finish reasons (stop, max_tokens, max_steps) mapped | ✅ VERIFIED | StopReason mapping completed/incomplete/failed + cap → `max_steps` (responses.rs + agent_loop) |
| Tools in provider request (schemas, function_call items) | ✅ VERIFIED | typed transcript items + tool schemas (`56f5500`, `225fed4`) — **opt-in via `OPENCODE_RK_TURN_TOOLS` (default off)** |
| Tool timeout / cancel / output byte bounds | ✅ VERIFIED | DISC-104 lane, bounded executor |
| MCP (Model Context Protocol) servers + tools, permission gating | 🟡 PARTIAL | TOOL-015..020: catalog search, lifecycle toggles, disabled-MCP exclusion (state modules); MCP spawn behind broker with SSRF guard = DISC-111 lane — live MCP server spawn not evidenced |
| LSP integration (diagnostics fed to model) | 🔴 LANDED-UNWIRED | DISC-109 LSP client lane exists behind permission broker; not in live turn path |
| Git tooling lane | 🔴 LANDED-UNWIRED | DISC-110 git lane behind broker — not reachable from app |
| WebFetch / WebSearch tools | 🔴 LANDED-UNWIRED | Tool registry entries; network fetch path not proven |
| Custom tools via plugins | 🔴 GAP | Plugin-registered tools not loadable (EXT lanes cover config/hooks, not runtime tool loading) |
| PTY / interactive terminal tool | 🔴 LANDED-UNWIRED | PTY control surfaces (NET-011 remote PTY, shell lane RUN-001 concurrent shell lane) — local TUI PTY journey unproven |

### 1.5 Permissions & security (OpenCode permission model)

| OpenCode feature | Status | Evidence / gap |
|---|---|---|
| Permission system: allow/ask/deny per tool, per bash pattern (globs) | ✅ VERIFIED | SEC-001/003 + execpolicy prefix rules + wildcard matching (492 tests) |
| `*` wildcard cannot bypass mandatory controls | ✅ VERIFIED | REQ-030 SEC-017 explicit anti-bypass tests |
| Pre/post tool hooks (plugin hooks) | ✅ VERIFIED (bus) | SEC-010/011/020 bounded hook bus + EXT-008; JS plugin runtime NOT included (native mode, by contract) |
| OS-enforced .env / sensitive-file restrictions | 🟡 PARTIAL | SEC-004/005/014 policy modules; Landlock/sandbox backend must be proven on actual platform (DISC-106) — **real OS sandbox not verified** |
| System files readable, not agent-editable | ✅ VERIFIED (policy) | SEC-006 |
| Approval dialog mid-turn (request_permissions) | 🟡 PARTIAL | TOOL-010 module + approvals state; live human-in-the-loop round-trip unproven |

### 1.6 Providers & models

| OpenCode feature | Status | Evidence / gap |
|---|---|---|
| Multi-provider (Anthropic, OpenAI, Google, local/OpenAI-compatible…) | ✅ VERIFIED (protocol) | `crates/providers` (392 tests): OpenAI Responses stream + typed tools; offline differential contract fixtures (PROV-024) |
| models.dev-backed model catalog, searchable API | ✅ VERIFIED | `crates/catalog` (CAT-001/002/007) |
| Provider auth: API keys, OAuth connectors (Codex, Claude Code), credential import | 🟡 PARTIAL | PROV-015..024: auth profiles, PKCE/loopback connectors, consent import, keyring hardening modules — live OAuth round-trip requires credentials (blocked evidence) |
| Usage/limits telemetry | 🟡 PARTIAL | PROV-021 bounded snapshots module |
| Streaming responses (SSE) incl. reasoning summaries | ✅ VERIFIED | responses.rs stream parser (reasoning events, function_call, incomplete/failed handling) |
| Multi-account routing (9router-style) | 🔴 LANDED-UNWIRED | ROUTE-001..012 + `app_routing.rs` lane — not in live provider selection path |
| Effort levels (reasoning effort per agent/model) | 🟡 PARTIAL | CAT-004/AGENT-006/ROUTE-012 state; provider param passthrough unverified |

### 1.7 Config, extensibility, ecosystems

| OpenCode feature | Status | Evidence / gap |
|---|---|---|
| Config file (opencode.json), global + per-project | 🟡 PARTIAL | BASE-006 config module; full config surface parity (formatAgents, instructions globs, etc.) not complete |
| AGENTS.md / CLAUDE.md / rules auto-load into system prompt | 🔴 GAP | SystemPrompt.custom() equivalent not implemented |
| Custom slash commands (markdown, $ARGUMENTS, !shell, @file) | 🔴 LANDED-UNWIRED | EXT-001/002 skill/command modules + UI-010 — command template engine not reachable in TUI |
| Skills (SKILL.md load/invoke, safe extraction) | 🟡 PARTIAL | EXT-013 safe-extraction + EXT-001/002 modules |
| Plugins (TS in Bun for OpenCode) → here: native hooks + config plugins | 🟡 PARTIAL | EXT-005/009/012 manifest/hook lanes; UI plugin behavior story (UI-012); **no JS plugin runtime by design (native mode)** — Solid/TS UI plugins are a separate compat product |
| Formatters (config-driven code formatters) | ⬜ NOT TOUCHED | Not in plan |

### 1.8 Storage & sync

| OpenCode feature | Status | Evidence / gap |
|---|---|---|
| Embedded storage (no Postgres/Mongo install) | ✅ VERIFIED | `crates/storage` sqlite + content-addressed dedupe (DB-017/018) |
| Dual rollout record (JSONL truth + sqlite snapshot) | ✅ VERIFIED | DB-017 |
| Import large existing OpenCode data | 🟡 PARTIAL | DB-012/016 + DISC-108 (large/blob import with migration agreement) — migration agreement on real dataset pending |
| Backup / restore / corruption detection | ✅ VERIFIED | DISC-107 + backup_v2 lane |
| Versioned sync event log + projector replay + persisted-vs-ephemeral split | ✅ VERIFIED | SYNC-001/002 + `event_cursor` |
| Share backend (publish, revoke, redact) | 🟡 PARTIAL | SHARE-001..005 + DISC-116 |

### 1.9 Protocols & clients

| OpenCode feature | Status | Evidence / gap |
|---|---|---|
| Typed SDK client + spawners (server/process/TUI) | ✅ VERIFIED | SDK-001/002 |
| ACP v1 JSONL bridge (stdio) with typed Unsupported | ✅ VERIFIED (framing) | ACP-001/002; live session wiring to daemon events = DISC-112 lane |
| Workspace HTTP/WS proxy + remote sync loop | ✅ VERIFIED | WSX-001/002 |
| Web client (ChatGPT-class local web UI on same daemon) | 🟡 PARTIAL | `web/` React client + server web host (WEB-006..017); prior audits flagged single-page/limited actions; E2E-APP lane repaired server+web; **full parity journey not browser-evidenced** |
| Desktop app / IDE extension | ⬜ NOT TOUCHED | Not in plan |
| Voice/dictation | 🔴 LANDED-UNWIRED | WEB-016 lane; native audio adapter absent |

---

## SECTION 2 — EXTRA features beyond OpenCode (things we have that OpenCode does not)

### 2.1 COMPLETED (implemented + frozen tests green in this repo)

| Feature | Evidence |
|---|---|
| **Bounded agentic tool loop in live turn streams** — iteration cap (MAX_STEPS=64, env-overridable), tool execution through policy-checked executor, typed `function_call`/`function_call_output` feedback rounds, StopReason mapping (completed/incomplete/failed + `max_steps`), safe-default policy (tools opt-in via `OPENCODE_RK_TURN_TOOLS`) | `crates/server/src/agent_loop.rs`, `crates/server/tests/agent_loop_turns.rs` (E1–E4), commits `56f5500`/`225fed4`/`c67b6c0` |
| **Task-claim coordination ledger for agent fleets** — fail-closed claim fencing (in-progress/blocked), orchestrator-only reclaim with evidence, evidence-noted completed/blocked statuses, plan-synced ready-task computation, scratchpad handback reporting | `tools/completion_claims.py`, `tasks/completion/claims.json`, `tests/completion/test_claims.py` (13 frozen tests), commit `17b15e6` |
| **Worker protocol for multi-agent orchestration** — claim-before-touch, per-worker scratchpad session persistence, commit-push-per-feature, `lane/<TASK-ID>` worktree workflow (branch persisted on remote, worktree removed after merge) | `.agents/WORKER.md`, `AGENTS.md` "Task-claim ledger" + "Landing work" sections, `prompts/COMPLETE_APP.md` |
| **Doctor diagnostics command with actionable next steps** | `opencode-rk doctor` (release smoke verified), OPS-010 |
| **Default-to-TUI bare launch with auto daemon spawn** (OpenCode requires manual serve; ours self-spawns) | `crates/cli/src/chat.rs`, `crates/cli/tests/default_tui.rs` 4/4 |
| **Native TUI pure-state lane set** — palette, navigation, approvals, status, layout engine, caps, console, host, input, keys, mouse, shell (13 modules, all wired, in-file frozen tests) | `crates/cli/src/native_*.rs`, commit `aea6210` |
| **Remote device lanes (NET-005..013)** — outbound connector, device inventory, tab routing, remote prompt/approvals, file/diff workflows, PTY control, replay/reconnect, revocation (state + protocol modules, wired + tested) | `crates/server/src/remote_*.rs`, commit `aea6210` |
| **Serial-mandate turn stream API** with TURN-STREAM-GATE determinism | `crates/server/tests/session_turn_stream_api.rs` (documented serial contract, 2/2) |
| **Provider-boundary tap with redaction + offline debug export** (adopted from harness mining; not an OpenCode feature) | PROV-013/014 lanes |
| **Content-addressed transcript dedupe** | DB-018 |
| **Headless run + redacted session export renderers** | HEAD-001/002 |
| **Turn submission state machine** (StartOrSteer/StartIfIdle, typed busy reasons — adopted from codex mining) | AUTO-007 lane |
| **Dangerous-pattern denylist + execpolicy prefix rules + permission-`*` anti-bypass** (hardened beyond OpenCode's ask/allow/deny) | SEC-018/019/017 |

### 2.2 IN PROGRESS (lanes claimed/landed but not complete)

| Feature | State |
|---|---|
| **Native OpenTUI Zig-renderer shell** (real native TUI beyond line mode) | TUI-003/004/010 lanes partially landed (`crates/opentui-sys`, `native/opentui-fork`); TUI-010-CAPS currently claimed in the ledger (in progress) |
| **Web client full parity** (ChatGPT-class: edit/retry/regenerate, attachments, deep research, pins/search, projects/memory, artifacts) | E2E-APP lane landed server + web repairs; WEB-013/015 card contracts still need future lanes |
| **Remote mobile clients** (iOS/Android apps, pairing, tabs, notifications) | MOB-001..006 planned; framework freeze DISC-118; nothing installable yet |
| **Cloudflare Tunnel gateway (self-hosted remote mode)** | NET-004 lane + COMPLETION_REMOTE.md design; no deployed gateway evidence |
| **Auth connectors live (Codex/Claude Code OAuth, credential import)** | PROV-016..018 modules + bounds fixtures; live flow blocked on credentials/consent |
| **MCP real server spawn behind broker with SSRF guards** | DISC-111 lane code; live spawn unproven |
| **LSP diagnostics into live turns** | DISC-109 lane; not wired into turn path |
| **Backup v2 / large history import on real datasets** | DISC-107/108; migration agreement pending |

### 2.3 NOT TOUCHED (beyond-OpenCode ideas with no implementation)

| Feature | Note |
|---|---|
| Desktop app (Electron/Tauri shell) | Not in plan; OpenCode itself ships one — see Section 1.9 gap |
| IDE extensions (VS Code/JetBrains) | Not in plan |
| Voice input on mobile/desktop | WEB-016 only covers web lane state |
| Hosted multi-tenant SaaS control plane | Explicitly out of scope by contract (self-hosted only) |
| Per-agent task-permission globs (which subagent may spawn which) | Considered in OpenCode parity (1.3) — absent both here and unclaimed as extra |
| Plugin marketplace / ecosystem registry | Not in plan |

---

## SECTION 3 — UPCOMING FEATURES (roadmap, not started unless noted)

These are beyond-OpenCode capabilities we commit to building. Status is
`PLANNED` — no lane claimed in `tasks/completion/claims.json` for any of them
yet. Each lists the design sketch and what will count as done (all types of
test code written, feature implemented, frozen tests green with zero test
edits — per `.agents/WORKER.md`).

### 3.1 LOOP — Claude-Code-style custom loop

**What:** a first-class user-facing agentic loop mode: the model keeps a
persistent goal list, plans → executes → verifies → re-plans across turns,
with user-visible loop state (current goal, step budget, retries) and the
ability to steer/interrupt/re-scope mid-loop. Distinct from today's single
bounded tool loop (`agent_loop.rs`): LOOP is a multi-turn *task-level* driver
with checkpoint/resume so a loop survives daemon restarts.

**Design sketch:**
- Build on `LoopController` (cap, `should_stop`) + turn submission state
  machine (AUTO-007) — one loop task per session, `BusyError` semantics kept.
- Loop state persisted via the sync event log (SYNC-001) so resume is
  deterministic replay, not ad-hoc state.
- Steer events (user message mid-loop) enqueue as loop-plan amendments;
  cancel reclaims the runner (RUN-001 contract).

**Done when:** loop start/steer/interrupt/resume scenarios have frozen tests;
checkpoint replay after daemon restart passes; iteration cap + stop reasons
surface in TUI status.

### 3.2 ULTRA MODE — orchestrator-written raw Rust subagents

**What:** our own ultra/ultracode-class mode: the user invokes an *ultra*
skill, and the orchestrator agent is asked to **write raw, efficient Rust
itself** — a custom subagent compiled and run natively for that task — instead
of driving a generic model loop. Claude-ultracode-like power, but more
efficient: the hot path is compiled Rust, not a JS/TS agent harness.

**Design sketch:**
- Ultra skill = orchestrator prompt contract: given a task, emit a typed
  subagent crate/module implementing the existing subagent trait surface
  (`crates/agents`), then build it with a bounded, sandboxed `cargo` invocation.
- Generated code runs behind the **same permission broker, byte budgets and
  policy engine** as every other tool (no broker bypass because it is Rust);
  build sandbox uses the OS sandbox backend (DISC-106) with inherited
  capabilities closed.
- Compile cache: content-addressed (same approach as DB-018 dedupe) keyed on
  generated source hash; build artifacts bounded and garbage-collected.
- Safety: every generated crate must compile with `forbid(unsafe_code)` by
  default; `unsafe` requires an explicit user approval prompt. Denylist of
  forbidden APIs (network exfil, secret paths) enforced at codegen review +
  execpolicy level.
- Falls back to normal model subagent if build fails N times (typed reason).

**Done when:** end-to-end frozen tests: task → generated Rust → compiled →
executed → result merged into session transcript; denial paths (policy,
build failure, unsafe without approval) verified; resource bounds measured
within the 8 GiB envelope.

### 3.3 `/CONTEXT` — context usage command

**What:** TUI/web slash command rendering live context-window usage: tokens
used vs model limit, breakdown by segment (system prompt, AGENTS/rules,
history, tool results, current turn), and compaction headroom.

**Design sketch:**
- Server exposes a bounded `/context` snapshot on the session (token counts
  per segment — provider tokenizer counts where available, else documented
  estimator); UI-016 context-detail state module already has the display
  shape.
- Values must reflect what is *actually sent* to the provider (tap into the
  provider request assembly), not an optimistic estimate.

**Done when:** frozen tests for the snapshot computation (segment split,
bounded size) and UI render; values reconciled against a real provider
request fixture.

**Status note:** display-side state exists (UI-016); the server-side token
accounting + command surface are the new work.

### 3.4 `/MEMORY` — memory files loaded command

**What:** command showing every memory/rules file currently loaded into the
system prompt, with source path, glob that matched, byte/token size, and load
order — plus which files were *skipped* and why (glob miss, conditional rule,
size cap).

**Design sketch:**
- Extends the UI-017 `/memory` viewer state module with live data from the
  rules loader (3.5).
- Reports are computed from the same immutable load record the server used,
  so the TUI cannot show a different truth from what the provider received.

**Done when:** frozen tests cover loaded/skipped classification, ordering,
and bounded output; TUI + web render from the same snapshot.

### 3.5 Rules folder + path globs — conditional memory loading/unloading

**What:** a project rules folder (e.g. `rules/` or `.opencode/rules/`) where
each memory file carries **path globs**; a file is loaded into context only
when the current session's touched/queried paths match, and is **unloaded**
(dropped from subsequent rounds) when no longer relevant — conditional,
gitignored-style memory control.

**Design sketch:**
- Frontmatter schema: `globs:` (match patterns), optional `always: true`,
  priority, size cap. Matching is evaluated against the session's file-touch
  set (reads/writes/edits this session) + explicit @mentions.
- Loader is pure-state and testable: given (file set, globs, touch set) →
  ordered include/exclude list, with a stability rule (hysteresis: a file
  stays loaded for N subsequent rounds after last match to avoid flapping).
- Unloading = excluded from the *next* provider round's system segment; the
  sync log records load/unload events for replay.
- This is the generalization of the Section 1.7 "AGENTS.md/rules
  system-prompt injection" parity gap — building 3.5 closes that gap too.

**Done when:** frozen tests: glob matching, hysteresis, unload semantics,
cap enforcement, interaction with /MEMORY (3.4) and /CONTEXT (3.3) snapshots;
loaded set provably equals what the provider request carried.

### 3.6 CI/CD-compatible non-interactive CLI

**What:** every capability usable from CI: non-interactive, no-TTY, typed
exit codes, machine-readable output (JSON/JSONL), explicit timeout/cancel,
and no hidden TTY-only fallbacks.

**Design sketch:**
- Build on HEAD-001/002 (headless run + redacted export, typed exit codes
  already exist): the gap is *coverage* — doctor, session management, approvals
  policy (headless deny/allow matrix), and the agentic loop must all work
  without a TTY.
- `--output json|jsonl|text`, `--no-color`, strict exit-code contract
  (documented table), `--max-steps`/`--timeout` overrides, and a
  `--require-approval-policy` flag that **fails closed** in CI when a step
  would need interactive approval (never auto-approves silently).
- GitHub Actions/GitLab recipes in docs; canary job in our own CI (SHIP-003
  pattern, budgeted).

**Done when:** frozen tests run the full command surface under pipes (no
TTY): deterministic exit codes, JSON schema stable, approval-required steps
fail closed with typed reasons; docs include CI recipes.

**Status note:** HEAD-001/002 headless core is ✅ verified — 3.6 is the
completion of that lane into full CI parity, not a greenfield feature.

### ⚠️ 3.7 WORKFLOW VIEWS — timeline, infinite canvas, custom workflow creation (HIGH-VISIBILITY GAP)

**What we committed to:** view the agent's work (1) as a **timeline** (the
default view: messages, tool activity, streaming), (2) on an **infinite
canvas** of interactive nodes and edges (sessions/subagents as nodes,
delegation/fork relationships as edges), and (3) **custom workflow creation**
(user-authored multi-step agent workflows).

**Current state (evidence-cited, 2026-09-18 @ `7b2ba00`):**

| Piece | State | Evidence / gap |
|---|---|---|
| Timeline view (default) | 🟡 ~60% — data model DONE, native render MISSING | `crates/cli/src/native_transcript.rs` (TUI-005 lane, 476L, 7/7 tests): streaming token deltas, tool states, hostile-output sanitization, bounded virtualized window, scrollback-stable marker. `native_status.rs` (TUI-009) + `native_navigation.rs` (TUI-007) give surrounding surfaces. **Nothing renders it in the native shell yet** — TUI-003 (shell on daemon state) and the headless native snapshot are open repair children (AUD-011) |
| Infinite canvas (nodes+edges) | 🔴 NOT TOUCHED as a view — but the data substrate exists | No task card, module, or worklog mentions a canvas/graph view (repo grep: zero hits). The data layer is ~40% there: `crates/agents/src/app_delegation.rs` persisted parent→child edge, fork provenance (`native_navigation.rs` `Tab.parent`), sync event log replay (SYNC-001). The VIEW is 0% |
| Custom workflow creation | 🟡 substrate partial, editor absent | OpenCode V2 semantics (recorded in `sources/req017-extensibility-ownership-gap.json`) fold named reusable workflows under skills; our EXT-001/002/013 skill lanes landed as modules but are not reachable end-to-end. No workflow editor/creator UI exists in any form |

**How the OpenTUI + Rust port changes this:**

- The timeline is no longer a web DOM surface: it renders through the **owned
  Zig fork's renderer via the 35-symbol C-ABI bridge** (AUD-011 verified:
  `libopentui.so` resolves all required symbols; flex/clip/mouse/color subset
  ported in `native_layout_engine.rs`). Remaining work is one integration
  lane: paint `native_transcript` state through the native shell's paint loop
  (TUI-003 + TUI-005 integration).
- The "infinite canvas" splits honestly per client — same daemon event stream
  feeds all three, one source of truth:
  - **Web client owns the true interactive canvas** (pointer drag, zoom,
    pan — React `web/` app on the shared daemon).
  - **TUI owns a bounded keyboard-navigable graph view** (braille/box-drawing
    nodes, edge lines, viewport panning) — terminal-constrained but native.
  - **Mobile owns read-only** (MOB-003 tabs pattern).
- Custom workflow creation follows the skills substrate: a workflow is a
  named skill with typed step edges; the TUI palette (`native_palette.rs`)
  and web composer are the creation surfaces.

**Needed lanes (none claimed in the ledger yet):**
1. `WF-TUI-TIMELINE` — native shell renders transcript/timeline state
   (closes TUI-003+TUI-005 integration gap).
2. `WF-TUI-GRAPH` — bounded TUI nodes/edges graph view over the
   delegation/fork graph.
3. `WF-WEB-CANVAS` — React interactive canvas on daemon events
   (nodes = sessions/subagents, edges = parent→child).
4. `WF-CREATE` — workflow-as-skill schema + TUI/web creation surface.

---

## Honest bottom line

- **Parity bar (Section 1):** core engine (daemon, sessions, storage, providers,
  tools, security, sync, protocols) is strong and test-backed; the **biggest
  remaining parity gaps are the fully native TUI render, live subagent/LSP/MCP
  wiring, themes, AGENTS.md/rules system-prompt injection, custom
  agents/commands loading, and the installed-app golden journey**.
- **Extras (Section 2):** our genuinely-new work (agentic loop hardening,
  fleet-claim ledger, worker protocol, doctor, native lane set, remote device
  lanes) is completed and tested; remote/mobile/tunnel are the large
  unfinished fronts.
- **Roadmap (Section 3):** seven committed beyond-OpenCode features (LOOP,
  ULTRA mode, /CONTEXT, /MEMORY, rules-folder globs, CI/CD non-interactive
  CLI, and the workflow views 3.7). None has a claimed lane yet;
  rules-folder globs (3.5) doubles as the fix for the Section 1.7
  rules-injection parity gap, CI/CD (3.6) completes the verified headless
  core, and **workflow views (3.7) are the highest-visibility UX gap —
  timeline data model is done and needs the native render, canvas and
  workflow-creation need new lanes**.
- This file is a raw inventory generated from repo evidence on 2026-09-18 at
  `17b15e6`. It is **not** the FEATURES.md acceptance record and must never be
  cited as acceptance evidence.
