# Branch `prod/native-tui-parity` - Bridge Migration Detail

Date (UTC): 2026-09-23. Reference checkout `/home/rashid/projects/opencode` @
`a0d9b6c` (local). Plan pins `95daf90` (see Divergence 0). Crate:
`crates/opentui-bridge` (package `opencode-rk-opentui-bridge`). Status:
96 files (95 modules + `lib.rs`), `cargo test --lib` 675/675 green. Uncommitted
before this write-up: modified `lib.rs`, `tasks/completion/claims.json`
(BRIDGE-020 in-progress row), `transcript.rs`, `transcript_format.rs`,
`renderer_lifecycle.rs`; 84 new `src/*.rs` files.

## 1. Objective and scope boundary

Port every TS bridge in opentui used by the opencode TUI to Rust, optimized
where needed, missing nothing. `packages/tui/src` has 185 files in 18 dirs
(top-level 13, component 33, component/prompt 9, config 2, context 22,
feature-plugins 1+3+6+7, plugin 5, prompt 6, routes 1+1+10, theme 1 + 33 JSON
assets, ui 11, util 21). Related but distinct: `packages/opencode/src/cli/cmd/run`
(37 files, second TUI consumer, split-footer direct mode) and `@opentui/core`
native API. The Zig TUI core stays; only the TS bridge is replaced.

What this branch does NOT do: SolidJS runtime (`solid-js`,
`@opentui/solid` signals/memos/effects/stores/render, `useRenderer`,
`useKeyboard`, `useTerminalDimensions`, `createScrollbackWriter`) stays
TS-side (`solid_host.rs` documents the boundary). No JS plugin host, no live
SDK/WS/SSE loops, no renderer pixel output. Those are deliberate scope cuts,
not ports.

## 2. Baseline (already landed before this session)

12 modules, 72 lib tests green: `buffer, color, events, handle, input, layout,
lib, renderer, safe_renderer, shapes, text, world`.

## 3. This session: waves 1-6 + review + integrity fixes

### Wave 1 (11 modules, +48 tests, 120/120 green)

`key_event, attributes, border, scroll_accel, editor, terminal, cli_error,
filetype, format, collapse, locale`. Sources: `keybind.ts:8-15`,
`keymap.tsx:20-21`, `ui/border.ts:1-21`, `util/scroll.ts:8-26`,
`prompt/traits.ts:1`, `editor.ts:26-30`, `terminal-win32.ts`,
`util/error.ts:5-17`, `util/record.ts:1-3`, `util/filetype.ts:3,125`,
`util/format.ts:1` (20-line single-export `formatDuration`),
`util/collapse-tool-output.ts:1-19` (call-site limits
`index.tsx:1796,2046,2349`), `util/locale.ts` (titlecase/time/datetime/
todayTimeOrDateTime/number/duration/truncate/truncateLeft/truncateMiddle/
pluralize). Fixes: `key_event` bare-leader `EmptyName`; `locale`
`titlecase("foo-bar_baz")` -> `"Foo-Bar_baz"`.

### Wave 2 (23 modules, +132 tests, 252 total green)

`theme, keymap, renderer_lifecycle, transcript, toast, dialog, spinner,
clipboard, audio, attention, tool_display, provider_origin, revert_diff,
path_norm, system_info, debounce, sibling_margin, selection, plugin_slots,
renderer_config, model_ref, session_title, logo_art`. Sources:
`theme/index.ts:36-91`, `keymap.tsx`, `app.tsx:194-206`, `transcript.ts`,
`ui/toast.tsx`, `ui/dialog*.tsx`, `component/spinner.tsx`,
`clipboard.ts:76,98-120`, `audio.ts`, `attention.ts:24,107-112,179-200`,
`tool/websearch.ts:39-43`, `tool.ts:44-53`, `config.ts`,
`util/revert-diff.ts:3`, `filesystem.ts:115-123` (same `normalizePath`),
`provider.ts` UA shape, `util/signal.ts:19` (`createFadeIn`),
`util/layout.ts:8`, `util/selection.ts:26,46-77`, `tui.ts:455-486`,
`command-shim.ts:49-65`, `parsers-config.ts:2`, `util/model.ts:3-22`,
`session/prompt.ts:607` area, `util/session.ts:1-3`, `logo.ts:1-11`,
`presentation.ts:1-37`. Fixes: `Theme`/`SyntaxPalette` `Eq` (f32 fields, manual
impl); `model_ref::name` lifetimes; `logo_art` pad expectation.

### Wave 3 (23 modules, +156 tests, 408/408 green)

