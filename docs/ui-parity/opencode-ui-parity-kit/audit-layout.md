# OpenCode v2 pinned UI: layout, navigation, and sidebar source audit

**Authoritative baseline:** `anomalyco/opencode`, project-selected commit `450e7b92412c780a7b5a75226bb8d49aaca24e22` (2026-10-25, package version 2.1.4, OpenTUI 0.6.8). Also compared against release tag `v2.2.0` at `1097e2668b1e08ad63bdf0b1c4860b9bc01e420c` (OpenTUI 0.7.0); section 13 records the differences. This is the full-screen TUI in `packages/tui`, not the desktop app and not `src/mini`. Findings below come from source and test inspection; no claim of a completed Rust port or live visual parity is made.

## 1. Important version distinction

The project-pinned v2 default right sidebar consists of **Context → Limit → MCP → Todo**, followed by a pinned footer. These are independent built-in contributions to `sidebar.content` and `sidebar.footer`. Each contribution can return no content, so the visible set depends on state. The title above them belongs to the sidebar shell itself. **There is no built-in LSP list or changed-file list in this pinned sidebar.** Do not copy a v1 screenshot and call it a reproduction of the project-pinned v2 UI. Additional sections can be useful extensions, but keep them out of an exact-baseline fixture unless the target OpenCode installation has an additional plugin. The built-in diff viewer is a separate panel, not an always-present changed-files sidebar. [L01], [L02]

The sidebar is not the left vertical tab rail. They happen to share the default width of 42 columns. The left rail is resizable and can become a compact 5-column rail; the right information sidebar remains 42 columns. [L03], [L04], [L05]

## 2. Whole-screen layout contract

All dimensions here are **terminal cells**, not browser CSS pixels. Any Rust bridge must make measurement, clipping, foreground/background composition, and pointer coordinates use the same terminal-cell coordinate system.

### Root tree

The root is a column exactly the terminal width and height, painted `theme.background.base`. Its main child is a growing row with `minHeight=0`, containing an optional vertical tab rail and the remaining route area. That route area is a growing column with `minWidth=0`. If tabs are horizontal, their one-row strip appears above the route. The route is Home, SessionFrame, or plugin content. App slots and overlays are layered after route content; the optional development bar is outside the main row. Toast/loading/reconnection/migration layers are root children. This hierarchy matters: putting tabs, sidebar, and prompt into one undifferentiated horizontal box produces incorrect shrink behavior. [L04]

The normal session frame is a row with a growing session pane and at most one right pane. Right-pane selection priority is: active plugin panel; manually opened sidebar; selected terminal; auto-visible sidebar. Opening a plugin panel clears manual sidebar state and the terminal selection. Selecting a terminal closes a plugin panel and manual sidebar state. Switching sessions closes a panel that belongs to another session. [L04], [L05]

### Constants and thresholds

| Item | Exact rule | Consequence |
| --- | --- | --- |
| Right information sidebar | 42 columns | Content does not stretch proportionally with the terminal |
| Left vertical rail preferred width | 42 columns | Persisted width can override |
| Left rail minimum / maximum | 5 / 72 columns | Also constrained by total width minus 44 |
| Left compact rail | Width strictly less than 12 | Width 11 is compact; width 12 is expanded |
| Vertical layout eligibility | Terminal width ≥ rail width + 64 | Default rail switches to horizontal below 106 columns |
| Auto right sidebar | Remaining width after left rail **strictly greater than 120** | At 120 it is hidden by default; at 121 it docks |
| Plugin panel split eligibility | Remaining width ≥ 108 | At 107 it becomes full screen; at 108 it may dock |
| Main route horizontal padding | 1 column each side if terminal width <44; otherwise 2 | The condition uses full terminal width |
| Session route bottom padding | 1 row | Prompt footer is inset from terminal bottom |
| Home prompt maximum width | 75 columns | The prompt is centered within the route |

The rail width clamp is `max(5, min(requested, 72, total - 44))`. Do not replace it with a guessed percentage. Vertical eligibility tests the **clamped current width**. Thus a configured 5-column rail fits vertically at terminal width 69, while a default 42-column rail needs 106. For default vertical tabs plus an auto information sidebar, both dock at total width 163: `163 - 42 > 120`. At 162 the right sidebar is not auto-docked. [L03], [L04], [L05]

The right panel/terminal resize clamp is different. Let `half=max(1,floor(available/2))`; the result is `max(min(24,half), min(requested,max(half,available-44)))`. The default is half the available panel width. The shared storage key is `layout`, using `paneWidth`, with legacy `terminalWidth` fallback. The left rail uses `verticalTabsWidth` in that same layout store. [L03], [L05]

### Information sidebar when narrow

When manually opened at remaining width ≤120, it is an absolute overlay, covering the frame with a black backdrop of alpha 70/255 and aligning the 42-column sidebar to the right. This overlay does not reserve 42 columns from the conversation. Escape or Ctrl+C closes this narrow sidebar through a higher-priority key layer. Subagent sessions (a session with `parentID`) never show this information sidebar. The source does not define a custom reduced-width sidebar below 42 columns; capture the baseline renderer at extreme widths instead of inventing one. [L05], [L06]

### Plugin panel focus and full screen

