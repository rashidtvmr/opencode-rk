# OpenCode v2 project pin: transcript, tool, Markdown, and interrupt-surface audit

## Baseline and meaning of this report

This is a source audit of `anomalyco/opencode`, project-selected commit `450e7b92412c780a7b5a75226bb8d49aaca24e22`, compared with tag `v2.2.0` at `1097e2668b1e08ad63bdf0b1c4860b9bc01e420c`. The selected commit is the reproduction target required by the Rust project; the release tag is a comparison baseline, not a substitute target. It describes the full-screen TUI in `packages/tui`, not the web/desktop application and not the separate `mini` TUI. All measurements below are terminal cells or rows, not CSS pixels. Padding is relative to the component's immediate parent; the session shell adds further inset. Observed rules come from the pinned source. Proposed Rust structures and tests are recommendations, not claims that the user's bridge already implements them. No upstream test suite or Rust application was executed for this audit.

The UI is a stateful transcript projection. A faithful port must reproduce grouping, content arrival, click targets, focus, expansion, scrolling, and color attributes as well as the final static text. A flat vector of preformatted strings cannot reproduce its behavior without separately rebuilding these facilities.

### Checked release delta

The selected and release snapshots have the same inspected message components, grouping UI, tool renderers, permission/form surfaces, Markdown handlers, spinner, shimmer, patch component, and Mermaid/LaTeX implementation. In the selected snapshot, assistant duration/throughput helpers are local to `routes/session/rows.ts:427–482` and duration formatting is local to `util/locale.ts:29–50`; v2.2.0 moves those into `@opencode/util` without changing the inspected footer geometry. `index.tsx` gains 2 lines before the footer and 5 after it in the selected pin; this report's immutable source anchors have been adjusted accordingly. Tab pulse scheduling differs between snapshots and belongs to the navigation audit; do not import v2.2.0's timer policy into the selected pin merely because other animation files match. Mermaid and LaTeX package version metadata differs, but inspected rendering source is identical.

Selected-pin duration is elapsed time from the appropriate input to the completed assistant step. Legacy sessions find the nearest input; marker-era sessions find the first input after the previous idle marker, so steers remain in the same logical turn. Throughput sums output plus reasoning tokens over the corresponding assistant steps and divides by the sum of provider stream durations, excluding tool time; it is absent if stream boundaries are missing. Format below 1 second as milliseconds, below 1 minute with one decimal seconds, then minutes/seconds, hours/minutes, or days/hours. See [selected turn helpers][M35].

## 1. Transcript projection and grouping

Sources: [row projection][M01], [grouping rules][M02], [group rendering][M03], [session defaults][M04].

### Observed production rules

- The default detail level is `medium`. The session defaults are `thinking: hide`, Markdown `rendered`, diff wrapping `word`, and related-tool grouping enabled unless `session.grouping` equals `none`.
- Transcript rows are not equivalent to chat messages. The row union includes a complete non-assistant message, an assistant part reference, an assistant footer, a queued-compaction divider, a debug turn-usage row, and recursive groups. Assistant text, reasoning, and tools therefore occupy separate stable rows.
- Group kinds are `reasoning`, `exploration`, `instructions`, and low-verbosity `activity`.
- Consecutive reasoning parts group. Exploration includes exactly `read`, `glob`, `grep`, `webfetch`, and `websearch` in the grouping classifier. Questions remain standalone. Instruction notices can group by distinct loaded file paths.
- Low verbosity wraps runs of non-text parts in an activity summary, including nested reasoning/exploration groups. It counts finished work; before anything finishes, it shows a stable status such as “Running command…” or “Preparing code…”. Code-mode execution counts its completed nested calls, with a fallback to counting the execute tool itself when there are none. Permission-blocked parts are excluded from the activity summary and remain visible below the collapsed group.
- High verbosity opens exploration and instruction groups by default. It does not automatically expand reasoning. An explicitly saved group expansion overrides the default.
- Group labels change from active to completed: exploration summarizes reads, searches, and fetches; reasoning uses the latest available summary title, completed thought count, and summed duration. Running summaries use spinners. Completed disclosure icons remain clickable even when a tool inside failed.
- The group disclosure color changes on hover. Text selection takes precedence: a mouse-up with selected text must not expand/collapse a group.
- Every outer `SessionRowView` has `marginTop=1` and `flexShrink=0`. This is a row-level rhythm, not a blank row blindly inserted before every nested tool. Expanded low activity uses `gap=1`, `marginTop=1`; expanded instruction items remain adjacent to the summary and to each other.
- Stable row keys, group IDs, and part anchors preserve expansion and scroll position when older history is prepended, streaming updates arrive, or a connection resynchronizes. A page boundary that lands within a group triggers more backfill until the group's beginning is known; otherwise its ID and summary would change after expansion.

### Rust port guidance

Use a typed transcript projection layer separate from widgets: `MessageStore -> TranscriptReducer -> Vec<TranscriptRow>`. Identify a tool/part by `(message_id, part_id)`, not `part_id` alone. Persist group disclosure in session-local UI state. The renderer needs an anchor map from part/group identity to current geometry, including offscreen/mounted behavior. Preserve the visible anchor offset when expansion, historical backfill, or wrapping changes height.

