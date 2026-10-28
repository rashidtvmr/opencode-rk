# CLI config + keybinds research
Updated: 2026-10-26T13:27:23Z; role: CLI config researcher; session: ses_e88d55f7dffdly4XpexGhsHhLE

## Goal
Compare upstream `selectedupstreamglobalcli.json` + OPENCODE_CLI_CONFIG_CONTENT behavior
to Rust actual config load. No project-local CLI invented. Produce precedence, accepted
keys, and one concrete missing behavior OR NO_CHANGE.

## Evidence read (file, symbol/line, revision)

### Upstream (TS) - pinned 450e7b92412c780a7b5a75226bb8d49aaca24e22
- packages/cli/src/config/config.ts:26-66
  - File source: `path.join(global.config, "cli.json")` => `$XDG_CONFIG_HOME/opencode/cli.json` or `~/.config/opencode/cli.json`
  - Env source: `process.env.OPENCODE_CLI_CONFIG_CONTENT` parsed via `parseRecord` + `decode` (Info schema)
  - Merge: `merge(migration ?? decode(readJson()), content)` at config.ts:65
  - mergeRecords (config.ts:130-138) deep-merges nested objects; arrays/scalars from env REPLACE file
  - content (env) has HIGHER precedence over file (passed second to merge)
  - update() at config.ts:83-110 also does `merge(config, content)` after write
- packages/cli/src/config/schema.ts:1-18
  - Info = Schema.Struct({ $schema, ...Config.Info.fields })
  - normalizeLegacyTabs: tabs.enabled -> tabs.mode (true=>"on", false=>"off"), deletes enabled
- packages/tui/src/config/index.tsx:69-257 — full Info struct (v1 config is separate)
  - Accepted top-level keys: $schema, theme{name,mode}, keybinds, plugins, leader{timeout},
    scroll{speed,acceleration}, attention{notifications,sound,volume,sound_pack,sounds},
    diffs{source,wrap,tree,single,fullscreen,view}, terminal{title,copy}, prompt{editor,paste,image_preview},
    session{sidebar,scrollbar,thinking,grouping,verbosity,image_preview,tps,markdown,new_location,permissions},
    tabs{mode,enabled,scope,layout,indicators}, mini{thinking,tools,shell_output,turn_summary,footer,splash,work_spinner,mono,replay,replay_limit},
    debug{devtools,timing,turn_tokens}, experimental, animations, highlights, mouse, cursor{style,blinking}
- packages/tui/src/config/keybind.ts:1-120
  - KeybindOverrides is a Struct keyed by every known command ID in Definitions
  - BindingValueSchema: false | "none" | string | KeyStroke{name,ctrl,shift,meta,super,hyper} |
    BindingObject{key,event,preventDefault,fallthrough} | array thereof
  - Definitions: leader (ctrl+x), app.exit, app.clear, app.debug, app.console, app.scrap,
    app.toggle.*, command.palette.show, help.show, docs.open, opencode.settings, server.pair,
    service.restart, location.reload, diff.*, prompt.editor, theme.*, session.sidebar.toggle,
    pane.focus.*, terminal.*, session.*, open.menu, session.tab.* ... (full list in keybind.ts:40-117)
  - leader.timeout at top-level `leader` object (not keybinds.leader)
  - Unknown command IDs are REJECTED by parse() (keybind.ts:342-344: `unknownKeys` throws)
- packages/cli/test/config.test.ts:82-115
  - "merges inline CLI config content over the global config" test:
    - file: tabs.mode=on, keybinds.app.exit=ctrl+q, plugins=[global], animations=true
    - env: tabs.enabled=false (legacy), keybinds.help.show=false, plugins=[inline], animations=false
    - result.loaded.tabs => {mode:off, scope:global}  (env enabled->mode=off, scope from file)
    - result.loaded.keybinds => {app.exit:"ctrl+q", help.show:false}  (merged, file+env)
    - result.loaded.plugins => ["inline"]  (array replaced by env)
    - result.updated => {animations:false, mouse:false}  (file rewritten, env overrides applied)
  - "reads the legacy tabs toggle without rewriting it" test confirms enabled not persisted

### Rust (read-only) - base 7b94da36
- crates/cli/src/tui_entry.rs:3145
  - `let inline_cli = env::var("OPENCODE_CLI_CONFIG_CONTENT").ok();`
  - Passed ONLY to `UiPreferences::tab_orientation(cli_path, inline_cli.as_deref())` (tui_entry.rs:3146-3152)
- crates/cli/src/native_ui_preferences.rs:49-78
  - tab_orientation: reads cli_path then inline (inline overrides file for layout)
  - parse_cli_tab_orientation (native_ui_preferences.rs:701-715): only parses `tabs.layout`
  - NO parsing of keybinds, leader, theme, cursor, animations, mouse, scroll, session, diffs,
    prompt, terminal, attention, mini, debug, experimental, highlights, plugins, or any other
    upstream Info key
