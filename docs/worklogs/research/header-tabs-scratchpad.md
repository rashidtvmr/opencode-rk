# HEADER-TABS scratchpad (task-3, lane `recovery/ui`)

- **Role:** expert implementation owner, header/tab-strip subsystem.
- **Base:** `5a4510aa2601b35490f3d73b0551e40bbdfe60ed` (main-v2 sync point).
- **Upstream oracle:** `/Users/mymac/Projects/tmp/opencode` @
  `450e7b92412c780a7b5a75226bb8d49aaca24e22` (HEAD verified).
- **Write scope honored:** `crates/cli/src/native_tabs.rs` (+tests) only;
  `native_tab_rail.rs` needed no change (the vertical rail has no plus, no
  spinner divergence, and no header row).

## Verdict on the three reported defects: all three are upstream geometry

The release captures were rendered by the installed binary built from
`57cb5e46` (Nov 7 18:17). No commit has touched `native_tabs.rs` /
`native_tab_rail.rs` since, so the captures reflect the current lane code.
Fresh captures from a build of this lane (`docs/worklogs/research/
header-tabs-captures/`) reproduce the same pixels — and each state matches the
pinned upstream source line-for-line.

### 1. `⠹ Chat` — the busy spinner is IN the tab indicator column, as upstream

Upstream `component/session-tabs.tsx:183-208` (`TabIndicator`): the box is
`width = numberWidth() + 1` cells wide, `justifyContent: "flex-end"` with
`paddingRight: 1`, and a busy tab (`runs() && animations && !numbers`,
`:191-198`) renders `<spinner frames={TAB_SPINNERS.dots.frames}
interval={80}>` inside that box. With one tab, `numberWidth() =
max(2, len("1")) = 2` (`session-tabs.tsx:1690-1692`), so the indicator box is
3 cells: glyph at strip column 1, padding cell at column 2, title from
column 3.

Measured captures (pyte, screen columns):
- old release `p1-ui3-busy-1.txt` row 0: `⠹`@1, `Chat`@3, `+`@33
- fresh lane build `busy-140x40-t1.6s.txt` row 0: `⠇`@1 (dots frame 8), same cells

oc2's port: `indicator_label_frame` / `spinner_frame_glyph`
(`native_tabs.rs:1681-1721`) returns the dots frame for `flags.busy`;
`horizontal_indicator_cells` + `format_indicator_column`
(`native_tabs.rs:1777-1848`) place the glyph at indicator cell `col-2` of the
`gauge = number_width + 1` column — the same right-justified box. The idle tab
keeps `idleLabel ""` (`session-tabs.tsx:176-178`) → blank column, title at
cell 3.

Upstream's OTHER busy surface is the prompt footer status slot
(`prompt/index.tsx:1911-1967`: spinner + interrupt label). oc2 paints that too
(capture row 38 `⬝⬝■■■■■■ esc interrupt`). Moving the tab spinner to the
footer would REMOVE an upstream render and diverge; no change made.

### 2. `+` at column 33 — the flow end after a 32-capped tab, as upstream

Upstream layout: `HorizontalSessionTabs` renders the tabs in one row and then
`<Show when={showPlus()}>` paints `" + "` (`ADD_TAB_WIDTH = 3`,
`session-tabs.tsx:76,1764-1800`) immediately after the solved tabs; the
available width subtracts the add button first
(`session-tabs-model.ts` caller `session-tabs.tsx:1356-1360`). Tab widths come
from `adaptiveSessionTabLayout` (`context/session-tabs-model.ts:178-260`) with
`SESSION_TAB_WIDTH = 22`, `SESSION_TAB_MAX_WIDTH = 32`,
`SESSION_TAB_MIN_WIDTH = 8` (`:33-35`).

One visible tab at ANY terminal width: `available = width - 3`, `fit` keeps 1
tab, `roomy` → `total = min(available, 32) = 32` → the tab occupies columns
0-31 and `" + "` lands at columns 32-34 (glyph at 33). The captured `+` at 33
is therefore upstream's own single-tab geometry, identical at 80 and 140
columns.