Do not update all rows by position after inserting history. That loses local hover/expanded state and remounts Markdown. Do not hide a permission-requesting tool inside a collapsed summary.

## 2. Message geometry, surfaces, and actions

Sources: [row and assistant footer][M05], [user messages and pending dock][M06], [message menu][M07], [split border][M08].

| Element | Source-derived geometry and behavior |
|---|---|
| Assistant text | `paddingLeft=3`, `flexShrink=0`; uses the base surface. Empty/whitespace-only text is absent. |
| User message | Raised base background; a left `┃` split border. Border uses current session agent color, or ordinary border color while pending delivery. Content interior has top/bottom padding 1 and left padding 2. |
| User hover | Background becomes `theme.decrease(theme.background.raised.base)`. The entire content block is interactive. |
| User click | If text is selected, do nothing. A pending steer opens “Move to queue” / “Delete”. Otherwise open Message Actions resolved by message ID, not a stale row object. |
| Message Actions | Jump to, Revert, Copy, Fork. Revert repopulates the composer from the projected user input before staging the revert. Copy uses actual textual content. |
| User skills/files | Chip rows below text have top padding 1, gap 1, and wrap. Prefix chip is `skill`, `file`, or `dir`; filename/name chip follows. |
| Queued prompt dock | Margin bottom 1, left split border, raised body; top/bottom padding 1, left 2/right 1. One-line, truncated count plus flattened next prompt. |
| Revert card | Margin top 1, left split border, raised body and same hover transform. Shows reverted-message count, changed files, per-file added/deleted counts, redo shortcut. |
| Assistant footer | Left padding 3. Agent name uses agent color unless message errored. Model text is muted and appears only when terminal width is at least 28. Duration is shown below 28 or at least 36, deliberately omitted at widths 28–35. Optional `tok/s` is one decimal. Interrupted responses append an interrupted label. |
| Switch notice | Left padding 3. Agent/model/variant notices use muted text; a location switch uses muted “Moved to” plus info-colored directory. |

The left border is specifically `┃`, with other custom border characters blank. A rounded rectangle around every message would be visibly different. Also do not add full-width colored backgrounds to ordinary assistant prose: it explicitly uses the base background.

## 3. Colored Markdown, reasoning, and special fenced blocks

Sources: [Markdown and reasoning components][M09], [thinking summary parsing][M10], [syntax recoloring][M11], [plugin renderer identity][M12], [built-in registrations][M13], [Mermaid][M14], [LaTeX][M15].

### Assistant Markdown

The assistant component renders the trimmed text with the current syntax style, plugin code-block handlers, `internalBlockMode="top-level"`, grid-style tables, horizontal table-cell padding 1, `theme.markdown.text` as foreground, and `theme.background.base` as background. Streaming is true until the assistant message has a completion timestamp. `conceal` follows rendered/source Markdown mode. The order of applying content and streaming matters: source explicitly applies content before streaming completion so the final code-fence token is not lost.

Implementing only ANSI-colored plain text or a Markdown-to-string pass is insufficient. The Rust bridge must expose, or the Rust application must supply, block parsing, code-fence language highlighting, tables, inline styling, wrapping, selection, and incremental updates with stable block identity. Exact OpenTUI rendering decisions must come from its pinned version, rather than an arbitrary Rust Markdown renderer. The choice of `pulldown-cmark`, `comrak`, `syntect`, or tree-sitter is an implementation choice; their output is not automatically equivalent.

Plugin code-block mappings are combined with last-wins behavior. The render-node callback is memoized using shallow equality of the effective map because changing it destroys/rebuilds all Markdown blocks. Unrelated plugin changes must leave the existing Markdown nodes mounted. A changed or removed handler must update existing messages.

### Reasoning

Reasoning is not colored like the answer. It receives a cloned syntax style with every registered foreground replaced by muted text, preserving the rest of each style. `[REDACTED]` placeholder content is removed and trimmed; empty results do not render.

Hidden thinking is a collapsed disclosure by default. A leading complete `**Title**` block separated from body by a blank line (or a completed title awaiting body) can become the header summary; ordinary initial bold words followed by more paragraph text are not incorrectly consumed as a title. Running headers show “Thinking” plus an optional title. Completed headers show “Thought”, optional title/duration, and disclosure signs. Warning color is used for thought headers; some expanded/completed states use the same color at 60% alpha.

The standalone reasoning wrapper has left padding 3. Expanded/shown reasoning uses a left split border and padding 1 for its header. The expanded body has margin top 1, another left border, and body padding 3 in minimal/hide mode or 1 in show mode. Grouped expanded thought entries use a separate left-border layout in `group-view.tsx`; do not force both routes through one hard-coded indent.

### Mermaid and LaTeX are part of the v2 surface

The built-in plugin list registers terminal Mermaid and LaTeX renderers. A full parity checklist must cover them or explicitly exclude them from its target.

