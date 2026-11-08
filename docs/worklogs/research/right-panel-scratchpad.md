# RIGHT-PANEL-OVERLAP scratchpad (task-4, lane `recovery/core` @ `5a4510aa`)

Role: expert implementation owner, right sidebar / right-pane geometry. Base
`5a4510aa2601b35490f3d73b0551e40bbdfe60ed`. Write scope honored:
`crates/cli/src/native_view_layout.rs` (+ its test module) only —
`native_view_pages.rs` needed **no** change (it hosts the graph/image-preview
page panels, not the right sidebar), and the paint seam in `native_view.rs`
was NOT edited (see the integrator seam below).

## Evidence audit — what the captures actually show (measured, not assumed)

Measured `/Users/mymac/Projects/oc2-p1-release-20261107/ui-defect-evidence/p1-ui3-session.txt`
(140x40, sidebar auto-docked at cols 98-139, main column 0-97):

- Row 6: assistant text `G2 first response` occupies cols 5-21; the panel title
  `⬖ Getting started … ✕` occupies cols 100-116. **Disjoint column ranges** —
  the transcript IS shrunk (`presentation().main_width == 98`,
  `native_view.rs` `render_content` wraps the chat body at
  `main_width - 4 = 94`). They share a row, not cells; upstream renders the
  same side-by-side arrangement.