oc2's port: `adaptive_layout` (`native_tabs.rs:1540-1650`) is a direct port of
the same solver (same 22/32/8 constants; allocation-vector test at
`native_tabs.rs:3302`), and the plus is placed at the flow end
`x_or_y` after the tabs and overflow markers
(`native_tabs.rs:1190-1200`), capped to `available`. The multi-tab case moves
the plus with the flow end (new test: five tabs → plus at the summed slot
width, not 32).

### 3. `Chat` at row 2, column ~100 — the session sidebar's title heading

Upstream `routes/session/sidebar.tsx:33-48` renders the session title as a
BOLD `title_shimmer` heading at the top of the session sidebar
(`paddingTop={1}` at `:27`). The tab strip shows the same session title — both
renders exist upstream. The captured title "Chat" is oc2's default session
title (`crates/cli/src/chat.rs:253-254` — the daemon-accepted title for a
session created without one), not a duplicated header paint.

At 80 columns the sidebar is not visible
(`NativeView::sidebar_visible` requires the session route plus width) and the
fresh 80x24 capture shows no second `Chat` — width-dependent, as upstream.
The sidebar itself is `native_sidebar.rs` / `native_view_status.rs` — outside
this task's write scope; nothing to remove here.

## Bonus finding (out of scope, for the integrator)

During the busy window the transcript is still empty, and the session body's
first row shows the WORKING DIRECTORY (`/private/...`) — the workspace header
row `ChatView::render_header` (`native_view_chat.rs:613-626`, painted at
`:4536/:4542/:4556` in every typed branch). The function's own comment
concedes "Upstream has no transcript-header status"; upstream's session view
paints no directory row above the transcript (the directory lives in the
prompt footer location slot and the sidebar footer). When messages arrive the
transcript window covers the row, which is why the settled capture shows no
header. Removing or relocating that row is a `native_view_chat.rs` change —
documented here, not implemented (outside write scope).

## Changes made (`native_tabs.rs`, +106 lines, tests only)

Three focused tests pinning the upstream geometry so it cannot silently
diverge:
- `header_tabs_busy_spinner_paints_the_tab_indicator_column` — busy glyph at
  indicator cell 1, title at cell 3, slot width 32, idle column blank;
- `header_tabs_plus_follows_the_tab_flow_end_across_widths` — plus at 32-34
  with one tab at 80 AND 140 columns; plus at the summed flow end with five
  tabs;
- `header_tabs_strip_paints_one_row_with_one_title` — one tab, one strip row,
  one title occurrence.

## Test evidence (exact commands, lane worktree @ candidate)

```
DYLD_LIBRARY_PATH=/Users/mymac/Projects/opencode-rk-recovery-20261005/crates/opentui-bridge/native/lib/aarch64-apple-darwin \
  cargo test --locked -p opencode-rk-cli --bin oc2 header_tabs
→ 3 passed; 0 failed
… --bin oc2 native_tabs    → 74 passed; 0 failed (71 pre-existing + 3 new)
… --bin oc2 native_tab_rail → 43 passed; 0 failed (file untouched)
… --bin oc2                → 2520 passed; 0 failed (full bin suite, green)
```

## Fresh captures (committed under `docs/worklogs/research/header-tabs-captures/`)

- `busy-140x40-t{0.4,1.6,3.6}s.txt`, `settled-140x40.txt` — from
  `capture-thinking-140x40.py` (adapted from the evidence script; BINARY =
  lane `target/debug/oc2`, DYLD_LIBRARY_PATH added for the debug build).
- `initial-80x24.txt`, `session-80x24.txt` — from `capture-session-80x24.py`
  (140x40 evidence script adapted to 80x24).
- Key rows: busy 140x40 row 0 `⠇ Chat … +` (glyph@1, +@33), row 2 sidebar
  title `Chat`@~100, row 38 interrupt scanner; session 80x24 row 0 `  Chat …
  +` (+@33), no sidebar title (sidebar hidden at 80 columns).