- Mermaid is registered for the `mermaid` fence. Supported detection in this commit covers flowchart, Gantt, gitGraph, sequence, state, and timeline. Do not assume every web Mermaid diagram type is supported.
- Mermaid diagrams render terminal styled text with height derived from the diagram, full available width, no wrapping, non-selectable content, and top margin 1. Horizontal drag changes `scrollX`; it clears text selection and captures the drag. Compact diagram spacing is the default; responsive horizontal layouts use a default maximum of 120 columns unless otherwise configured. Streaming syntax errors can retain a previous valid diagram or render a valid prefix; unsupported/oversized diagrams fall back rather than crashing the transcript.
- LaTeX registers both `latex` and `math` fenced blocks. Successful math is rendered as cells with baseline text/subdued structural symbols and bold/italic/dim attributes. The viewport has top margin 1, horizontal-only scrolling, no visible scrollbars, and non-selectable formula content.
- During an incomplete append, LaTeX retains a previous good layout only if the new text extends its source. Unsupported commands, malformed/oversized input, and unresolved parsing fall back to the original code block so the source is not lost.

## 4. Reusable tool-row primitives

Sources: [InlineToolRow][M16], [tool wrapper and block wrapper][M17], [name/wire-data normalization][M18].

### Inline row

`InlineToolRow` has left padding 3. A completed icon occupies a fixed 2-cell column. The textual label grows beside it, so wrapped continuation lines align under the label, not the icon. Expanded failure details add another 2-cell padding and align under the label.

The status-bearing form uses a wrapping row with `columnGap=1`. The label is at most 100% width and does not shrink; a status such as the padded Background badge wraps as one item. The source has explicit tests at 70 and 62 columns to catch a badge splitting or losing its leading space.

Do not equate the prop named `complete` with successful execution. Several tool views pass “input field exists” to that prop. The actual lifecycle is the tool state; this prop selects available label vs pending text. Render a failure with a useful completed label if it exists, or a failure label when the tool failed before input became available.

`InlineTool` color precedence is: explicit color; permission warning; non-denial failure error; clickable hover base text; otherwise muted text. Tool permission attention is suppressed in autoaccept mode. A denial/dismissal is detected separately from ordinary failures and gets strikethrough; it does not become a conventional red expandable error. Source recognizes rejected questions, rejected permission, a specified permission rule, and user dismissal.

Clicking while selecting text is ignored. A failed tool opens its error details or its custom error action; otherwise it invokes its normal action. Failure details are ANSI-stripped. Errors do not replace the entire card with an unrelated generic dialog.

### Block row

`BlockTool` has a left split border colored with base background, raised base surface, top/bottom padding 1, left padding 2, gap 1, and `flexShrink=0`. It applies the raised-surface decrease transform only when it has a click action and is hovered. Headers are muted unless permission requires warning color or the renderer supplies a specific color. A spinner replaces the header marker during active states. Path headers have a one-cell gap and a width-aware `FilePath` component; filename truncation must leave room for the label and spinner.

`StatusBadge` is an indivisible muted text item with exactly one leading and trailing space, on a decreased base surface (or decreased raised surface when inside a block).

### Dispatch

The display registry handles shell, glob, read, grep, webfetch, websearch, write, edit, subagent, execute, patch, todowrite, question, and skill. Legacy aliases normalize `bash -> shell`, `task -> subagent`, `apply_patch -> patch`. All unknown/plugin/MCP names use the generic renderer. Do not fabricate a special MCP transcript visual without matching a real registered renderer.

Generic summaries include primitive string/number/boolean arguments in a compact bracket list and collapse all whitespace to a single line. Nested objects remain hidden until expansion. Expanded generic details use left padding 5, muted keys, base-colored word-wrapped values, pretty JSON for non-string values, and an optional `output:` row. Running/streaming state spins; completed shows a tick; error shows a cross. Terminal output is ANSI-stripped.

## 5. Tool-specific inventory

Sources: [shell/write/search/read][M19], [subagents and execute][M20], [edit/patch/todo/question][M21], [shell truncation][M22].