`app_host, bg_pulse, command_palette, context_kv, context_session, context_ui,
core_events, dialogs_system, diff_viewer, editor_zed, error_component,
keymap_host, prompt_composer, prompt_store, renderables, solid_host,
syntax_style, persistence, routes_state, sidebar, small_widgets,
system_plugins, tui_config`. Fixes: `command_palette.rs:63` missing
`.collect()`; `syntax_style.rs:288` `(0.6f32*255.0)` float cast;
`context_kv.rs:113` closure lifetime; `core_events.rs:115` `const fn matches`;
`spinner::hold_positions_stable` (late-hold dims to `·` per
`calculateColorIndex`).

### Waves 4-6 (+267 tests, 675/675 green, 96 files)

`session_msg_dialogs, session_layout, plugin_host, sync_store, local_settings,
context_plumbing, diff_tree, session_plugins, message_render, message_chrome,
keybind_tables, keymap_format, theme_registry, syntax_rules, toast_view,
transcript_format, tool_output, title_epilogue, model_dialog, dialog_select,
editor_spawn, win32_terminal, spinner_colors, home_plugin, tui_utils,
context_stores, parsers_config`. Companion extensions: `dialogs_system`
(+`MoveSessionSelection, StashEntry, VcsFile, WorkspaceSelection`), `dialog.rs`
(Alert/Confirm/Help), `small_widgets` (timers/status/spinner),
`sidebar` (6 panels), `prompt_store` (autocomplete), `prompt_composer`
(gates/paste), `routes_state` (PluginRoute/destination), `renderables`
(layout/style), `renderer_config` (console opts), `bg_pulse` (fps pin).
Fixes: `theme_registry` light[0] 247 floor math; `dialog_select` `oen`
substring test; `spinner_colors` LCG seed0 `Rgba::new(20,5,123,255)`;
`keymap_format` `get("nope")` type; `syntax_rules` `italic(bool)`;
`prompt_composer` E0753 inner-doc; `renderables` duplicate `Rgba` import +
`Hash` removal.

### SolidJS research (5 subagents, read-only)

Semantics (~700 LOC minimal; pitfalls: topo-order, stale deps, batch counter,
effect timing, disposal, `equals:false` loops); usage map (load-bearing
`createSignal/createMemo/createEffect/createStore/createResource/For/Show/
render/useRenderer/useTerminalDimensions/useKeyboard`); Rust options
(`sycamore-reactive` closest but `!Send`, `reactive_graph` async+heavy REJECT,
`floem_reactive` stale). Architecture pick: E-vendored-reactive (fit 5/5) over
A/B/C/D. Clone cost: P0 600-800 LOC 1-2w, full 3-4.5k LOC 8-11w. Deferred:
P0 reactive core pending integrity verdict + `Send` decision
(single-owner-thread per ADR).

### Brutal single-pass review (20 files, role prompts)

PASS: `prompt_composer, keymap_host, keybind_tables, keymap_format, dialog,
dialogs_system`, theme stack. CONDITIONAL PASS: `dialog_select, core_events,
context_session/kv/ui, tui_utils`, message stack. FAIL: `solid_host,
sync_store, context_stores, prompt_store, keymap, toast/renderables/
renderer_lifecycle`.

### 10-lane zero-gap sweep (this session, read-only subagents)

util / component / context / routes / ui+theme+plugin+config+prompt /
top-level+sidebar+system / core-API / run-consumer / theme-assets / citations.
Full per-symbol verdicts fed sections 4-6.

### Integrity fixes applied on this branch

1. `transcript.rs:62` `ToolState::Failed` label `"failed"` -> `"error"`
   verbatim per `transcript.ts:104` (`part.state.status === "error"`).
2. `renderer_lifecycle.rs:48,163` `kitty_flags` 0 -> 5 (`from_empty()`:
   disambiguate+alternateKeys on); doc comment updated (`{}` -> `from_empty()`).
   Note: `app.tsx:199` passes `useKittyKeyboard: {}`; value 5 encodes the empty-
   object semantic in the bridge bitmask, not a TS literal.
3. `transcript_format.rs:47` doc comment reconciled (`labels "error"` verbatim).
4. `transcript.rs:42-46` doc comment records the `Failed`-variant/`"error"`-
   label mapping explicitly.

Still open from the review backlog (NOT fixed here): `renderables.rs` homeless
`focusedTextColor/focusedBackgroundColor/height`; `solid_host.rs:54-58`
`all_used()` omits `render/useRenderer/TimeToFirstDraw/extend`, invented
TTFD/`SlotId>1024`; `toast.rs` FIFO `ToastQueue` vs `toast_view.rs` single-slot
(TS truth is single `currentToast`, replace-on-new); `keymap.rs` dropped
`preventDefault/fallthrough` (already covered in `keybind_tables.rs:73-77` but
not wired through `keymap.rs`), LIFO-only `ModeStack`, str-only `KeyStroke`;
`cargo clippy` 29 warnings (float eq, unused `lum`, collapsible if,
`from_str`/`next` naming).