- Row 2 `Chat` at cols 100-103 and row 18's path at cols 100-137 are the
  SIDEBAR'S OWN header and footer-location rows — upstream-faithful:
  `routes/session/sidebar.tsx:25-77` (title header, scrollbox, footer slot)
  and `feature-plugins/sidebar/footer.tsx:93-113` (`sidebar.footer.location`,
  `FilePath maxWidth={38}` — the truncation the user reads as "folder name
  partially hidden" is upstream's own 38-cell budget).
- The task's premise "the transcript does not shrink when the right panel is
  open" is FALSE for the docked regime; acceptance items 1-2 already hold
  there and are now pinned by rendered tests (below).

## The REAL overlap — measured and reproduced (narrow drawer regime)

With VERTICAL tabs (`TabReservation::VERTICAL_WIDTH = 42`) at 140x40, the
effective width is `140 - 42 = 98 <= 120`, so the sidebar does not auto-open;
when it is explicitly opened (`toggle_sidebar`), oc2 enters the narrow OVERLAY
regime (`narrow_sidebar_overlay_open()`, LAY-08):

- `presentation()`: `main_offset=42, main_width=98, sidebar_offset=Some(98),
  sidebar_width=42` — the transcript lays out across cols 42-139 while the
  drawer owns cols 98-139.
- Probe (temporary test, since removed) with a 264-char assistant message and
  a bound sidebar location: rows 1-2 get clipped at col 97 and the sidebar's
  rows paint at 98+, but **row 3+ keep the transcript text flowing through
  cols 98-137** because the overlay paint
  (`native_view.rs:5414-5427`) iterates only `sidebar.iter()` — the sidebar's
  content vec is shorter than the frame, so every frame row below the vec
  keeps its transcript cells under the panel region. That inconsistent
  clipping is exactly the user's "folder name and other placeholder partially
  hidden behind … right sidebar".
- Upstream's drawer (`component/session-frame.tsx:383-391`) paints an
  absolute, right-anchored sidebar over a dim backdrop
  (`RGBA.fromInts(0, 0, 0, 70)`) covering the whole area — the overlap is
  intentional and visually declared. oc2 has no backdrop and only partially
  covers.

## Upstream oracle (pin verified `450e7b92412c780a7b5a75226bb8d49aaca24e22`)

- `packages/tui/src/component/session-frame.tsx:106` —
  `wide = width - verticalTabsWidth > 120` (strict at 120).
- `:109-115` — `sidebarVisible`: parent sessions never; explicit open always;
  `config.session.sidebar ?? "auto"` only while wide.
- `:115-120` — `rightPane()` precedence: panel > open sidebar > terminal >
  auto sidebar.
- `:262-336` — the row flex split: `#session-pane` `flexGrow={1}
  flexBasis={0} minWidth={0}` shrinks; the right pane is `flexShrink={0}` at
  `SESSION_SIDEBAR_WIDTH` (42, `ui/layout.ts:1`) for the sidebar and the
  resized width for terminal/panel — docked panes always shrink the main
  column.
- `:383-391` — the narrow drawer: absolute, right-anchored, dim backdrop.
- `packages/tui/src/routes/session/sidebar.tsx:25-77` — sidebar column
  structure (header / scrollbox / footer slot).
- `packages/tui/src/feature-plugins/sidebar/footer.tsx:16-126` — the
  "Getting started" onboarding block and `sidebar.footer.location`
  (`FilePath maxWidth={38}`).

## What I shipped (in scope)

`native_view_layout.rs`:

- New `SessionPaneLayout { main_offset, main_width, sidebar_offset,
  sidebar_width, docked }` + `session_pane_layout(...)` — the single-source
  right-pane geometry authority porting the upstream model above (visibility
  gates, `right_pane` precedence, dock-vs-drawer, docked main-width shrink).
  `docked == true` means `main_width` already excludes the pane (no main
  cell can reach the pane); `docked == false` means the pane overlays those
  columns and the caller owns the drawer contract.
- Tests: `session_pane_layout_docks_and_shrinks_main_at_140x40`,
  `session_pane_layout_overlay_prescribes_the_upstream_drawer`,
  `session_pane_layout_panel_and_terminal_panes_always_dock`,
  `session_pane_layout_parent_and_hidden_gates_suppress_the_sidebar`,
  `right_pane_docked_frame_never_paints_under_the_panel_at_140x40` (rendered:
  long transcript + bound sidebar; asserts no `reply token` cell at
  cols >= 98, the panel title inside its own columns, and the footer path
  left of the pane), `right_pane_overlay_geometry_matches_the_upstream_drawer`
  (rendered drawer-geometry facts).

Results (lane worktree,
`DYLD_LIBRARY_PATH=/Users/mymac/Projects/opencode-rk-recovery-20261005/crates/opentui-bridge/native/lib/aarch64-apple-darwin`):
`session_pane_layout` 4/0, `right_pane` 9/0, `native_view_layout` 41/0,
`sidebar` 135/0, `layout_` 43/0. Full bin suite: see the handoff (background
run recorded in the commit message).

Fresh pyte captures (140x40, panel open/docked) committed under
`docs/worklogs/research/right-panel-captures/` (scripts adapted into
`/tmp/core-caps/cap_session3*.py`, lane debug binary + lane dylib): the
session view shows the transcript at cols <= 21 with the panel at 98+ —
clean separation; footer path once at row 38.

## Integrator seam (the actual overlap fix — OUT OF MY WRITE SCOPE)

The narrow-drawer overlap is fixed in `native_view.rs` (not edited per the
task). Two upstream-anchored options; the task's acceptance ("no transcript
cell paints under the panel") is satisfied by either:

1. **Shrink (task-literal):** in `NativeView::presentation()`
   (`native_view.rs:2881-2901`), replace the duplicated
   `sidebar_wide()/inline` logic with
   `crate::native_view_layout::session_pane_layout(...)` and treat the pane
   as docked whenever it is visible (drop the `&& self.sidebar_wide()` from
   `inline`). The transcript then wraps at the pane's left edge in every
   regime; the overlay paint block at `native_view.rs:5414-5427` becomes dead
   and can be removed. Note: this deviates from upstream's narrow drawer
   (`session-frame.tsx:383-391`) — a deliberate product choice that should be
   recorded (LAY-08's overlay contract is superseded for the geometry, the
   Esc/Ctrl+C close layer at `tui_entry.rs:14165` stays useful).
2. **Upstream-faithful drawer:** keep the geometry, and at the overlay paint
   (`native_view.rs:5414-5427`) pad the sidebar slice to the frame height
   (`sidebar` rows are shorter than the frame — pad with blank 42-cell rows)
   so the drawer covers the full column, and paint the upstream dim backdrop
   over the covered region before the drawer rows. This keeps upstream's
   drawer UX but still hides content behind the pane (dimmed), which does not
   satisfy the task's acceptance literally.

Recommendation: option 1 — it matches the task's acceptance text, removes the
inconsistent partial clipping I measured, and the layout authority is already
committed for it.

## Environment notes

- The stale `opentui-bridge` build-script binary (retired F2-DIALOG baked
  path) reappeared in this worktree's `target/`; fixed with
  `cargo clean -p opencode-rk-opentui-bridge` (build artifacts only).
- `git stash` remains hook-blocked; not used.