| Renderer | Required visible content and state behavior |
|---|---|
| Shell | Raised block. Uses actual command, optional `cd <formatted workdir> &&` prefix. `$ ` appears when no longer running; running uses spinner plus label, one-cell gap, and non-shrinking spinner. “Writing command…” until command exists. Command is character-wrapped with maximum 2 rows when collapsed, word-wrapped without that cap when expanded. Output uses muted text. |
| Shell output collapse | Shared 10-line budget for input/output, not 10 lines each. Character budget is `10 * max(20, content_width - 6 - (running ? 2 : 0))`. Output collapse preserves the tail and prepends an earlier-line-count label. Command collapse handles graphemes/display width. |
| Shell background | Has padded Background badge. While background output is collapsed it is not polled unnecessarily. Expanded live background output polls at 1 second; completion drains the remaining output. Output retention is capped at 1 MiB with earlier-output-omitted notice. The process's original location remains authoritative even if the session moves. |
| Write | Pending inline `← Write <path>` / preparation state; completed raised `# Wrote <path>` block, syntax-highlighted code with line-number minimum width 3 and right padding 1, then diagnostics. |
| Read | `→ Read <path>`, spinner while running; optional `:start-end` line range. Loaded instruction files appear below with an additional indent and `↳ Loaded` label. |
| Glob/Grep | `✱`, pattern in quotes, optional formatted directory and match count. Pending copy differs for finding files vs searching contents. |
| WebFetch | `%` icon, URL label; pending fetching state. |
| WebSearch | `◈`, query and optional named provider. Provider retry animation is a specialized `RetryProvider`, not a general message spinner. |
| Subagent | See next section: lifecycle is child-session-aware; click navigates to child; explicit model override only. |
| Execute | Inline root with loader/tick/cross plus child call metadata rows, and live detail dialog. Successful output stays in details; error output gets a compact inline preview. |
| Edit | Raised file path block; structured patch when metadata provides one; preparation block otherwise. Optional syntax, colored changes, line numbers, diagnostics. |
| Patch | One raised block per changed file, with a one-row gap. Labels distinguish Created, Deleted, Patched. Deleted file shows a removed-line count rather than a code diff. Metadata with only applied resources has its own fallback; streaming can derive target filenames from patch text. |
| TodoWrite | Completed nonempty tasks use a `# Todos` block and `TodoItem`; empty completed list says Cleared todos; failure says Todo update failed. |
| Question tool result | A `# Questions` block with each question muted and answer base-colored; questions separated by gap 1; unanswered entries use an explicit no-answer label. Pending state reports question count. Interactive forms are a distinct surface. |
| Skill | Inline arrow with skill name; loading placeholder until its name is known. |
| Diagnostics | Only severity 1 entries with valid finite line/character and nonempty messages, matching normalized file path; maximum first 3. Display 1-based line/column as `Error [line:column]`. |

## 6. Subagent behavior and code-mode execution

Sources: [transcript subagents/execute][M20], [execute details][M23], [background completion notices][M24].

### Subagents

A subagent is active if its tool state is running **or** its child session is running. Ending the initial tool call therefore does not necessarily end its spinner/status. A new running subagent uses a spinner. Continuation calls use `↳` and a Continue subagent label; they deliberately do not use that new-subagent spinner path. New labels include title-cased agent type and description. A model suffix appears only for an explicit model override, resolves provider/model to a display name, and renders `#variant` as a parenthetical variant. Unknown models retain the supplied reference.

The Background badge is true only when the tool has completed while metadata status is still running. A foreground running tool must not carry that badge. Clicking navigates to the child session when its ID is available. This is an indeterminate activity indicator; no transcript percentage calculation exists here. Inventing numeric progress would fail exact parity.

Later synthetic completion notices distinguish finished, failed, and cancelled work. Finished uses info color, failed error, cancelled warning; child-session hover uses base text. Descriptions are width-truncated. Shell completion text collapses whitespace. Child-session click remains available and respects text selection.

### Execute / custom script

The source explicitly says execute child calls stream through metadata, not a child session. Preserve `metadata.toolCalls` as an ordered live collection of tool, running/completed/error status, and input. Invalid child entries are filtered at the boundary.

The root execute row always has click and error-click actions opening details. Its spinner covers streaming or running. Runtime failure can be represented by `metadata.error === true` even when the transport tool completed; use both. The child rows have left padding 3, a fixed 2-cell icon (`›` or error cross), a one-line truncated compact argument label, hover base text, and independent expansion. Expanded input is under a left border with padding 1. Expanding a child does not open a subagent session.

Inline output preview appears only for runtime errors and is capped at 4 lines / `4 * max(20, content_width - 6)` characters. The first line has an arrow; following lines preserve aligned indentation. Successful logs should not flood the main transcript.

The detail dialog is centered and xlarge. Its maximum body height is `max(3, floor(terminal_height * 0.7) - 6)`. It uses one scrollable body for Code and Output, a high raised background, hidden scrollbar, top/bottom padding 1, and a shared gutter width. Code is TypeScript; valid JSON output is highlighted as JSON; trailing Warnings/Logs text is rendered below at the same content column. Source and output line-number gutters align even when they have different line counts, and wrapped continuation rows are unnumbered.

It distinguishes receiving code, running, completed, and failed with duration. Missing code/output and completed no-output have separate placeholders. Arrow/page/home/end navigation works inside the detail view. `c` copies code and `o` copies output, replacing that action label with a success-colored copied state. The dialog observes the live tool object while mounted. The entry action captures a stable tool object before a session resync can drop the original row.

## 7. Diffs, large files, images, and boundaries

Sources: [inline diff wiring][M21], [PatchDiff][M25], [dedicated diff viewer][M26], [image previews][M27].

Inline edit/patch view selection respects explicit unified/split. In auto mode it switches only when **content width > 120**. This is not terminal width, and it is not the same threshold as the dedicated diff viewer. Permission edit preview instead consults terminal width > 120. Test each route separately.

All inline diff colors are semantic: hunk-header text; added/removed/context backgrounds; added/removed sign highlight; line-number text; context/added/removed line-number backgrounds. Preserve syntax highlighting on top of the change backgrounds. Inline diff body has one additional left cell inside its block card.