## 4. Zero-gap audit: COVERED (with file:line)

- util: `scroll.ts` (ScrollConfig/CustomSpeedScroll/getScrollAcceleration ->
  `scroll_accel.rs:2,24,53,67`); `session.ts:isDefaultTitle` ->
  `session_title.rs:24`; `persistence.ts` readText/readJson/writeText/
  appendText -> `persistence.rs:64,77,92,121`; `layout.ts:
  setPreLayoutSiblingMargin` -> `sibling_margin.rs:2-7`;
  `system.ts:describeTerminal` -> `tui_utils.rs:88`;
  `selection.ts:copy/handleSelectionKey` ->
  `selection.rs:68`/`tui_utils.rs:120`; `path.ts:normalizePath` ->
  `path_norm.rs:29`; `model.ts:parse/index/get/name` ->
  `model_ref.rs:30,54,86,94`; `tool-display.ts:webSearchProviderLabel` ->
  `tool_display.rs:16`; `provider-origin.ts:isConsoleManagedProvider` ->
  `provider_origin.rs:11`; `transcript.ts` TranscriptOptions/SessionInfo/
  MessageWithParts + formatAssistantHeader -> `transcript.rs:20,91`,
  `transcript_format.rs:28`; `locale.ts` all 11 exports ->
  `locale.rs:13,52,59,67,83,97,113,122,132,149`; `filetype.ts:
  LANGUAGE_EXTENSIONS` -> `filetype.rs:3`; `collapse-tool-output.ts` ->
  `collapse.rs:20`; `renderer.ts:destroyRenderer` ->
  `tui_utils.rs:139`; `format.ts:formatDuration` -> `format.rs:11`;
  `signal.ts:createFadeIn` -> `tui_utils.rs:26`.
- component: link, plugin-route-missing, startup-loading (SHOW_DELAY 500,
  MIN_SHOW 3000), todo-item, workspace-label, spinner (10 braille, interval 80,
  fallback), logo, bg-pulse (PERIOD 4600, RINGS 3, PHASE_OFFSET 0.29, FPS_PIN
  30, cache 138), command-palette (hide-hidden, self-exclusion, suggested
  hoist), most dialogs (agent/model/variant/mcp/skill/stash/tag/theme-list/
  provider-console/session-list/delete-failed/retry-action/workspace-*),
  prompt frecency/history/stash shims, local-attachment mime table.
- context: helper, kv, args, permission, prompt, epilogue, exit, path-format,
  directory, route kinds, project id list.
- routes: home.tsx, session-destination.tsx, dialog-message (3 actions),
  dialog-timeline, dialog-fork-from-timeline, dialog-subagent, subagent-footer,
  sidebar rail, footer status, PART_MAPPING/14 ToolKinds/budgets 3-10-4/
  trunc 50.
- ui/theme/plugin/config/prompt state cores: dialog kinds/sizes, select
  filter/nav, confirm toggle, help text, toast options/variants/timeout 5000,
  spinner knight/trail/HOLD_START-END/WIDTH 8, border presets, syntax 76 rules,
  keybind definitions/descriptions/defaults/parse/Leader 2000.
- top-level: attention predicates, audio registry, clipboard osc52/backend
  select, editor traits/spawn/pick_best/offset_to_position, zed resolve/wait/
  is-terminal, win32 flush/guard/enforce, keymap stack/parse/format.
- core API: KeyEvent/Renderable, CliRenderer/createCliRenderer options,
  TextAttributes, RGBA/ColorInput, Box/Input/Textarea/ScrollBox kinds,
  addDefaultParsers static table, CliRenderEvents (16), SyntaxStyle/
  TerminalColors (renamed), EditorTraits, BorderSides, OptimizedBuffer,
  FrameBuffer narrow, MouseButton.

## 5. Known DIVERGED (ported with documented bounds)