- crates/cli/src/native_ui_preferences.rs (tab_config_tests): tests only cover tab layout parsing
  - global_cli_layout_drives_navigation_orientation_without_rewriting_files
  - inline_cli_config_overrides_file_for_tab_layout
  - file_and_inline_config_are_bounded_independently
- crates/cli/src/native_theme_runtime.rs: references tui.json (not cli.json) for theme loading
- crates/sessions/src/tui_state.rs:348-395 (UI-018): footer_hints/keybinding_help are HARDCODED
  - KeyHint values: Enter/Ctrl-J submit, Ctrl-P model switcher, Ctrl-T context, ? help
  - These are NOT read from any cli.json keybinds section
- crates/cli/src/tui_entry.rs:3264-3266, 5432-5440: leader chord hardcoded ctrl+x (ctrl+X arming)
  - No leader.timeout config read (hardcoded default)
- No Rust code references OPENCODE_CLI_CONFIG_CONTENT beyond tab_orientation
- No Rust code parses keybinds.*, leader.*, or any upstream Info field from cli.json

## Findings

### Precedence (CONFIRMED matches upstream)
- Upstream: file (cli.json) -> env (OPENCODE_CLI_CONFIG_CONTENT) overrides; env wins for scalars/arrays, deep-merges objects
- Rust: file (cli.json) -> env (OPENCODE_CLI_CONFIG_CONTENT) overrides; inline overrides file for tabs.layout
- PARITY: precedence order matches (env > file)

### Accepted keys (Rust accepts FAR FEWER than upstream)
- Upstream Info accepts 19 top-level groups with full nested schemas (see index.tsx:69-257)
- Rust native_ui_preferences.rs ONLY parses: tabs.layout (maps to TabOrientation::Horizontal/Vertical)
- Rust does NOT parse: theme, keybinds, leader, plugins, scroll, attention, diffs, terminal, prompt,
  session, tabs (mode/scope/indicators/enabled), mini, debug, experimental, highlights, animations,
  mouse, cursor, $schema

### Keybinds (CONCRETE MISSING BEHAVIOR)
- Upstream keybind.ts:308-322: KeybindOverrides = Struct keyed by every Definitions command ID
  - app.exit, app.debug, app.clear, app.console, command.palette.show, help.show, diff.*,
    prompt.editor, theme.*, session.*.tab.*, etc. (~70 command IDs)
  - BindingValueSchema: false|"none"|string|KeyStroke|BindingObject|array
  - leader.timeout under top-level `leader` object
  - Unknown command IDs rejected by parse()
- Rust: keybindings are ENTIRELY hardcoded
  - footer_hints (tui_state.rs:378): Enter/Ctrl-J/Ctrl-P/Ctrl-T/?
  - keybinding_help (tui_state.rs:386): same
  - Leader chord hardcoded ctrl+x (tui_entry.rs:3264)
  - NO config-driven keybind loading; cli.json keybinds.* section ignored entirely
  - NO `keybinds.disable`/`help.show=false` support

### Legacy tabs normalization (NO_CHANGE - matches)
- Upstream normalizeLegacyTabs (schema.ts:12-17): enabled -> mode, deletes enabled, does NOT rewrite
- Rust parse_cli_tab_orientation (native_ui_preferences.rs:701-715): only reads layout, ignores enabled/mode
  - Does not rewrite file (matches "reads without rewriting")

## Decisions and rationale
- Precedence verified as matching upstream (env > file).
- Keybinds config is the single concrete missing behavior: upstream loads and applies cli.json
  `keybinds.*` overrides + `leader.timeout`; Rust hardcodes all bindings with no config read.
- No project-local CLI settings file is defined upstream (cli.json is global only); Rust matches this.

## Alternatives rejected
- Reading upstream V2 keybind.ts vs V1: V2 (packages/tui/src/config/index.tsx) is the active schema
  via packages/cli/src/config/schema.ts which does `...Config.Info.fields` from `@opencode/tui/config`.

## Verification actually executed
- Verified upstream HEAD == pinned SHA 450e7b92412c780a7b5a75226bb8d49aaca24e22
- Verified Rust base == 7b94da36 (current working tree)
- All source citations from actual file reads; no user config consulted

## Artifacts and hashes
- N/A (read-only research)

## Remaining uncertainties
- Whether Rust intends to implement keybind config loading (design decision, not research gap)

## Verdict: CONCRETE MISSING BEHAVIOR
Rust TUI does NOT load `keybinds.*` overrides or `leader.timeout` from cli.json
(OPENCODE_CLI_CONFIG_CONTENT or global file). Upstream applies them via
KeybindOverrides schema + resolve(). Rust hardcodes bindings in tui_state.rs.
Precedence (env > file) matches. Accepted key set diverges widely (Rust only reads
tabs.layout). No project-local CLI settings file in either.
