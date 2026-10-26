# OpenCode RK Web: ChatGPT-style parity and delivery checklist

**Updated:** 2026-10-26. **Active worktree:** `/Users/mymac/Projects/opencode-rk-web-parity-20261026`, branch `feat/web-parity-restoration-20261026`. **Integration target:** `/Users/mymac/Projects/opencode-rk-recovery-20261005` (`main-v2`). Do not overwrite that shared working tree without resolving concurrent edits.

This is the source-of-truth checklist for the **web** experience, not a claim that
every listed feature has shipped. The Rust daemon owns authentication, tool
authorization, provider credentials, shell policy, subagent runs, and durable
conversation state. The browser MUST NOT reimplement or bypass those controls.

## What this implementation actually provides

- [x] Chat-oriented responsive shell, persisted conversation history, session
      actions, workspaces, model selection and streaming turns (existing app).
- [x] Safe rich message rendering: headings, lists, paragraphs, inline
      emphasis/links/code, fenced code with copy, tables, quotes and rules.
      HTML is always displayed as inert text. This is **not full CommonMark**
      or LaTeX support.
- [x] Live structured tool-call and tool-output cards (tool name, arguments,
      running/completed states, outputs). Tool-role messages saved by the daemon
      have expandable output cards.
- [x] Paginated durable typed tool-call/output records after reload, including
      call IDs, arguments, success/failure and bounded output. The storage
      layer now hydrates tools for **the selected page**, not an unrelated
      first/last message range.
- [x] Autocomplete for configured `/commands` using authenticated
      `GET /api/commands`, with actual backend execution via
      `POST /api/sessions/:id/command`. Unknown commands fail visibly; never
      degrade silently to plain chat.
- [x] `@agent` discovery from the same daemon inventory, plus server-routed
      top-of-message agent turns via `POST /api/sessions/:id/agent/stream`.
      The server resolves prompts/tool restrictions, not user-provided system
      prompts. Example: `@reviewer inspect src/App.tsx`. Inline mentions in
      arbitrary positions remain plain message text (see backlog).
- [x] Live subagent panel showing child state, model, role, generation, output
      preview, errors, approval wait and queued follow-ups; direct stop, retry,
      follow-up and transcript navigation use existing child endpoints.
- [x] Settings dialog with appearance, general help, daemon model catalog and
      capability inventory; six themes, compact/comfortable display, text
      scale and reduced-motion preference persisted in local storage.
- [x] Rust daemon browser-resume authorization permits **GET-only**
      `/api/commands`; all POST actions remain authenticated and
      origin-bound. No provider secrets are sent to the browser.
- [x] Browser authentication test fixtures updated to use the newer
      native-token-to-browser-capability exchange without weakening production
      security.
- [x] Nine workspace views: Projects & read-only Git worktrees, Files & Git
      diff, native foreground shell, Agents, Models, Providers, MCP servers,
      Permission Rules and durable session Tasks.
- [x] Read-only file lookup for `@file` suggestions: bounded traversal
      within the selected session directory, no symlinks, blocklisted secrets,
      bounded previews. The selected file path is inserted into the prompt,
      not automatically read into provider context.
- [x] Workspace pages are code-split from the main conversation bundle.
- [x] Origin-bound native global search across local conversation titles and
      inline user/assistant messages, with current-session file paths and
      advertised agents included in the keyboard-accessible Ctrl/Cmd+K panel.
      Search results are rendered as inert text; binary/blob content is not indexed.

## P0 — Correctness and security gates

- [x] Read existing `main-v2` Git state; preserve uncommitted work and
      concurrent integration commits; fetch upstream and fast-forward only
      when appropriate.
- [x] Require the native daemon for command discovery and execution.
- [x] Resolve `@agent` roles on the server. Enforce built-in read-only
      restrictions when configuring tool schemas.
