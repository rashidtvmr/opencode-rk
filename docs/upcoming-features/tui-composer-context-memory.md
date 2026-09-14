# Scratchpad: TUI composer + context inspector proposal

## Claim
No interactive TUI exists yet: `crates/cli/src/main.rs:1-14` is headless (doctor/session/models/serve only); workspace `Cargo.toml:2-11` has no TUI crate; no ratatui/crossterm/reedline dep. Proposal adds: editor-grade composer (Enter send, Shift+Enter newline, send button via mouse), clickable model/context status items, context detail view, /memory viewer with unload + cache warning. Precedent: claude-code `PromptInput/PromptInput.tsx:984-1105` onSubmit + `utils.ts:18-23` Shift+Enter handling; codex `tui/src/keymap_setup/actions.rs:105` composer submit/queue actions + backtrack restore to composer.

## Source evidence
- opencode-rk `crates/cli/src/main.rs:4` Command enum (Doctor/Session/Models/Serve, no TUI cmd). Workspace `Cargo.toml:21-37` deps (no TUI libs).
- claude-code `src/components/PromptInput/PromptInput.tsx:164,225,984` onSubmit shape; `src/components/PromptInput/utils.ts:18-23` terminal Shift+Enter notes.
- codex `codex-rs/tui/src/keymap_setup/actions.rs:92-105` composer submit/queue keymap actions; `app_backtrack.rs:103-141` restore prompt to composer.
- Owned targets: REQ-016 UI-006/ROUTE-008 (status), REQ-019 UI-009/BASE-006 (settings/themes), REQ-013 UI-005 (steer), REQ-006 SESS-001/017 (history), REQ-018 CAT-004/AGENT-006/UI-007 (effort/model), synthesis #8 (cost counters), #6 (transcript store).

## Observed scenario
User request only; no implementation. Terminal must support: multiline edit, keybinding display, mouse click regions, model switcher, context breakdown, /memory list + unload confirm.

## Target boundary
New TUI crate (e.g. `crates/tui`) over existing SessionService + Catalog + router state. No daemon protocol change. Stories: UI-005/006/009/010 adjacency + SESS-001 read. Propose as UI-family slice(s), not new REQ except /memory-unload warning which touches SEC posture (confirm destructive? No, context-only, so warning not grant).

## Feature breakdown (proposed slices)
1. Composer: multiline editor, Enter=send, Shift+Enter=newline, Ctrl+J fallback, visible Send button (mouse click), draft preserved across interruptions, queue-while-busy. Keymap configurable (codex-style `tui.keymap.composer.submit`).
2. Status bar: clickable `model` (opens switcher filtered by Catalog, shows effort level), clickable `context x%/n` (opens detail view). Counters from synthesis #8.
3. Context detail view: per-source token breakdown (system, history, tools, skills/memory files, cache-read vs fresh), largest blocks top, truncation markers.
4. /memory: lists loaded memory files (path, bytes, tokens, cached flag); unload per file with explicit warning (disturbs prefix cache, may increase cost/latency on next turn); reload path. Read-only inspection first; unload second slice.
5. Keybinding help footer + /keybindings parity (claude has /keybindings; codex has keymap_setup picker).

## Lean/risk notes
- TUI lib choice is the cost driver: ratatui + crossterm is standard, no new runtime; mouse support opt-in. Reject Ink/React (per SYNTHESIS reject list).
- Shift+Enter varies by terminal (claude utils.ts notes); must ship fallback (Ctrl+J) + visible hint + keymap config.
- Send button needs mouse mode; keep keyboard-first, mouse optional, off by default if it complicates parser.
- /memory unload warning is UX copy, not a permission grant (no SEC grant needed); but must state cache-cost consequence honestly.
- Context view reads existing counters; if synthesis #8 counters land first, this is cheap. Order: counters -> composer -> status clicks -> context view -> /memory.

## Tests (proposed, not yet written)
- Composer keymap unit: Enter submits, Shift+Enter inserts newline, Ctrl+J inserts newline, draft survives interrupt.
- Status click regions map to actions (model switcher opens, context view opens).
- Context view renders fixture breakdown with cache flags.
- /memory lists fixtures; unload removes file + shows warning text; re-add restores.
- No-mouse fallback: all actions reachable by keyboard.

## Decisions
- Recommend accepting as UI-family backlog (fits REQ-016/019/013/006/018, no new REQ needed except possibly memory-unload if controller wants explicit story).
- Recommend sequencing after cost counters (synthesis #8) so context view has data.
- Do NOT start implementation here: plan files (ralph.json/tasks) owned by controller; await user approval then file discovery proposal.

## Remaining unknowns
- Which TUI lib approved (ratatui version, mouse policy).
- Whether daemon exposes token/cost counters yet (depends on synthesis #8 landing).
- Memory file source list: which paths count (project CLAUDE.md equivalents? skill files?).
- Exact keymap defaults and vim-mode scope.
