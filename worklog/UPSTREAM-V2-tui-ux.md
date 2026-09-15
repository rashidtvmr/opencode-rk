# OpenCode V2 upstream research: TUI and UX

Source: `/home/rashid/projects/tmpcodes/opencode`
Observed commit: `07619a09d6da7945ea3fdbb4f8ae5ce8dc2b6eeb`.

## TUI architecture

The TUI is under `packages/opencode/src/cli/cmd/tui/` and uses SolidJS with
`@opentui/solid`/`@opentui/core`. `app.tsx` composes route, SDK, sync, KV,
theme, keybind, dialog, toast, exit, and project providers. Mouse is enabled by
default and can be disabled by `OPENCODE_DISABLE_MOUSE` or `mouse:false`.
`context/sdk.tsx:20-58` subscribes to SSE and batches event updates into one
render. Exit is idempotent in `context/exit.tsx:15-50`.

## Layout and panel behavior

- `routes/session/index.tsx:155-175` uses a 42-column sidebar when width is
  greater than 120 or the user explicitly opens it. Content width subtracts the
  sidebar and margins.
- `routes/session/index.tsx:2054,2107` uses split layouts above 120 columns;
  permission UI becomes unified below that width and narrow below 80.
- `routes/session/sidebar.tsx` provides the right-side 42-column plugin-slot
  sidebar. It is not a first-class metadata/MCP manager.
- `ui/dialog.tsx:22-24,144` uses fixed widths of 60/88/116; narrow overflow is
  a parity gap that the Rust UI should handle more safely.

## Prompt, completion, dialogs, and keybindings

- `component/prompt/autocomplete.tsx` supports `@` and `/` completion with
  frecency/fuzzysort, max height 10, keyboard and mouse acceptance.
- History/stash use JSONL persistence. Tag completion queries files and limits
  results to five. Stash deletion requires a second confirmation key.
- Command palette and slash commands are implemented by `dialog-command.tsx`
  and `command/index.ts`; keybind defaults live in `context/keybind.tsx` and
  config keybind schema.
- Dialog primitives support select, alert, confirm, prompt, help, theme, model,
  provider, agent, variant, skill, session, workspace, MCP, and status flows.
- Keyboard access includes arrows/return/Escape, permission allow/always/reject,
  question digits/tabs, confirmation left/right, sidebar toggle, and fullscreen
  permission view. Mouse can be disabled globally.

## `/cd` and requested metadata panel

There is no dedicated `/cd` command in the upstream TUI. The only `cd` match is
the shell command allowlist in `packages/opencode/src/tool/bash.ts:26`.
Therefore `/cd` remains a new feature in the Rust backlog, not upstream parity.

Upstream has footer/sidebar metadata and a read-only `dialog-status.tsx`, but no
dedicated right-side all-stats panel with the REQ-042 contract. MCP UI consists
of `component/dialog-mcp.tsx` (toggle/connect/disconnect/status refresh) and
`feature-plugins/sidebar/mcp.tsx` (status dots/counts/collapse). It has no catalog
search/install, bulk selection, separate checkbox/power controls, lifecycle
persistence, or invocation-time disabled payload filter.

## MCP/UI state and persistence

`context/local.tsx:371-387` derives enabled status from connected MCP state.
`routes/session/footer.tsx` and feature sidebar plugins show directory, LSP, MCP,
and version metadata. KV JSON persists sidebar mode and animation preferences;
model JSON persists recent model/agent/variant. There is no upstream MCP
lifecycle persistence equivalent to local TOOL-018.

## Themes, accessibility, narrow behavior, and failures

`context/theme.tsx` supports bundled/custom/system themes and refresh. Mouse and
keyboard paths coexist. Terminal color detection has a one-second fallback;
clipboard has OSC52 and platform fallbacks. Startup loading debounces 500 ms and
holds for 3 seconds. Fatal errors destroy the renderer, flush input, copy a
bounded issue report, and exit. Workspace session creation retries 5xx, but the
retry behavior needs a bounded local equivalent.

## Tests and local baseline

Observed tests include keybind, CLI TUI plugin/slot/theme/sync, prompt-part, and
TUI config tests. No complete interaction snapshot suite was identified.
Local `crates/cli/src/main.rs:26-80` has Doctor/Session/Models/Serve/Web and no
native interactive TUI. Local `crates/sessions/src/tui_state.rs:9-20` has bounded
pure state machines for queued drafts, switchers, sources, memory files, and
hints, but not the upstream renderer.

## Checklist

- [x] App/provider/render/event lifecycle
- [x] Home/session layout, sidebar thresholds, dialogs, palette, slash commands
- [x] Prompt completion, history, stash, selection, clipboard, editor
- [x] Permission/question interactions and narrow layout
- [x] Themes, mouse opt-out, keyboard paths, startup/error/exit behavior
- [x] MCP toggle/status/sidebar behavior
- [x] `/cd` checked and absent upstream
- [x] Right-side all-stats panel checked and absent as a dedicated surface
- [ ] Full upstream TUI interaction test inventory