Below its 108-column split threshold, a plugin panel becomes full screen; both horizontal and vertical session tabs are hidden. The prospective width calculation still accounts for a configured vertical rail so transitions do not oscillate. The conversation pane remains mounted but is invisible and noninteractive; a full-screen panel uses all currently available frame width. Pane focus is explicit. When the right pane has focus, the session pane gets an absolute click guard; mouse-wheel events inside the transcript viewport are forwarded to the transcript, and a click returns focus without accidentally approving a permission underneath. Default focus bindings are leader+Left and leader+Right. [L04], [L05], [L07]

### Resize interaction

Resize handles have a **2-column hit target**, one visible highlighted column, `zIndex=10`, and full parent height. The highlight appears on hover or drag using `background.raised.high`. Left rail sizing uses pointer `x+1`; right-pane sizing uses `terminalWidth-x-1`. Drag updates preferred size locally, and persisted size changes once at release. A second boundary click within 300 ms resets to default. Parent-level drag/release handlers keep resizing active outside the handle. Do not persist on every mouse move or change the hit target to a 1-column decorative line. [L04], [L05], [L08]

## 3. Main conversation geometry and scrolling

Inside SessionFrame the session route has a growing transcript area, an always-reserved one-row line below it, then a nonshrinking composer region, then the route's one-row bottom padding. The reserved line displays right-aligned history-loading text or a hoverable “Jump to latest ↓” action; it exists even when empty. Removing that empty row moves the prompt and breaks parity. The jump action has 1 column of left padding and changes its secondary-action text token on hover. [L06]

The scrollbox is sticky to the bottom except during explicit message navigation/navigation slack. Its viewport adds 1 column right padding when the scrollbar is enabled; the vertical scrollbar also has 1 column left padding. The track background is a decreased raised surface and foreground is `border.base`. Each session row's outer wrapper has `marginTop=1`. The sidebar scrolls independently, and the composer never scrolls with messages. Scroll acceleration is either the OpenTUI macOS accelerator when explicitly configured, a configured constant speed, or the default constant speed **3**. [L06], [L09]

Scroll restoration is anchored to a message/group target and its screen Y position, not a raw absolute scroll offset. Parent and subagent anchors are separate even though they share a root session tab. Expansion state is per-session and per-group. Closing a root tab clears anchors and expanded groups for its family. These are layout semantics, not cosmetic extras: recreating only text at the bottom will lose the user's reading position after switching tabs or resizing. [L10]

One source caveat for a faithful port: the render-context content width subtracts 4 from the measured session pane even though terminal widths below 44 use physical padding of 1 on each side. Preserve the upstream behavior in parity fixtures first; any intentional fix is a separately recorded divergence. [L06]

## 4. Home screen

Home uses the same horizontal inset rule, centered horizontal alignment, a growing empty spacer, an extra shrinking 3-row spacer, the real Logo component, a fixed 1-row gap, update notification, a prompt wrapper (`width=100%`, `maxWidth=75`, top padding 1, `zIndex=1000`), and a second growing spacer. The footer is a separate full-width, nonshrinking child. The result is a vertically balanced composition, not a prompt pinned at a hard-coded Y coordinate. [L11]

Global MCP forms can appear while on Home: the prompt is disabled and the form is an absolute overlay with left/right inset 2, bottom offset 1, `zIndex=2000`. Update notification layout is constrained to the measured logo width, uses hover action coloring, and has behavior for available/installed/remote updates. [L11]

The default home footer is hidden below height 14 or width 44. It has left/right padding 2, horizontal gap 2, and top/bottom padding 1 except at heights 14–15 where those vertical paddings are zero. MCP status and plugin failure status are left; a growing spacer pushes the version to the right. `/mcps` and version appear at width ≥64; `/plugins` appears at width ≥80. MCP failure count wins over connected count. Each status block opens its matching picker. The footer also has the additive `home.footer.status` extension slot. [L12]

## 5. Horizontal session tabs

### Width allocation

The strip is exactly one row, nonshrinking, horizontally arranged, `zIndex=1`. An idle plus affordance reserves 3 columns and renders a plus with one space on either side. When on a new-session page that control promotes into one active “New session” tab; it is not duplicated as a separate plus and draft tab. [L13]

The layout model uses preferred width 22, max width 32, and minimum inactive width 8. It maintains a sticky window of visible tabs around the active tab. Overflow labels reserve `digits(count)+2` cells for an arrow, count, and gap. Roomy layouts split width equally up to max 32, with integer remainder distributed from the first visible tab. Under pressure the active tab receives the remaining width after equally sized inactive tabs. A single tab caps at 32 rather than stretching across the terminal. [L14]

Authoritative model test examples (available width below is the width passed to the model, **after** the caller reserves the idle plus):

| Tabs / active / available | Expected widths |
| --- | --- |
| 7 / fourth / 76 | 9, 9, 9, 22, 9, 9, 9 |
| 5 including active draft / 54 | 8, 8, 8, 8, 22 |
| 3 / any / 100 | 32, 32, 32; 4 cells remain unallocated |
| 4 / second / 100 | 25, 25, 25, 25 |
| 5 / third / 100 | 19, 19, 24, 19, 19 |
| 1 / first / 100 | 32 |

These already have unit tests; port the function with its rounding and sticky-window rules rather than approximate it with generic flex-grow. [L15]

### Label, status, and colors