- [x] Reject unknown command/agent names and unknown role permissions.
- [x] Keep privileged command execution out of the frontend process.
- [x] Preserve bounded streamed NDJSON parsing and tool-event validation.
- [x] Rust file traversal/symlink/secret restrictions with unit tests; Git
      status and diff use fixed argv, no shell, disabled external diff/textconv,
      bounded streaming stdout and short timeouts.
- [ ] Add backend integration tests for the new agent role route (unknown,
      disabled, read-only, configured model, per-session isolation).
- [ ] Test configured-command discovery across multiple repo/workspace roots.
- [ ] Confirm per-agent permission and custom command authorization parity
      with native CLI on a real browser/daemon pair.
- [ ] Audit all shell actions against `rm -rf /`, `rm -rf .`, symlink traversal,
      `.env` secrets and command injection policies.
- [ ] Security review for HTML/Markdown links and downloadable artifacts.

## P1 — Conversation surface

- [x] User, assistant and system text messages through persisted daemon history.
- [x] Streaming assistant text plus persisted message reconciliation.
- [x] Expandable tool calls, output and shell-like stdout blocks during streaming.
- [x] Markdown headings, lists, code blocks, links, emphasis and basic tables.
- [x] Edit/retry/regenerate via existing branch-based conversation actions.
- [x] Bounded older-history pagination.
- [x] Durable structured call ID, args, success/failure, and output chronology
      in paginated history.
- [ ] Elapsed duration, process exit codes, approval-event timeline and native
      event reconstruction spanning daemon restart.
- [ ] Different renderers for file writes, patches, Git diffs, searches,
      tasks, MCP tools and nested subagent tool events.
- [ ] Collapsible thinking/reasoning traces with provider-specific redaction.
- [ ] Syntax highlighting, Git diff highlighting and downloadable terminal logs.
- [ ] Full CommonMark/GFM including task lists, nested tables and math.
- [ ] In-message images, audio, video and rich structured results via safe,
      capability-gated UI; optional OpenUI-inspired result renderers.
- [ ] Stream reconnect/replay, stop/continue semantics and partial-generation
      state across page refresh.
- [ ] Virtualized 10k+ messages with preserved scroll position/anchors.
- [ ] End-to-end mobile reading and accessibility screen-reader coverage.

## P2 — Composer and contextual actions

- [x] Persisted per-session draft, safe paste, multiline editor, model and
      reasoning selectors (existing app).
- [x] Keyboard-accessible `/` and `@` suggestions from daemon inventory.
- [x] Real command POST and leading `@agent` role routing.
- [x] Scoped `@file` path suggestions from the bounded, authenticated
      session lookup endpoint; files are never silently injected into context.
- [ ] `@folder`, `@symbol`, Git-ignore-aware indexing and permission-gated
      file content attachments.
- [ ] Mention chips whose identity survives editing, copying and pasting.
- [ ] Agent mentions anywhere in the prompt with explicit behavior for
      delegation versus role selection, not just leading-role syntax.
- [ ] Queue prompts and steer a running orchestrator **without stopping** it.
- [ ] Drag/drop attachments that are actually sent to capable providers.
- [ ] Full slash command argument completion and per-workspace inventories.
- [ ] Multimodal uploads, voice and clipboard file previews.
- [ ] Keyboard shortcut guide, command palette and configurable shortcuts.

## P3 — Agent orchestration panel

- [x] Poll native child registry for live statuses and output previews.
- [x] Open transcript, stop, retry where permitted, and send queued follow-ups.
- [ ] Nested parent/child tree for 2+ levels of subagents with stable ordering.
      The current Rust turn service explicitly disables the `task` tool for
      child sessions; genuine deeper delegation needs a backend change first.
- [ ] Live per-agent tool chronology and execution cost/tokens/time.
- [ ] Cross-session child discovery and history after daemon restart.
- [ ] Parallel agent launch/control (with consent/permissions and safety caps).
- [ ] User and orchestrator message queue, steering, permissions and handback UI.
- [ ] Context compaction, retries and automatic recovery visualization.
- [ ] Tree layout and grouped timeline/kanban view with search and filters.