`PatchDiff` splits hunks and reserves a shared gutter digit width across them and across split sides. Each subsequent hunk has a one-row colored header. Its gutter has one leading cell plus optional gutter padding. When supplied a scroll container and dealing with an added-file diff larger than 3,000 lines, it can use 128-line chunks, render only the viewport plus one chunk on each side, and leave exact-height placeholders offscreen. It highlights the whole file once and slices ranges into chunks so a fragment is not incorrectly parsed as an independent source file. This optimization is conditional: the inline callers shown do not pass a scroll container, so do not claim all inline patches automatically virtualize.

The dedicated diff viewer uses a different policy: its patch pane must be at least 100 cells for split view; otherwise it forces unified. File-tree width is clamped to 30–40 cells around `floor(view_width/4)`. Full layout uses 2-cell insets; docked or narrow presentation is compact, has no file tree, and uses the raised surface. Fullscreen uses base surface. Preserve this distinction when sharing Rust diff widgets.

Image preview is controlled by `session.image_preview`. Inline tool previews accept `data:image/` file content. User images are deduplicated before display. At most three thumbnails are visible; excess becomes a `+N more` tile. Thumbnail height is `clamp(floor(terminal_height/4), 4, 8)` and width/basis twice that height; images use cover fit and automatic protocol. Row gap is 1, padding top/bottom 1, left 3 by default or 2 for user cards, right 2. A failed thumbnail displays No preview. Left-click opens the gallery at that image and stops propagation so the enclosing card does not also activate.

## 8. Permissions and interactive forms

Sources: [permission source][M28], [permission presentation mapping][M29], [form source][M30].

These are composer-replacing interactive states, not ordinary assistant text. Preserve focus, active input scope, and restoration when a form/permission completes or becomes stale.

### Permission prompt

- Raised base surface and left split border using focused primary action background.
- Header: warning-colored triangle, base-colored Permission required, and a second action/title line except for shell requests. Content comes from typed permission action/resources plus tool input/metadata.
- Inner body: gap 1, left padding 1/right 3/top 1/bottom 1. Normal max height 15. Fullscreen uses a portal and absolute geometry inset left/right 2, with bottom 1.
- Footer: decreased raised background, top/bottom padding 1, left 2/right 3, gap 1. At terminal widths below 80 it becomes a vertical layout with left alignment; otherwise row + space-between + centered alignment.
- Options include allow-once and reject, with always available only when save rules exist. Selecting always shows the rules it will persist. Hover moves selected option; click selects and replies. Arrow-left/h and arrow-right/l cycle options; Enter confirms. Escape first minimizes fullscreen; otherwise rejects.
- A rejected child-session permission opens a rejection-reason editor; a normal parent permission rejects directly. Reject editor uses error border/triangle, same raised card geometry, enter confirm, escape cancel, and ctrl-c clears nonempty reason before cancelling.
- Edit permission content supports structured diffs, raw patch code fallback, and a no-diff placeholder. Its scrollbars and all diff colors follow the theme. This is another important Rust bridge capability beyond drawing boxes.

### Forms / MCP elicitation / questions

- Same raised card and focused-action left border. Body gap/padding matches the permission body. Title is muted; optional message is base-colored.
- Supports text, numbers/integers, boolean, single choice, multiselect, external URL fields, defaults, conditional fields, validation, and custom answers. Field visibility is recomputed in order from answers to earlier active fields. A single boolean/choice can use a one-step presentation; other forms have field navigation plus review/submit.
- Full tab strip is used only if the sum of labels (each truncated to 24 characters plus spacing) and Submit fits terminal width minus 8. Otherwise use a compact Field N of M / completed-count row. Tab hover is focused-formfield color; active tab is bold base text; inactive is muted.
- Text/custom-answer editors have height 1–6 rows, correct cursor style, placeholder color, and their own input mode. Enter/paste/typing behavior must not accidentally submit the form.
- Choice rows have ordinal plus right padding 1. Multiselect reserves a fixed 4-cell checkbox column so checked/unchecked states and wrapped labels align. Description indent is 7 for multiselect, 3 for single choice. Formfield focused/selected/base tokens are distinct from outcome feedback colors.
- Review uses a scrollbox with maximum height `max(3, terminal_height - 14)`, hidden scrollbar, and one-cell inset. It shows compact per-field values and marks invalid/required/missing external acknowledgements using error color. The scroll hint appears only if wrapped review content exceeds the available height.
- Footer has left padding 2/right 3/bottom 1, row/space-between, action gaps 2, and current validation error at the opposite side. Hints change with textual/choice/external/review/editor states.
- External fields require explicit acknowledgement after opening or copying their URL. Merely opening a browser must not mark the action done. URL, instructions, acknowledged success state, and copy shortcut are all visible states.
- Draft answers survive temporary unmount via a per-form draft store. Terminal replies/revalidation failure restore the composer; stale forms must not leave focus trapped.

## 9. Animation, errors, compaction, and runtime notices

Sources: [spinner wrapper][M31], [spinner renderable][M32], [shimmer][M33], [compaction/errors/retry][M34].