The indicator region reserves `max(2, digits(totalTabs))+1` cells, including its 1-cell right pad, so replacing an ordinal with a spinner does not move titles. Title resting width is `max(1,tabWidth-1-numberWidth)`, shortened by 2 more cells on hover to reveal the close control. Titles are grapheme-aware, use display-cell width, and fade over their final 4 cells when truncated; they do not simply append an ellipsis. Selected/dragged titles are bold. Selected background blends toward a decreased raised surface; hovered inactive tabs use the primary action hovered surface. [L13], [L16]

The status-mode priority is: permission `!`; question `?`; animated busy spinner; plus; unread marker; otherwise an empty indicator. In numbers mode, the ordinal remains visible even when busy or blocked. Busy spinning is suppressed when attention is required. Defaults are the dots spinner at 80 ms and small bullet unread marker; alternate styles exist in the component. Error unread uses the error text token; attention uses the accent hue. A busy child or non-synthetic queued user work can make the root tab busy; a child permission/question can make the root attention indicator appear. [L16], [L10]

Long title hover starts a marquee after 600 ms, advances every 80 ms, runs one cycle, then returns to offset zero. Leaving resets it; moving between descendants has a deferred hover-clear to avoid flicker. Membership changes animate surviving tab widths from current values and grow new tabs from zero with a 0.1s visual-duration spring; terminal resizes normally jump to final widths. [L16], [L13]

### Mouse behavior

Left press then release selects; middle-click closes without first selecting; right-click opens a context menu. Hover reveals a close glyph one cell in from the right edge. A pointer release over an invisible/unhovered close glyph must not close a tab. Dragging previews reorder locally; releasing commits one move and selects the dragged tab. A synthetic mouse-up after drag must not activate a plus/close control. [L13], [L17]

After click-close, the next close target is held under the same pointer cell for up to 5 seconds, including overflow-window changes; moving outside the strip or changing incompatible state releases the hold and allows reflow. This subtle behavior is tested and is part of a polished exact port. [L16], [L17]

Right-click on an existing tab offers New session, Rename, Copy session ID, Close. Right-click on the plus/draft area shows up to 10 recently closed tabs. The underlying recently-closed stack holds up to 25, unique by session ID. Context-menu width is at least 20 and at most 50 before the final terminal-width clamp; each label's requested width includes 2 cells of horizontal padding. Its height is clamped to terminal height; it appears below the click, shifted inside screen edges, with absolute z-index 2500. Rows have one cell of horizontal padding and are 1 row tall. Escape/Ctrl+C or outside press dismiss. [L16], [L14]

## 6. Vertical session tabs

The expanded rail is full height, uses `background.raised.base`, has one row top padding, and a scrollbox with hidden scrollbars. Normal tab entries are 2 rows tall, separated by a 1-row gap (stride 3). The first row is indicator+title; the second is project/worktree detail. Detail is the project name, plus `⎇ branch` only when it is a worktree whose branch differs from the default branch. A normal branch in the canonical project does not always appear. [L18], [L14]

Indicator width is still `max(2,digits(total))+1`. Expanded title resting width is `railWidth-prefixWidth-1`; hover removes one additional title cell. First row right padding is 1; second row left padding is the prefix width and right padding 2. Selected expanded tabs use the selected primary action surface; hovered/dragged unselected tabs use raised-high. Muted details are further tinted toward the background by 0.35. Title renaming uses a dedicated shimmer; with animations disabled it becomes dim text. [L18]

The add slot is a single 1-row control separated by the same gap; it shows “+” and New session when expanded, and can itself become the selected draft tab. Normal expanded hovered tabs show `✕` at right 1; the draft uses `×`. Selecting a tab scrolls its entry into view after layout on the next frame. Drag indices derive from viewport Y, current scrollTop, and stride; the compact version includes an extra offset. [L18]

### Compact mode (<12 columns)

A compact rail shows a centered search glyph `⌕` at top, then centered single-row indicators at stride 2, including the plus. Idle status mode displays the first grapheme of the trimmed title (fallback U), rather than an ordinal. Active/hovered entries use half-block caps above/below (`▄` and `▀`) with neighbors' colors so adjacent highlights join correctly. The selected compact glyph is bold. It uses the same status overrides and optional numbers mode as the full tabs. [L18], [L19]

Hovering a compact item opens a tooltip immediately unless dragging or a context menu is open. It starts at the rail's right edge, is `min(54,terminalWidth-railWidth)` columns wide and 4 rows high, with two content rows, one-cell horizontal inset, and half-block top/bottom caps. It displays title and detail. Vertical placement is clamped to the screen; z-index 2000. Search requires a corresponding left-button press and release, avoiding activation when a transcript selection merely ends over it. [L18], [L19]

The existing renderer test checks an 80×24 frame with rail width 5: search at row 1; session glyphs at rows 3,5,7; plus at row 9; transcript starts at column 5. Treat this as a useful starting golden fixture. [L20]

## 7. Navigation state and persistence

Default tabs config is `mode=auto`, `scope=cwd`, `layout=horizontal`, `indicators=status`; auto is disabled in the `HERDR_ENV=1` terminal environment. Tabs render only if enabled, at least one real session is open, and the route is not a plugin route. Thus initial fresh Home does not necessarily display a plus rail despite Home having a draft state. [L04], [L21]

Persisted tabs have global and per-working-directory buckets keyed by session ID. Subagent routes normalize to the root tab, so opening a subagent is not automatically opening a separate tab. Titles prefer live session data, then persisted fallback, then generated title fallback. The provider separately retains session data, per-session scroll anchors, expanded groups, recent closed tabs, selection history, and background prompt pulse counts. Recent-closed and navigation history are in memory; tab order and metadata persist. [L10]

