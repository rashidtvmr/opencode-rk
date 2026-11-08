# FOOTER-CHROME scratchpad (task-2, lane `recovery/core` @ `57cb5e46`)

Role: expert implementation owner, bottom chrome. Base `57cb5e46fccc0857c130cb435dc19e48590d4c63`
(main-v2 sync point). All edits inside the granted scope: `crates/cli/src/native_view.rs`
(+ its test modules). `native_view_layout.rs` and `native_home.rs` needed **no** changes.

## Runtime evidence reviewed

`/Users/mymac/Projects/oc2-p1-release-20261107/ui-defect-evidence/`:
`p1-ui3-session.txt` (140x40 session), `p1-ui3-busy-1.txt`, `p1-ui3-initial.txt`
(140x40 home), `p1-ui-initial.raw` (80x24 home). Findings confirmed:

- Session idle: the project path painted at rows 38 **and** 39 (col 2); busy: row 38 =
  scanner + `esc interrupt`, row 39 still carried the path.
- Row 39 crammed the hint groups: `0.1.0-alpha.1 shift+tab agentsctrl+p commands`.
- Home 140x40: composer rows 21-26, path+hint row 26, version row 38, band padding rows
  37/39; home 80x24: path row 18 at col 3, version row 22.

## Upstream oracle (pin `450e7b92412c780a7b5a75226bb8d49aaca24e22`, verified `git rev-parse`)

- `packages/tui/src/mini/footer.width.ts:27-120` — `footerStatuslinePolicy`: statusline
  GROUP ids (`spinner,status,escape,queued,subagents,background,agent,model,context,
  provider,cost,menu` — **no directory cell**), `place()` joins groups with
  `separator = " \u00b7 "` (`:63-73`), parts inside a group concatenate.
- `packages/tui/src/mini/footer.view.tsx:973-997` — `#mini-statusline` paints the groups
  left-aligned with a muted separator span between sections (`:990-993`).
- `packages/tui/src/component/prompt/index.tsx:1628-1651` — `footerLocation`: the
  location label exists ONLY while idle (`status() !== "idle"` returns undefined);
  `:1898-1961` — the prompt footer row (location | editor file) is the single directory
  row, inside the prompt component (used by home AND session routes).
- `packages/tui/src/routes/home.tsx:80-108` — home root `alignItems="center"` with TWO
  `flexGrow` spacers (`:89`, `:104`) around the 3-row block + logo + prompt
  (`maxWidth={75}`, `:101`); the `home.footer` band is the non-shrinking sibling
  (`:106-108`) — the band docks at the bottom, the logo+prompt group is vertically
  CENTERED.
- `packages/tui/src/feature-plugins/home/footer.tsx:7-13,82-116` — home footer row:
  Mcp/Plugins/status-slot blocks, `flexGrow` spacer, trailing muted version cell
  (width ≥ 64); band visible at height ≥ 14, padding 1 at height ≥ 16.

## Root causes and fixes (all in `native_view.rs`)

1. **Duplicate path** — `footer_line` (the shared shell footer) painted
   `footer_directory(&self.workspace, …)` as its left term while `composer_footer_row`
   (the `prompt.footer.location` port) already paints the idle directory. Upstream has
   exactly one directory row (prompt footer location, idle-only); the statusline groups
   carry no directory cell. Fix: `footer_line` no longer paints the path; the composer
   footer row keeps it (idle) and the running scanner/interrupt replaces it there.
2. **Crammed keybind cells** — `footer_line` flattened the rail spans with `slot_text`,
   concatenating the `Agents` and `Commands` groups (`shift+tab agentsctrl+p commands`).
   Fix: new `NativeView::footer_statusline_text` groups consecutive spans by
   `RailSegment` and joins groups with upstream's ` · ` separator.