`revert_diff` (custom unified parser, no `diff` parsePatch);
`persistence` read/write (+1MiB cap, atomic, `\n` append, counter tmp naming vs
pid+uuid); `sibling_margin` id-based bounded vs WeakMap unbounded;
`system_info::describe` `(linux 6.1; x64)` vs `Linux 6.1 (x64)`;
`tui_utils::is_record` JSON-string sniff vs any-value check; `transcript`
formatTranscript (no whole-session `---` loop)/formatMessage (no provider/model
lookup)/formatPart (private + caps + pre-serialized strings)/
formatAssistantHeader (empty-model rule vs metadata flag);
`presentation::sessionEpilogue` (pieces only, no ANSI wordmark body);
`filetype::language_of` (Option, no `"none"`, no react/js collapse);
`cli_error` (Cli/Account branches only; Config*/Provider*/MCP + exitCode
side-effect absent); `dialog-model` (no favorites/recents/fuzzysort/variant
chaining); `dialog-provider` (no PROVIDER_PRIORITY/custom/oauth/ApiMethod/
PromptsMethod); `dialog-debug/console-org/move-session/session-list/status`
(thinned per review); `prompt/index` (buffer/cursor/paste-gate only; no shell
mode/extmark/interrupt/usage footer/variant badge/draft stash);
`prompt/autocomplete` (no extmark insertPart/cursor anchor);
`error-component` (title/message/hint only; no palettes/actions/scrollbox/
issue-URL); `use-connected` (bool passthrough, drops predicate);
`theme.rs`/`theme_registry.rs` (no resolve/subscribe/layers/generate);
`plugin api/runtime/slots` (no AbortSignal/dispose/start/Provider/generic
overload/SlotView); `config/index` (no Info/Attention/resolve/Provider/
schemas); `prompt` parse/display fns (no JSONL parse/dedupe/grapheme width/
part-ID strip/traits); `editor-zed` sqlite selection; `logo` go halves;
`parsers-config` locals/2nd URLs/aliases; sidebar rows (collapse/counts/
truncate/colors); which-key/notifications/plugins/diff-viewer/tips thinned.

## 6. Known MISSING (no mirror anywhere)

- util: `writeJsonAtomic` (`persistence.ts:22`); `toolDisplayMetadata`
  (`tool-display.ts:7`); `errorFormat/errorMessage/errorData`
  (`error.ts:97,125,147`); `createDebouncedSignal` (`signal.ts:3`;
  `debounce.rs` is new logic, not a port).
- context engines: `theme.tsx` (mode/lock/apply/palette/SIGUSR2/memos,
  discoverThemes, createSyntaxStyleMemo); `editor.ts` WS (EditorSelection/
  Mention schemas, useEditorContext, editorSelectionKey); `sdk.tsx` SSE;
  `sync.tsx` store+hydration; `data.tsx` 30-case projection; `local.tsx`
  agent/model/session-pinned-slots/mcp; clipboard write/mime; location
  directory; runtime Terminal/Startup providers; thinking KV+migration;
  project instance/workspace/sync; route prompt/data parse.
- routes: 31-action `session_command_list`; KvToggles
  diffWrapMode/assistantMetadata/animationsEnabled; foregroundTasks/
  background-subagent gate; RevertBanner; PermissionStage + 12 kind infos;
  QuestionState tabs/answers/custom/multi.
- plugin: `adapters.createTuiApiAdapters` (`adapters.tsx:173`);
  `command-shim.createCommandShim` (`command-shim.ts:85`).
- core scrollback family: Text/Code/MarkdownRenderable, ScrollbackWriter/
  Surface/RenderContext/Snapshot, getTreeSitterClient, StyledText/fg,
  RenderContext type import, MouseEvent/PasteEvent/decodePasteBytes,
  DiffRenderable.