Unread is derived from server `time.idle > time.viewed` (or no viewed time), not a local boolean cleared on click. Root outcome failed yields error unread. A selected session is acknowledged viewed only once focus has been confirmed. Failures retry with exponential delay starting 250 ms and capped 5s. Family attention is permission first, then form/question; selecting does not itself clear a pending question or permission. Background subagent completion alone does not mark the root unread. [L10]

Closing the active tab prefers the previously visited still-open tab from navigation history; absent that, it chooses the right neighbor, then left, then Home. Reopen restores the original index, clamped to current list length. The history limit is 100. Closing a background tab should not steal focus. Deleting a session also removes it from reopen history. Async tab admission is cancellation-aware so a slow persisted write cannot resurrect a just-closed tab. Shared-store updates must not reopen a tab solely because another client still views it. [L10], [L14]

Defaults to reproduce: Ctrl+Tab/Alt+Down next; Ctrl+Shift+Tab/Alt+Up previous; Alt+Shift+Down/Up next/previous unread-or-attention; leader+W close; Ctrl+Shift+T reopen; leader+1…0 and Ctrl+1…0 direct select; leader+B sidebar. History back defaults to unbound, forward to Ctrl+I. Read bindings through the configured command registry so customized shortcuts change hints and behavior together. [L22]

## 8. Right sidebar details

### Shell

Surface: raised base. Fixed 42×full-height; padding top 1, bottom 1, left 2, right 2. Inner width 38. Pinned title wrapper adds right padding 2 and bottom padding 1; title is bold or dim while pending with animations disabled. The central scrollbox grows with `minHeight=0` and never shows a horizontal bar. Its vertical bar is positioned absolutely at right 0, width 1; it does not reflow text when visibility changes. The content stack reserves 1 column right padding and gap 1 between sections. Footer is outside the scrollbox, nonshrinking, with top padding 1 and gap 1. Therefore its working-directory label stays at the bottom as central sections scroll. [L02]

### Context

Shows if usage exists or session cost>0. Heading bold base text. Separate muted lines for localized token count, rounded percent used when a context limit is known, and USD cost when>0. Tokens use the last eligible assistant usage before the revert boundary and sum input+output+reasoning+cache-read+cache-write. Percentage uses that assistant's model/provider context limit; it is not the sum of every message or the currently selected model's limit. Missing usage is hidden, not fabricated as 0%. Cost is separately fetched as session cost. [L23]

### Limit

Only shown when workspace usage limits apply to the selected model. Heading bold, then currency remaining. At out/reached levels the remaining text is error-colored; otherwise muted. This is a different quantity from context-window consumption and spending. [L24]

### MCP

No section if no servers. For one or two servers always show rows and omit disclosure arrow. For more than 2, show `▼`/`▶`, preserve expanded preference (`view.open`, initially true), and toggle on heading mouse-down. A collapsed summary gives connected active count and failed-or-needs-auth error count. Each row is a horizontal three-part line: colored bullet, bold name that takes remaining space and truncates without wrapping, and fixed-width status at right. Row gap 1. Connected bullet success; failed error; disabled muted; needs-auth warning; pending/unknown muted. Status labels are Connected, Connecting, Error, Disabled, Sign in. Only failed status text itself is error-colored; the rest is muted. Clicking a row opens that server's MCP dialog, with details already opened for failed servers. [L25]

No extra row background hover is implemented here; do not invent one and mark it exact. Global MCP failure/auth transitions additionally trigger a toast once per changed problematic state, with an action to open servers. One small source edge case should be preserved or explicitly classified as a deliberate fix: the MCP collapsed summary depends only on persisted `!view.open`, so if a list shrinks to≤2 while collapsed, rows become visible and the summary can remain; Todo's summary additionally checks list length>2. [L04], [L25], [L26]

### Todo

No section with no current todos. Current list is the input of the latest **completed** `todowrite` before revert boundary; partial/running calls do not replace it. Same >2 collapsibility threshold and initial open preference as MCP; collapsed summary counts pending+in_progress as remaining. Markers are `[ ]`, `[•]`, `[✓]`, `[-]`. In-progress is warning text; cancelled faint and strikethrough; other states muted. Marker is nonshrinking, body word-wraps. [L26]

### Footer

An onboarding card appears only at terminal height ≥22, after integration state has loaded, when no provider is connected and the card has not been dismissed. Raised-high surface, vertical padding 1, horizontal padding 2, row gap 1; icon at left, heading+dismiss, explanatory copy, and Connect provider action at right within a vertical content box. Dismiss persists. [L27]

Working-directory text is a formatted path, optionally followed by `:currentBranch`, with display max-width 38. It uses the shared FilePath algorithm to preserve basename/extensions and Windows/UNC roots under truncation, accounting for graphemes/display width. Hover changes muted to base text. Click opens Copy path / Open folder / Workspaces actions; text selection suppresses that menu. The footer is separate from the below-prompt rail: preserve both when the baseline renders both. [L27], [L28]

## 9. Motion beyond a spinner