## P4 — Application pages and navigation

- [x] Chats, conversation search, workspace picker, artifacts and attachments
      already present in the legacy web app.
- [x] Settings dialog and agent side panel added.
- [x] Projects / Repositories / Worktrees viewer with read-only native status,
      branches and worktree paths for the selected session.
- [x] Session-rooted file explorer and preview + bounded read-only Git diff.
- [ ] Writable file editor, patch review, staging and per-write native approval.
- [x] Native Shell console for explicit foreground commands, live output and
      stop/cancel through the native permission broker.
- [ ] Durable background Jobs page with restart recovery and log replay.
- [x] Agents catalog, advertised descriptions/roles and live subagent entrypoint.
- [ ] Full agent configuration editor and execution analytics.
- [x] Models and native custom Providers page with write-only credential input.
- [x] MCP servers status, disabled registration and confirmation-gated connect/disconnect.
- [ ] Remote MCP, catalog discovery and restart-persistent server configuration.
- [x] Permission Rules page with explicit acknowledgement before allow.
- [x] Session Tasks / TODO list with durable native status and priority updates.
- [ ] Cross-session scheduled runs, OS notifications and recurring tasks.
- [x] Unified native search across conversation titles, inline messages,
      agents and **current-session** file names, with accessible keyboard shortcut.
- [ ] Global file search across every registered workspace, blob-backed message
      indexing, search hit jumping/scroll anchoring, and FTS-backed scaling.
- [ ] Real route-based navigation, deep links, breadcrumbs and browser history.

## P5 — Personalization and visual parity

- [x] Dark, light, system, midnight, ocean and forest themes.
- [x] Display density, text sizing and reduced-motion controls.
- [x] Responsive panel/dialog foundations and keyboard focus handling.
- [x] Workspace hub is responsive and lazy-loaded; settings dialog traps
      keyboard focus and restores focus on close.
- [ ] Complete CSS variable/token audit: eliminate dark-only hard-coded colors
      in all legacy components (light mode is not yet pixel-perfect).
- [ ] Full keyboard/focus-trap semantics for dialog and suggestion listbox.
- [ ] Persist layout, sidebar width, tool-card collapse preferences.
- [ ] Accessible high-contrast theme, color-blind palettes, typography presets.
- [ ] Visual regression tests at common desktop/tablet/mobile widths.
- [ ] Complete ChatGPT-style interaction polishing with OpenCode RK branding.

## P6 — Testing and release

- [x] TypeScript compile and Vite production bundle build.
- [x] Existing React accessibility and interaction suite.
- [x] New tests for command/agent endpoint routing, composer suggestions,
      settings, subagent actions and safe markdown/tool rendering.
- [x] Workspace UI tests for projects, files, diffs, providers, tasks, MCP,
      rules and shell execution, plus durable typed history pagination tests.
- [x] Node-backed fragmented tool NDJSON tests using browser-resume exchange.
- [x] `cargo check -p opencode-rk-server --offline` after server changes.
- [ ] Real-daemon E2E with configured commands and an actual child task.
- [ ] Browser screenshot/interaction walkthrough: Chrome, Safari, Firefox.
- [ ] Nontrivial responsive/zoom/keyboard/screen-reader manual acceptance.
- [ ] Load test: 50+ concurrent agents and long conversations.
- [ ] Reconnection, daemon restart and permission denial integration tests.
- [ ] Production release packaging and smoke test of embedded assets.
- [ ] Rollout, telemetry/privacy review and user documentation.

## Wire contracts implemented now