The standard tool spinner is the ten Braille-frame sequence in `spinner-frames.ts`, advancing at 80 ms. It occupies one fixed glyph cell; with a label there is one gap cell. A bare spinner cannot shrink, otherwise Yoga can round its fractional width to zero and paint it over the gap. Disabled animations show a static ellipsis glyph and the same label. Some other UI regions use specialized one-cell or subcell animations; do not globally replace them with this spinner.

The custom spinner uses the renderer clock, requests individual frames, and deliberately is not a continuously live renderable. It resets on frame/interval/autoplay changes, respects visibility/culling, and cancels its timer on destroy. A port that starts an endless timer for each historical spinner is both behaviorally and operationally wrong. Shimmer text is separately implemented with a 1,200 ms mask sweep and must preserve actual foreground/background cell composition.

Compaction is a horizontal divider with flexible top borders on either side of a padded title group. Running has an 80 ms spinner (or static fallback); failed non-cancelled has error cross/color; cancelled is muted with cancelled suffix. Completed native compaction can say Provider compaction. Usage here is the compaction request's input/cache and output/reasoning usage, not the resulting context size, which only appears on a later assistant step. Summary below uses the same rendered Markdown rules and left padding 3/top padding 1. A queued compaction is a distinct bordered divider with a diamond and queued label.

Execution errors appear at left padding 3 with error foreground. Interrupted assistant steps suppress the normal error block and append interrupted to the footer. Ordinary retries have a warning line with ceil-rounded seconds until retry time, attempt count, and actual error message; a passed deadline displays a due state. Free-limit error has a specialized reset-time notice and connect/models actions rather than the generic error. Optional debug turn-token rows are collapsed by default and show a per-step table only when enabled, so they must not be added unconditionally to the standard UI.

Long-running shell/subagent work may show a background-action shortcut hint after 3 seconds. Time it from the tool's start identity, retain it while that work remains the same, and remove it with that work; avoid flicker on metadata updates.

## 10. Acceptance checklist for the Rust implementation

Every item starts unchecked because source inspection is not runtime parity validation. Run these against captured upstream fixtures at the same version, terminal dimensions, theme/configuration, and injected clock. Compare characters **and** foreground/background RGBA, attributes, geometry, cursor/focus, and hit targets.

### Transcript and messages

- [ ] **MSG-001:** Default medium transcript groups thoughts and exploration, leaves questions standalone, and preserves one-row outer spacing without double gaps in nested groups.
- [ ] **MSG-002:** Low/medium/high and thinking show/hide reproduce correct default disclosures; changing one session does not corrupt another session's saved expansion.
- [ ] **MSG-003:** Pending permission tool remains visible outside collapsed groups; a same part/call ID reused in another message does not hide the wrong row.
- [ ] **MSG-004:** Expand/collapse, backfill, reconnect, tab switch, and width resize preserve stable anchors, selected content, and expansion. No jump to the transcript bottom when the user is reading history.
- [ ] **MSG-005:** User card content, one-cell `┃` border, agent/pending border colors, padding, hover transform, chips, and image gap match cell-for-cell.
- [ ] **MSG-006:** Text drag selection never opens message actions, group disclosure, tool details, or a child session on mouse-up.
- [ ] **MSG-007:** Message action menu correctly restores prompt on revert, copies message text, forks by current message ID, and tolerates a resync while open.
- [ ] **MSG-008:** Footer behavior at widths 27/28/35/36 is correct; agent/model/duration/TPS/interrupted states match.
- [ ] **MSG-010:** Duration and throughput cover legacy input boundaries and marker-era turns with steers, model switches, missing stream timestamps, and an absent assistant anchor; tool duration is excluded from throughput.
- [ ] **MSG-009:** Queue dock, pending-steer actions, revert card, location/model/agent notices, and child completion notices match in each state.

### Markdown and highlighting

- [ ] **MD-001:** Paragraphs, headings, bold, italic, strikethrough, inline code, blockquotes, ordered/unordered lists, nested lists, links, rules, fences, and grid tables match upstream cells and colors.
- [ ] **MD-002:** Unfinished fence, unfinished inline formatting, split Unicode/grapheme input, blank/whitespace chunks, and completion arriving before/after final content preserve all text.
- [ ] **MD-003:** Both final-content/completion event orders finish a streamed fence with exactly its complete content and one block; no stale initial text and no duplicate block.
- [ ] **MD-004:** Rendered/source toggle reproduces concealment and preserves selection/scroll anchors. Reflow on narrow width does not corrupt code/table geometry.
- [ ] **MD-005:** Theme switch recolors existing syntax and Markdown without losing text; reasoning uses muted foreground while retaining noncolor syntax attributes.
- [ ] **MD-006:** Valid bold first-block thinking titles become summary metadata; ordinary initial bold prose does not. Redacted-only reasoning renders nothing.
- [ ] **MD-007:** Unrelated plugin toggle preserves Markdown block identity; changed/removed fence handler updates existing blocks.
- [ ] **MD-008:** Each supported Mermaid kind, wide horizontal drag, streaming partial syntax, invalid syntax, oversized canvas, and fallback preserve geometry and source.
- [ ] **MD-009:** LaTeX/math fences retain last good partial layout only when source extends it; unsupported syntax falls back; long formulas scroll horizontally and do not steal vertical transcript scroll.