Tabs have running sweeps, completion pulses, prompt-admission flashes, unread glow, attention glow, title shimmer, selection/width transitions, and hover marquee. `TabPulseRenderable` has explicit elapsed-time state: running cycle 2800 ms, running attack 450 ms, fade-out 500 ms, completion 1200 ms, edge flash 800 ms, glow ignition 600 ms, and glow release 900 ms. At the project-selected commit it is a live renderer node advanced by each renderer frame's `deltaTime`; the 20 fps timer-driven optimization belongs to the later v2.2.0 tag, not this baseline. These cannot be reproduced by swapping Unicode spinner characters alone. Clone the timing/easing and color-blend algorithm or route this production renderable through an equivalent bridge. Use a controllable clock in tests. [L29]

## 10. Proposed Rust implementation boundaries (recommendations, not observed Rust APIs)

1. **LayoutPolicy**: pure cell-based computation of route insets, vertical/horizontal choice, compact mode, right-pane selection, overlay/dock/fullscreen, clamps, and adaptive tab width array. Feed actual frame width into children after layout.
2. **SessionTabsStore**: stable session identities, ordering, scope, root/family projection, unread/attention/busy projection, MRU history, closed stack, anchors, expansion state, and one-shot persistence effects. Keep animation state outside persisted data.
3. **NavigationRenderer**: expanded rail, compact rail, horizontal strip, tooltip, tab context menu; reuse indicator, marquee, title treatment and pulse primitives.
4. **SidebarShell** plus ordered **section contributions**: Context, Limit, MCP, Todo, footer; remove hidden sections before vertical stacking. Expose append/replace slots if plugin parity is intended.
5. **PaneController**: mutually exclusive panel/terminal/info sidebar; focus capture; fullscreen transitions without remount; shared resize behavior.
6. **TextMetrics**: grapheme iteration, display-cell width, truncation/fade, path shortening. Centralize this, since title, status, path, and mouse hit boxes must agree.
7. **Clock and event queue**: deterministic animations, post-layout reveal, drag-release suppression, debounce/timeout, and deferred effects.

## 11. Acceptance checklist

Each row is a concrete uncompleted acceptance criterion. Capture the production TS component and Rust component from identical data/config/terminal/theme/clock and compare cell text, fg/bg, attributes, bounds, clipping, focus, and effects. Do not declare parity from text-only snapshots.

### Page and pane fixtures

- [ ] **LAY-01** Root fills terminal at 80×24,120×40,160×48,200×60 without unintended overflow.
- [ ] **LAY-02** Width 43/44 switches route horizontal inset 1→2; prompt/footer remain within viewport.
- [ ] **LAY-03** Default vertical rail at 105→horizontal,106→vertical; width 60 rail at 123→horizontal,124→vertical.
- [ ] **LAY-04** Resizing rail respects 5/72/min-content 44; compact transition exactly 11/12.
- [ ] **LAY-05** Width 68/69 with width 5 rail crosses vertical eligibility correctly.
- [ ] **LAY-06** No-left-rail width 120 has no auto sidebar;121 shows fixed 42 sidebar.
- [ ] **LAY-07** Width 162/163 with default vertical rail crosses auto-right-sidebar threshold.
- [ ] **LAY-08** Manual narrow sidebar overlays rather than shrinks transcript; Escape/Ctrl+C closes.
- [ ] **LAY-09** Child session suppresses information sidebar, including manually requested state.
- [ ] **LAY-10** Panel width 107/108 switches fullscreen/docked; tabs hide only while panel full screen.
- [ ] **LAY-11** Sidebar/terminal/plugin panel precedence matches source; one right pane at a time.
- [ ] **LAY-12** Pane focus guard cannot pass Enter/click into a permission prompt; transcript wheel remains usable.
- [ ] **LAY-13** Resize handle 2-cell target,1-cell highlight, drag beyond bounds, release persistence, double-click reset 300 ms.
- [ ] **LAY-14** Transcript and sidebar scroll independently; title and footer remain pinned.
- [ ] **LAY-15** One reserved row below transcript remains present when jump action is absent.
- [ ] **LAY-16** Scrollbar show/hide reserves expected cells; sidebar absolute scrollbar does not reflow headings.
- [ ] **LAY-17** Switching tabs/restoring after reflow preserves semantic anchor and screen Y; child anchor independent.
- [ ] **LAY-18** Home prompt max-width 75, logo spacing, balanced spacers and footer gates match at short/wide sizes.
- [ ] **LAY-19** Home global form disables prompt and appears at expected bottom inset/z-order.
- [ ] **LAY-20** Width 44/64/80 and height 13/14/15/16 correctly gate home footer/actions/version/padding.

### Tab fixtures and interactions