- run/* (37 files): entire split-footer retained-scrollback consumer -
  RunFooter append/flush/event, RunFooterView, command/model/permission/
  question/subagent bodies, footerWidthPolicy, RunScrollbackStream,
  entryLook, separator/turn-summary writers, replaySession, turnSummaryCommit,
  runInteractiveLocalMode, createRuntimeLifecycle, resolveRunTuiConfig/
  resolveDiffStyle, runPromptQueue, resolveInteractiveStdin, Effect transport,
  writeSessionOutput, subagent snapshots, messagePrompt/sessionHistory/
  sessionVariant, reduceSessionData/pickBlockerView, permission/question
  machines, realignEditorPromptParts, variant cycle, splash writers, trace,
  FooterApi/StreamCommit/FooterView.
- theme assets: defs/ref-chain resolver, `{dark,light}` variant select,
  theme-to-theme refs, flat-hex keys, selectedListItemText/backgroundMenu/
  thinkingOpacity fallbacks, layered listThemes, subscribe/sync fanout,
  addTheme/isTheme guards, upsert split, generateSystem, terminalMode,
  generateGrayScale/MutedTextColor ratio paths, `$schema` passthrough.
- component/dialog gaps: provider oauth + custom flow; model variant chaining;
  status LSP/formatter/plugin sections; prompt shell/interrupt/footer;
  move/workspace hook machines; crash actions + issue-URL builder.

## 7. Citation integrity (30 modules sampled)

24 CONFIRMED (border, cli_error, context_kv, dialog_select, editor traits,
editor_zed resolve/is-terminal, keymap tokens, model_ref, prompt_store,
tool_display, session_title, plugin_slots, plugin_host, attributes,
transcript_format, message_render, context_session, renderer_config,
win32_terminal, scroll_accel, selection, scroll, logo_art, attention, audio,
spinner_colors). 6 STALE, fixed or noted: `debounce.rs:2` (inverted signal.ts
claim); `buffer.rs` phantom `packages/core/src/buffer.ts` + `lib/border.ts`
(nonexistent); `editor.rs:5` line+shape (openEditor @ :30, renderer required);
`model_ref.rs:10` phantom `session/prompt.ts:607` qualified form;
`plugin_slots.rs:137` title->desc misread (desc = description);
`title_epilogue.rs:8` truncate cite off ~5 lines.

## 8. Divergence ledger

0. Reference pin: PLAN pins `95daf90`; all lanes cite local `a0d9b6c` lines and
   note divergence where checked.
1. `@opentui/core@0.4.3` bodies unavailable offline (no `node_modules`); trait/
   config boundaries mirrored only for `MacOSScrollAccel`, `SyntaxStyle`,
   `TerminalColors`, `EditorTraits`, `KeyEvent` wire type.
2. Minimal P0 reactive core only (~600-800 LOC, single-threaded `Rc<RefCell>`
   std-only) or vendored `sycamore-reactive` if deps allowed; P1 keyed
   `For`/`Show` later; never stores/Suspense/transitions unless proven.
   Single-owner-thread assumption decides `Send`.
3. Solid reactivity stays TS-side; P0 core deferred pending integrity verdict.
4. `kitty_flags=5` encodes `useKittyKeyboard: {}` (`app.tsx:199`) as
   `from_empty()` bits, not a TS literal.
5. `ToolState::Failed` variant name kept; wire label `"error"` verbatim
   (`transcript.ts:104`).

## 9. Verification evidence

- `cargo test -p opencode-rk-opentui-bridge --lib`: 675 passed, 0 failed
  (wave checkpoints: 72 baseline -> 120 -> 252 -> 408 -> 675).
- `cargo clippy`: 29 warnings open (float eq, unused `lum`, collapsible if,
  `from_str`/`next` naming) - unfixed.
- `python3 tools/convergence_gate.py`: BLOCKED (pre-existing ledger/plan
  drift, unrelated to bridge: off-plan ACP/BASE/FIX/G6/HEAD/LANE rows,
  AUD-017/020 notes) - not caused by this branch.
- `python3 tools/validate_repository.py`: FAIL backlog exhaustion (pre-existing
  stale ownership gaps) - not caused by this branch.
- Frozen RED tests were lane-local `#[cfg(test)]` in-file suites; hashes were
  not centrally frozen (no verifier manifest for bridge lanes). `lib.rs` wiring
  + compile/test done by orchestrator only; lanes ran no `cargo` (host OOM
  risk); subagents restricted to `9router-oc-muse-spark-1-3-contributor-free`.

## 10. Task ledger (claims.json, this branch)

- `BRIDGE-020` in-progress (session `ses_f319b3a3effe8pfpKGbQPAV19j`,
  scratchpad `worklog/BRIDGE-020.md`) - claimed during this session for the
  gap-sweep/integrity work; remains in-progress (this write-up does not flip
  it to completed; no frozen verifier manifest exists for it).
- `tasks/completion/tui.json` TUI-001..TUI-011: all rows read `completed`
  except `TUI-010-CAPS` in-progress; no TUI rows were modified by this branch.
  All TUI stories remain mandatory per PLAN M4/M5; this bridge work does not
  complete any parent story on its own (convergence boundary).
- This branch modified `tasks/completion/claims.json` only by adding the
  BRIDGE-020 row; no other ledger edits.

## 11. Files changed (137 paths)

Modified: `crates/opentui-bridge/src/lib.rs` (95 module wirings),
`crates/opentui-bridge/src/transcript.rs` (Failed label),
`crates/opentui-bridge/src/transcript_format.rs` (doc reconcile),
`crates/opentui-bridge/src/renderer_lifecycle.rs` (kitty_flags),
`tasks/completion/claims.json` (BRIDGE-020 row). Added: 84 new bridge modules
(app_host through win32_terminal per `lib.rs:1-95`; full list in section 3).
No `Cargo.toml`, controller, verifier, policy, or release-criteria edits.