3. **Footer shape** — statusline groups now lead LEFT-aligned (upstream statusline) and
   the version/MCP chrome trails RIGHT (upstream home footer's trailing version cell).
   The `/context` hit rect moved with the label (starts at the 2-cell inset).

## Defects 3 and 4 (home docking / path anchor): upstream-faithful already — DISPUTED ITEM

The task text claims "upstream docks composer+footer to the bottom" for the HOME screen.
The pinned oracle says otherwise: `routes/home.tsx:80-108` centers the logo+prompt group
between two `flexGrow` spacers and docks ONLY the `home.footer` band. oc2 already
implements exactly that — `native_home.rs` (module header + `layout()`/`footer_rows()`)
documents the same port with the Yoga `div_ceil` rounding assumption, and
`docs/ui-parity/PARITY_STATUS.md:731-733` records LAY-18/19/20 as `ALREADY_CORRECT`.
The "12-row dead gap" (home rows 27-37) IS upstream's lower flex spacer; the home path
anchor (col 3 at 80 cols, col 33 at 140) follows the centered prompt block
(`alignItems="center"` + `maxWidth={75}` arithmetic: `pad_x + (inner-75).div_ceil(2)`),
identical to upstream, and is consistent relative to the prompt's own `┃` rail at every
width.

I did NOT rewrite the centered home layout: that would contradict the pinned oracle, the
documented `native_home.rs` model, and the reviewed LAY records, and would require
rewriting semantic home tests (forbidden without an independent contract-evaluator
supersession). Instead I pinned the docking contract with new tests (below). If the lead
wants a deliberate deviation (session-style bottom-docked home composer), that is a
product choice needing explicit authorization + contract-evaluator supersession of the
home geometry tests; the seam would be `native_home::layout` + `NativeView::home_positions`
(`native_view.rs:5694` area) + `home_column`.

## Tests

Updated (task-mandated contract change — single-path bottom chrome; intent preserved):
- `project_directory_and_loader_share_only_the_composer_footer_row` — shell row must NOT
  repeat the directory; bottom chrome paints the path exactly once (idle) / zero times
  (running). The transcript header's workspace row is top chrome, outside this contract.
- `shell_footer_uses_full_padded_row_and_returns_to_original_idle_footer` — directory
  restored in the composer footer row; shell row restores usage chrome without the path.
- `footer_directory_tail_keeps_selected_project_name_visible` — now reads the composer
  footer row (cell-exact to `presentation().main_width`, cells not chars); added direct
  `footer_directory` tail assertions (16-cell segment keeps the basename; 12-cell keeps
  the ellipsis + suffix).
- `narrow_conversation_keeps_confirmed_model_and_provider_visible` — directory asserted
  in the composer footer row; shell row must not repeat it.

Added:
- `footer_chrome_path_renders_once_and_hints_separate_at_140x40`
- `footer_chrome_path_renders_once_and_anchors_at_the_rail_at_80x24`
- `footer_statusline_groups_join_with_the_upstream_separator`
- `home_footer_band_docks_at_the_bottom_at_24_and_40_rows`
- `home_footer_path_follows_the_centered_prompt_anchor`

Results (lane worktree,
`DYLD_LIBRARY_PATH=/Users/mymac/Projects/opencode-rk-recovery-20261005/crates/opentui-bridge/native/lib/aarch64-apple-darwin`):
- `cargo test --locked -p opencode-rk-cli --bin oc2 footer` → **154 passed; 0 failed**
- `home_` 62/0, `status_rail` 36/0, `rail_` 66/0, `sidebar` 134/0
- Full bin suite `-- --test-threads=1` → **2509 passed; 1 failed** — the one failure is
  `native_view_pages::tests::image_preview_page_renders_surface_and_honest_empty_state`
  (`native_view_pages.rs:2167`, "[Image 3]" Description line). PRE-EXISTING on the lane
  base: `native_view_pages.rs` is byte-identical to `57cb5e46` (sha256
  `f0ca1aa69b9db7b0f402e78a39a2937d3906e2e8` both), the module has zero references to
  `footer_line`/`rail_spans`, and `PageState::render_styled` is self-contained — my diff
  cannot reach it. MEDIA-001 owner/integrator should pick this up; it is outside my
  write scope.

## Fresh pyte captures (fixed build, lane debug binary)

`docs/worklogs/research/footer-chrome-captures/` (scripts adapted from the evidence dir
into `/tmp/core-caps/cap_session3*.py`, binary = lane `target/debug/oc2`, dylib = lane
vendored `libopentui.dylib`):
- `ui3-session.txt` (140x40): row 38 path ONCE; row 39
  `shift+tab agents · ctrl+p commands` left + `0.1.0-alpha.1` right.
- `ui24-session.txt` (80x24): row 22 path once at the rail column; row 23 hints
  separated + version right.
- `ui3-initial.txt` / `ui24-initial.txt` (home): unchanged centered group + docked
  version band (upstream-faithful).

## Environment notes

- Both lane worktrees (`ui`, `core`) carried a stale `opentui-bridge` build-script
  binary compiled in the retired F2-DIALOG worktree (baked `CARGO_MANIFEST_DIR`);
  `cargo clean -p opencode-rk-opentui-bridge` fixes it (build artifacts only). Lane
  provisioning should stop copying `target/` across worktrees.
- `git stash` is REJECTED by the Recovery Mode reference-transaction hook
  (`refs/stash` is a protected ref). Do not use stash workflows on lanes; pre-existing
  stashes from other lanes remain untouched.

## Integrator seam

NONE required in `tui_entry.rs` — the whole change is inside `native_view.rs`
(`footer_line` + `footer_statusline_text` + tests). `tui_entry.rs` consumers verified
unaffected: `view.footer_click`/`footer_hit` belong to the SIDEBAR location row
(`native_view.rs:5247`), not the shell footer; `context_usage_hit` is recomputed by
`footer_line` itself; `footer_directory()` getter (`tui_entry.rs:5722`) reads
`status`, not the footer row.