- [ ] **TAB-01** Port all adaptive layout numerical examples and sticky overflow-window tests.
- [ ] **TAB-02** Idle plus consumes 3 cells; active draft replaces it without duplication.
- [ ] **TAB-03** Status/number switches never shift title start cell; include 100+ tabs to grow number width.
- [ ] **TAB-04** Busy→permission/question uses !/? and pauses busy spinner; numbers mode stays numeric.
- [ ] **TAB-05** Root status includes child busy/attention, excludes child-only unread; permission outranks question.
- [ ] **TAB-06** Background user enqueue increments flash even when already busy; synthetic parked entries don't mark busy.
- [ ] **TAB-07** Selected/hovered/dragged/unread/error/renaming colors and text attributes match.
- [ ] **TAB-08** Four-cell title fade and grapheme-aware width handle CJK, emoji ZWJ, combining accents, Tamil, and long paths.
- [ ] **TAB-09** Marquee waits 600 ms, advances 80 ms, stops after one cycle, resets on leave/fit.
- [ ] **TAB-10** Expanded entries 2 rows+gap 1; compact entries 1+gap 1; detail project/worktree rules match.
- [ ] **TAB-11** 5-column compact golden: search row 1, items 3/5/7, plus 9, transcript column 5; half-block colors correct.
- [ ] **TAB-12** Compact tooltip 4 rows/max 54 cols, clamp on bottom edge, hide while dragging/menu.
- [ ] **TAB-13** Click/middle-close/right-menu/hover-close each match; hidden close cell never closes.
- [ ] **TAB-14** Transcript selection release over plus/search/close does not trigger action.
- [ ] **TAB-15** Drag reorder preview persists once and suppresses synthetic release activation.
- [ ] **TAB-16** Horizontal repeated close retains pointer's close cell through overflow change; leave/5s releases.
- [ ] **TAB-17** Active tab reveal happens after new layout and respects viewport during long vertical lists.
- [ ] **TAB-18** Context menu placement, entries, hover, outside dismissal and up-to 10 recent list match.
- [ ] **TAB-19** Close-active chooses MRU then right/left fallback; close-background keeps active; reopen original position.
- [ ] **TAB-20** Tab order/scope survive restart; recent-closed/history remain in-memory by design.
- [ ] **TAB-21** Focus-confirmed view acknowledgement and retry 250 ms→5s reproduce unread transition.
- [ ] **TAB-22** Async/shared-store mutations cannot resurrect a closed tab or alternate title fallbacks.
- [ ] **TAB-23** All default and customized tab/sidebar/focus shortcuts dispatch same commands.
- [ ] **TAB-24** Reduced-animation state preserves statuses/attention and dims pending title; deterministic clock captures full-motion phases.

### Sidebar fixtures

- [ ] **SIDE-01** Width 42, outer padding 2/2/1/1, title inset, body gap 1, pinned footer and absolute scrollbar match.
- [ ] **SIDE-02** Visible default section order Context→Limit→MCP→Todo; hidden contributions leave no artificial blank section.
- [ ] **SIDE-03** No usage/no cost hides Context; cost-only, tokens-only, unknown-model-limit, revert and cached-token cases match.
- [ ] **SIDE-04** Usage percentage includes correct five token classes and uses last eligible assistant's model.
- [ ] **SIDE-05** Limit hidden/unlimited/positive/reached/out states and text colors match separately from context.
- [ ] **SIDE-06** MCP 0/1/2/3 servers trigger correct section/disclosure behavior; collapsed active/error counts correct.
- [ ] **SIDE-07** MCP pending/connected/disabled/failed/needs_auth map to exact label/dot/name/status styles.
- [ ] **SIDE-08** Long MCP name truncates before fixed status; row click selects server and failed details.
- [ ] **SIDE-09** MCP heading expanded state persists; no invented row hover treatment.
- [ ] **SIDE-10** Todo takes latest completed list before revert, wraps body, renders markers/styles and remaining count.
- [ ] **SIDE-11** MCP/Todo <=2 remain visible even if persisted collapsed from an earlier larger list.
- [ ] **SIDE-12** Onboarding at height 21/22, integrations loading/connected/disconnected, dismissed/undismissed matches.
- [ ] **SIDE-13** Footer path max 38, branch suffix, UNIX/Windows/UNC/CJK/emoji basename truncation matches.
- [ ] **SIDE-14** Footer hover/copy/open/workspaces actions match; text selection prevents opening menu.
- [ ] **SIDE-15** LSP/changed-files additions are labeled extension fixtures, never silently counted as project-pinned v2 default parity.

## 12. Source map

All links below are immutable. Listed source paths are relative to the upstream repository.