| Feature | HTTP route | Browser policy |
| --- | --- | --- |
| Commands / agent inventory | `GET /api/commands` | Authenticated, read-only, browser-resume admitted |
| Configured command | `POST /api/sessions/:id/command` | Authenticated, server-resolved command |
| Leading `@agent` turn | `POST /api/sessions/:id/agent/stream` | Authenticated, server-owned prompt and tool restrictions |
| Agent child status | `GET /api/sessions/:id/children` | Authenticated |
| Agent stop / retry / follow-up | `POST /api/sessions/:id/children/:child/{stop,retry,continue}` | Authenticated, server state machine enforces limits |
| Standard streaming | `POST /api/sessions/:id/turns/stream` | Authenticated, bounded NDJSON |
| Session file listing + preview | `GET /api/sessions/:id/files?path=...` | Authenticated, read-only, bounded, secret-filtered |
| `@file` lookup | `GET /api/sessions/:id/files/lookup?q=...` | Authenticated, read-only, bounded and symlink-safe |
| Git status + worktrees | `GET /api/sessions/:id/git` | Authenticated, fixed-argv read-only Git |
| File Git diff | `GET /api/sessions/:id/git/diff?path=...` | Authenticated, no external diff/textconv, bounded |
| Native shell | `POST /api/sessions/:id/shell/stream` | Authenticated, explicit command via native broker |
| Durable tasks | `GET/PUT /api/sessions/:id/todo` | Authenticated native storage validation |
| Providers | `GET /api/providers`, `POST /api/providers/:id` | Authenticated, key never read back |
| MCP lifecycle | `GET/POST /api/mcp`, `POST /api/mcp/:id/{connect,disconnect}` | Authenticated, connect requires UI confirmation |
| Permission rules | `GET/POST /api/permission-rules` | Authenticated origin-bound preference update |
| Global search | `GET /api/search?q=...` | Authenticated browser-resume read; literal match, max 25 sessions/50 messages |

## Release checks

```sh
cd /Users/mymac/Projects/opencode-rk-web-parity-20261026/web
pnpm install --frozen-lockfile
pnpm typecheck
pnpm test
pnpm build

cd ..
cargo check -p opencode-rk-server --offline
cargo test -p opencode-rk-storage --test display_tool_pagination --offline
cargo test -p opencode-rk-server --lib --offline
```

The web `pnpm build` embeds bundles into `crates/server/web_dist`, but
**the Rust executable must be rebuilt and restarted** before a running
native daemon serves those new assets. Do not call the web app deployed or
fully verified merely because Vite built successfully.

## Behavior and scope notes

1. `/review inspect files` selects a configured command; it is not equivalent
   to asking the LLM to interpret arbitrary slash-looking text.
2. `@reviewer inspect files` activates a server-owned role on the current turn.
   A native delegated child remains a separate lifecycle object surfaced in
   the Subagents panel. These are distinct concepts.
3. `/api/commands` requires a sufficiently recent daemon; the UI remains
   usable if its capability inventory cannot be loaded but does not pretend
   missing commands are active.
4. Slash menu suggestions and child polling are best-effort, bounded and
   do not replace the daemon's source of truth or access control.
5. Shell and tools may require native runtime feature flags, provider support
   and interactive permissions; browser UI rendering does not turn them on.
6. `@src/App.tsx` is a searchable **path reference**, not automatically an
   attachment or embedded file content. Agents still require an authorized
   native read tool to inspect it.
7. The Shell view is a **foreground** native session stream, not a detached
   job scheduler. Read-only Git status/diff pages do not stage or modify files.
8. Saved provider API keys are accepted only by the Rust configuration API
   and cleared from the browser form on success. MCP connections launch
   configured local processes and therefore require an explicit second click.
9. The Git diff viewer currently shows unstaged changes in a single tracked
   file, not staged changes, untracked contents, or a patch-apply workflow.
10. The production JS chat bundle still triggers Vite's 500 KiB chunk
    warning. Workspace, settings and subagent panels are code-split; further
    vendor/UI chunking and visual-browser QA remain release gates.