### Tools and subagents

- [ ] **TOOL-001:** All 14 specialized names plus unknown/plugin/MCP fallback dispatch correctly; legacy names map to shell/subagent/patch.
- [ ] **TOOL-002:** Streaming/running/completed/error/denied/permission-needed each have correct label, icon, spinner, color, strikethrough, and click behavior.
- [ ] **TOOL-003:** Wrapped labels start after fixed 2-cell icon column; expanded error text aligns to the label; Background badge wraps as one padded item at narrow widths.
- [ ] **TOOL-004:** Primitive generic arguments flatten to one line; nested JSON is only in expanded view; ANSI output does not leak escape sequences.
- [ ] **TOOL-005:** Shell command/output share a 10-line collapsed budget; command max 2 visual rows; output tail and earlier-line label stay correct for wide glyphs and multiline commands.
- [ ] **TOOL-006:** Background shell collapses without needless polling, live expansion updates at the correct cadence, completion drains output, large output is bounded, moved sessions retain original process location.
- [ ] **TOOL-007:** New subagent, continued subagent, running child after tool completion, completed, error, cancelled, and detached Background badge all match without invented percentage progress.
- [ ] **TOOL-008:** Subagent explicit model override and variant display match; missing override adds no model label; click follows the correct child session.
- [ ] **TOOL-009:** Execute root and metadata child rows update independently; metadata runtime error changes root color even when transport state is completed.
- [ ] **TOOL-010:** Execute child expansion reveals structured arguments; root/details action is distinct; success output stays out of inline preview; runtime errors show exactly the bounded preview.
- [ ] **TOOL-011:** Execute dialog live status, TypeScript/JSON syntax, matching gutters, wrapped continuation line numbers, output/logs, copy actions, and scroll keys match.
- [ ] **TOOL-012:** Read range and loaded-instruction rows, Glob/Grep counts, WebFetch/Search/provider names, skill names, todos, and question-result blocks match.

### Diffs and media

- [ ] **DIFF-001:** Inline auto split at content widths 120/121; permission auto split at terminal widths 120/121; dedicated viewer split availability at patch-pane widths 99/100.
- [ ] **DIFF-002:** Add/edit/delete/move, multiple hunks, long paths, 3-to-4-digit line numbers, wrap/source mode, and syntax colors align with all diff background/gutter roles.
- [ ] **DIFF-003:** Added-file virtualization only in eligible scroll-aware context; crossing 3,000 lines/chunk boundaries preserves source highlighting and exact placeholder heights.
- [ ] **DIFF-004:** Deleted-file count and applied-resource fallback appear without fabricated diff content; malformed wire metadata is filtered without a crash.
- [ ] **DIFF-005:** Only valid severity-1 diagnostics render, maximum 3, with normalized path and 1-based positions.
- [ ] **MEDIA-001:** Image preview off/on, 0/1/3/4 images, failed image, short/tall terminals, user/tool previews, and gallery click all match, including no click bubbling.

### Permission, forms, timing, and recovery

- [ ] **FORM-001:** Permission once/always/reject variants; missing save rules omit always; changing selected option changes the body; nested rejection reason is correctly scoped.
- [ ] **FORM-002:** Permission width 79/80, normal/fullscreen, Escape minimize/reject, ctrl-c clear/cancel, and edit-diff body preserve geometry and focus.
- [ ] **FORM-003:** Text/number/integer/boolean/single/multi/external/conditional/default fields, validation, review, and draft restoration behave identically.
- [ ] **FORM-004:** Narrow field fallback replaces tabs at the exact fit boundary; 24-character tab labels, checkbox width 4, description insets 3/7, and textarea height 1–6 are verified.
- [ ] **FORM-005:** Paste into selected custom choice opens editing without submit; paste into active editor inserts at cursor; burst typing is not lost; up leaves the editor only from first visual line.
- [ ] **FORM-006:** Space toggles multiselect but not single choice; optional empty multiselect omits the answer; validation waits until the matching submit point.
- [ ] **FORM-007:** External URL is not acknowledged by merely opening/copying; review errors/progress count external acknowledgement; wrapped review only offers scroll when needed.
- [ ] **FORM-008:** Terminal/stale/revalidation-failed forms restore composer focus and drafts; keyboard scope does not leak actions into the underlying transcript.
- [ ] **STATE-001:** Spinner clock at 0/79/80/160 ms, animations off, visibility/culling, frame-list change, destroy, and parent background composition match.
- [ ] **STATE-002:** Compaction queued/running/completed/provider-native/failed/cancelled and summary streaming match; request usage is not mislabeled as new context size.
- [ ] **STATE-003:** Retry countdown with zero/past/future deadline, ordinary error, interruption, free limit, and optional token diagnostics match.
- [ ] **STATE-004:** Background hint is absent before 3 seconds and appears for the same eligible long-running work, then disappears on completion/replacement.

## 11. Upstream tests to port or use as an oracle