- **[L01]** `packages/tui/src/plugin/builtins.ts:1–35` — [built-in registration order](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/plugin/builtins.ts#L1-L35).
- **[L02]** `packages/tui/src/routes/session/sidebar.tsx:13–80` — [sidebar shell](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/sidebar.tsx#L13-L80).
- **[L03]** `packages/tui/src/ui/layout.ts:1–26` — [layout constants and clamps](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/layout.ts#L1-L26).
- **[L04]** `packages/tui/src/app.tsx:538–623,1402–1481` — [rail sizing, visibility, MCP alerts](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/app.tsx#L538-L623), [root render tree](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/app.tsx#L1402-L1481).
- **[L05]** `packages/tui/src/component/session-frame.tsx:29–408` — [session frame](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/session-frame.tsx#L29-L408).
- **[L06]** `packages/tui/src/routes/session/index.tsx:255–260,1510–1667,1680` — [sidebar Escape and width](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/index.tsx#L255-L260), [session route render](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/index.tsx#L1510-L1680).
- **[L07]** `packages/tui/src/context/panel.tsx:11–42` — [panel presentation state](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/context/panel.tsx#L11-L42).
- **[L08]** `packages/tui/src/ui/pane-resize.ts:4–76`, `pane-resize-handle.tsx:4–33` — [resize controller](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/pane-resize.ts#L4-L76), [handle](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/pane-resize-handle.tsx#L4-L33).
- **[L09]** `packages/tui/src/util/scroll.ts:10–29` — [scroll behavior](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/util/scroll.ts#L10-L29).
- **[L10]** `packages/tui/src/context/session-tabs.tsx:35–498` — [tab controller, state, focus, effects](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/context/session-tabs.tsx#L35-L498).
- **[L11]** `packages/tui/src/routes/home.tsx:81–176` — [home geometry and update notification](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/home.tsx#L81-L176).
- **[L12]** `packages/tui/src/feature-plugins/home/footer.tsx:7–116` — [responsive home footer](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/feature-plugins/home/footer.tsx#L7-L116).
- **[L13]** `packages/tui/src/component/session-tabs.tsx:1286–1801` — [horizontal tab implementation](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/session-tabs.tsx#L1286-L1801).
- **[L14]** `packages/tui/src/context/session-tabs-model.ts:1–261` — [pure tab algorithms](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/context/session-tabs-model.ts#L1-L261).
- **[L15]** `packages/tui/test/context/session-tabs-model.test.ts:246–333` — [width fixtures](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/test/context/session-tabs-model.test.ts#L246-L333).
- **[L16]** `packages/tui/src/component/session-tabs.tsx:57–502` — [status, fade, marquee, context menu](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/session-tabs.tsx#L57-L502).
- **[L17]** `packages/tui/test/component/session-tabs-mouse.test.tsx:13–216` — [mouse behavior regression tests](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/test/component/session-tabs-mouse.test.tsx#L13-L216).
- **[L18]** `packages/tui/src/component/session-tabs.tsx:542–1284` — [vertical and compact tab implementation](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/session-tabs.tsx#L542-L1284).
- **[L19]** `packages/tui/src/component/session-tabs-rail.tsx:8–94` — [rail search and half-row primitives](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/session-tabs-rail.tsx#L8-L94).
- **[L20]** `packages/tui/test/component/session-tabs-rail.test.tsx:17–125` — [compact renderer fixture](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/test/component/session-tabs-rail.test.tsx#L17-L125).
- **[L21]** `packages/tui/src/config/index.tsx:185–203,290–338` — [config schema](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/config/index.tsx#L185-L203), [resolved defaults](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/config/index.tsx#L290-L338).
- **[L22]** `packages/tui/src/config/keybind.ts:96–120,152–161` — [pane/tab commands](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/config/keybind.ts#L96-L120), [tab direct selection](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/config/keybind.ts#L152-L161).
- **[L23]** `packages/tui/src/feature-plugins/sidebar/context.tsx:10–52`, `util/session.ts:46–66` — [context rendering](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/feature-plugins/sidebar/context.tsx#L10-L52), [usage computation](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/util/session.ts#L46-L66).
- **[L24]** `packages/tui/src/feature-plugins/sidebar/limit.tsx:7–47` — [workspace usage limit](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/feature-plugins/sidebar/limit.tsx#L7-L47).
- **[L25]** `packages/tui/src/feature-plugins/sidebar/mcp.tsx:5–103` — [MCP section](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/feature-plugins/sidebar/mcp.tsx#L5-L103).
- **[L26]** `packages/tui/src/feature-plugins/sidebar/todo.tsx:6–58`, `component/todo-item.tsx:5–27`, `util/todo.ts:25–43` — [todo section](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/feature-plugins/sidebar/todo.tsx#L6-L58), [todo item style](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/todo-item.tsx#L5-L27), [todo data/markers](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/util/todo.ts#L25-L43).
- **[L27]** `packages/tui/src/feature-plugins/sidebar/footer.tsx:9–126` — [onboarding and path footer](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/feature-plugins/sidebar/footer.tsx#L9-L126).
- **[L28]** `packages/tui/src/ui/file-path.tsx:14–112`, `working-directory-actions.tsx:9–66` — [path shortening](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/file-path.tsx#L14-L112), [directory menu](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/working-directory-actions.tsx#L9-L66).
- **[L29]** `packages/tui/src/component/tab-pulse.tsx:32–121,421–455,632–637` — [motion constants and blending](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/tab-pulse.tsx#L32-L121), [live renderer construction](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/tab-pulse.tsx#L421-L455), [frame update](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/tab-pulse.tsx#L632-L637).

Additional regression suites to port or adapt: `test/ui/layout.test.ts`, `test/context/session-tabs.test.tsx`, `test/component/session-tabs-status.test.tsx`, `test/component/session-tabs-marquee.test.ts`, `test/feature-plugins/sidebar-context.test.tsx`, `test/feature-plugins/sidebar-footer.test.tsx`, `test/session-panel.test.tsx`, `test/panel.test.ts`.

### Suggested upstream validation commands

Run from `packages/tui` of the pinned checkout after its matching dependencies are installed. These commands are proposed groups, not a claim that this audit ran them. The repository forbids running tests from its root.

```bash
bun test test/ui/layout.test.ts test/context/session-tabs-model.test.ts test/panel.test.ts
bun test test/component/session-tabs-marquee.test.ts test/component/session-tabs-status.test.tsx test/component/session-tabs-rail.test.tsx test/component/session-tabs-mouse.test.tsx
bun test test/feature-plugins/sidebar-context.test.tsx test/feature-plugins/sidebar-footer.test.tsx
bun test test/context/session-tabs.test.tsx test/session-panel.test.tsx
```

For layout/tab/sidebar behavior, the authoritative fixtures are chiefly **inline assertions in these test files**, not external `.snap` files. Renderer tests use `testRender`, `waitForFrame`, `captureCharFrame`, and sometimes `captureSpans` to check glyph position and attributes. A Rust parity harness should additionally capture all foreground/background values and clipping rectangles. The pure-model width matrix is in `test/context/session-tabs-model.test.ts:252–331`; exact compact glyph rows and bold attribute are in `test/component/session-tabs-rail.test.tsx:64–116`; status/menu assertions are in `test/component/session-tabs-status.test.tsx`; focus-safe panel transitions are in `test/session-panel.test.tsx`.

## 13. Verified difference from release tag v2.2.0

The first pass inspected release tag `v2.2.0`; the project instructions then established `450e7b92412c780a7b5a75226bb8d49aaca24e22` as the authoritative target. Every source/test file referenced by this layout report was compared using Git between those two revisions. Main-source links above now target the project pin. All explicitly declared geometry, sidebar composition, tab layout mathematics, mouse rules, theme role usage, and visibility thresholds described here are identical in the two revisions. Runtime layout/paint should still be tested against each version's pinned OpenTUI dependency: the project pin uses 0.6.8 and v2.2.0 uses 0.7.0. The relevant source differences are narrowly scoped:

| File | Project-selected commit | Later v2.2.0 tag | Port consequence |
| --- | --- | --- | --- |
| `src/component/tab-pulse.tsx` | A live render node advances pulse state every renderer frame using `deltaTime` | Uses the renderer clock, 50 ms stepping, scheduled redraw fallback, and cleanup of its timer; no continuously live node | Match project-pin frame behavior first. A later 20 fps optimization is an explicit upgrade, not baseline behavior. Timing/easing constants are otherwise unchanged. |
| `src/context/session-tabs.tsx` | Session-retention helper imported from `./session-retention` | Helper imported from `@opencode/client/solid` | Import move only within this file; geometry/status rendering is unchanged. |
| `src/routes/session/index.tsx` | Session-turn helpers imported from local `rows` | Helpers accessed via `SessionTurn` from shared utility | Main shell has unchanged markup; its line numbers shift by 2. Assistant timing/footer computation is outside this layout report's scope. |
| `test/component/session-tabs-rail.test.tsx` | Also creates 40 tabs, activates tab 40, and requires its ordinal to become visible | This final 5-line assertion is absent | Retain the project's last-tab reveal regression test. |
| `test/context/session-tabs.test.tsx` | Explicit 20-line test of default current-working-directory persistence | This test is absent | Retain that persistence regression test. |

The unchanged comparison set includes `app.tsx`, `ui/layout.ts`, `component/session-frame.tsx`, `routes/home.tsx`, `routes/session/sidebar.tsx`, `component/session-tabs.tsx`, `component/session-tabs-rail.tsx`, `context/session-tabs-model.ts`, `context/panel.tsx`, all sidebar/home feature components, resize helpers, FilePath and directory actions, scroll utility, todo component/data helpers, built-in registration, config/default keys, and the other referenced model/renderer/sidebar/panel test files.

Reproducible comparison command, from a checkout containing both objects:

```bash
git diff 450e7b92412c780a7b5a75226bb8d49aaca24e22 1097e2668b1e08ad63bdf0b1c4860b9bc01e420c -- packages/tui/src/component/tab-pulse.tsx packages/tui/src/context/session-tabs.tsx packages/tui/src/routes/session/index.tsx packages/tui/test/component/session-tabs-rail.test.tsx packages/tui/test/context/session-tabs.test.tsx
```

Evidence: [project-pin pulse driver](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/tab-pulse.tsx#L632-L637), [v2.2.0 pulse driver](https://github.com/anomalyco/opencode/blob/1097e2668b1e08ad63bdf0b1c4860b9bc01e420c/packages/tui/src/component/tab-pulse.tsx#L645-L683), [project-pin 40-tab regression](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/test/component/session-tabs-rail.test.tsx#L118-L121), [project-pin cwd persistence regression](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/test/context/session-tabs.test.tsx#L356-L374).

[L01]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/plugin/builtins.ts#L1-L35
[L02]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/sidebar.tsx#L13-L80
[L03]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/layout.ts#L1-L26
[L04]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/app.tsx#L538-L623
[L05]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/session-frame.tsx#L29-L408
[L06]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/index.tsx#L255-L260
[L07]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/context/panel.tsx#L11-L42
[L08]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/pane-resize.ts#L4-L76
[L09]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/util/scroll.ts#L10-L29
[L10]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/context/session-tabs.tsx#L35-L498
[L11]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/home.tsx#L81-L176
[L12]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/feature-plugins/home/footer.tsx#L7-L116
[L13]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/session-tabs.tsx#L1286-L1801
[L14]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/context/session-tabs-model.ts#L1-L261
[L15]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/test/context/session-tabs-model.test.ts#L246-L333
[L16]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/session-tabs.tsx#L57-L502
[L17]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/test/component/session-tabs-mouse.test.tsx#L13-L216
[L18]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/session-tabs.tsx#L542-L1284
[L19]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/session-tabs-rail.tsx#L8-L94
[L20]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/test/component/session-tabs-rail.test.tsx#L17-L125
[L21]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/config/index.tsx#L185-L203
[L22]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/config/keybind.ts#L96-L120
[L23]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/feature-plugins/sidebar/context.tsx#L10-L52
[L24]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/feature-plugins/sidebar/limit.tsx#L7-L47
[L25]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/feature-plugins/sidebar/mcp.tsx#L5-L103
[L26]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/feature-plugins/sidebar/todo.tsx#L6-L58
[L27]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/feature-plugins/sidebar/footer.tsx#L9-L126
[L28]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/file-path.tsx#L14-L112
[L29]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/tab-pulse.tsx#L32-L121