These are existing source evidence, not tests run by this audit:

| Upstream file | Contract worth retaining |
|---|---|
| `test/cli/tui/session-rows.test.ts:15–310` | Selected-pin duration, throughput, historical footers, missing anchors, unrelated-history subscription, and steered marker-era turns. |
| `test/markdown-streaming.test.tsx:21–51` | Final fence text survives both batch orders of content/completion. |
| `test/plugin-markdown.test.tsx:14–117` | Effective language-handler identity, unchanged mounted nodes, refreshing changed handlers. |
| `test/cli/tui/inline-tool-wrap-snapshot.test.tsx:126–256` | Alias fallback, failure labels, reminder alignment, atomic status wrapping, malformed metadata, generic/execute summaries, Background predicate, explicit model suffixes. |
| `test/cli/tui/__snapshots__/inline-tool-wrap-snapshot.test.tsx.snap` | Wrapped icon/text/error-column geometry; use fixture source as well as snapshot because snapshots are not comprehensive production behavior. |
| `test/cli/tui/group-view.test.tsx:18–301` | Nested expansion, exact group/part anchors, adjacent instruction rows, unfinished low activity, failed but expandable activity. |
| `test/session-group-navigation.test.tsx` | Expanding and navigating thought groups while maintaining viewport offsets. |
| `test/session-backfill-disclosure.test.tsx` | Backfilled thought groups remain clickable. |
| `test/session-activity-anchor.test.tsx` | Activity summaries at the tail open without invalid scroll jumps. |
| `test/cli/tui/grouping-tree.test.ts` | Group merge/split seams, direct-child boundaries, leaf counts, empty chunks, invalid offsets. |
| `test/cli/tui/session-grouping.test.ts` | Same tool call ID in another message is not incorrectly suppressed. |
| `test/cli/tui/thinking.test.ts` | Leading title recognition, streamed complete title, indentation preservation, ordinary bold prose. |
| `test/cli/tui/form.test.tsx:138–734` | Terminal form restoration, external acknowledgement, compact/wrapped review, paste and burst typing, custom/multi input, checkbox spacing, space behavior. |
| `test/component/spinner-renderable.test.tsx` | Frame clock restarts, autonomous frame request without live renderer, destroy, generated colors, explicit background. |
| `test/component/shimmer-text.test.tsx` | Masked text painting and title shimmer/rename wipe. |
| `test/cli/tui/collapse-tool-output.test.ts` | Shared command/output budget. |
| `test/component/dialog-shell-output.test.tsx` | Live output and completion/polling behavior. |
| `test/cli/tui/diff-viewer.test.tsx` | Dedicated diff controls, line/geometry rules, bounded rendered lines for large files. |

The repository's `.opencode/skills/tui-tests/SKILL.md` gives important test-harness rules: explicitly set width/height, enable Kitty keyboard mode when testing modified keys, inject `ManualClock` for animations, await highlighting completion rather than merely a quiet frame, and destroy renderers/timers before removing fixtures. Port the intent to Rust. A quiet renderer can still be awaiting a tree-sitter worker; taking a snapshot at that moment produces a misleading blank block. Compare work by counts (mounted rows/rendered lines) instead of fragile wall-clock speed assertions.

[M35]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/rows.ts#L427-L482

## Source links

[M01]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/rows.ts#L27-L185
[M02]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/grouping/session.ts#L14-L175
[M03]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/group-view.tsx#L63-L403
[M04]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/index.tsx#L242-L253
[M05]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/index.tsx#L1673-L2070
[M06]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/index.tsx#L2253-L2515
[M07]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/dialog-message.tsx#L13-L93
[M08]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/border.ts#L1-L21
[M09]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/message-parts.tsx#L43-L199
[M10]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/context/thinking.ts#L1-L20
[M11]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/thinking-syntax.ts#L1-L8
[M12]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/plugin/markdown.ts#L1-L18
[M13]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/plugin/builtins.ts#L14-L34
[M14]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/merman/src/markdown.ts#L43-L336
[M15]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/latex/src/markdown.ts#L27-L119
[M16]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/message-parts.tsx#L202-L295
[M17]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/index.tsx#L2736-L2979
[M18]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/util/tool-display.ts#L1-L68
[M19]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/index.tsx#L2982-L3288
[M20]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/index.tsx#L3291-L3446
[M21]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/index.tsx#L3448-L3825
[M22]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/util/collapse-tool-output.ts#L4-L95
[M23]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/dialog-execute.tsx#L28-L279
[M24]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/index.tsx#L2103-L2158
[M25]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/patch-diff.tsx#L23-L201
[M26]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/feature-plugins/system/diff-viewer.tsx#L44-L55
[M27]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/index.tsx#L2655-L2725
[M28]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/permission.tsx#L113-L622
[M29]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/util/permission.ts#L23-L188
[M30]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/form.tsx#L59-L1175
[M31]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/spinner.tsx#L12-L50
[M32]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/spinner-renderable.ts#L32-L145
[M33]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/shimmer-text.tsx#L11-L43
[M34]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/index.tsx#L2171-L2569
