# OpenCode selected v2 revision: composer, status rail, auxiliary dock, and input behavior

## Scope and evidence

This is a source audit of `anomalyco/opencode` at the Rust project’s authoritative upstream pin `450e7b92412c780a7b5a75226bb8d49aaca24e22` (commit dated 2026-10-25), checked out in `/tmp/opencode-selected-audit`. The initial audit used release tag `v2.2.0` (`1097e2668b1e08ad63bdf0b1c4860b9bc01e420c`); all relevant files were then diffed against the project pin and references below were corrected to that pin. All observations below concern the full terminal UI in `packages/tui`, not the web app or the separate `mini` terminal frontend. These component observations were established from source. A later focused upstream baseline run passed 224 tests across 28 files after resolving environment-only dependency identity and fixture-listener issues; see `upstream-test-results.md` for commands and limitations. No Rust implementation was validated. Items described as proposed tests or Rust design are implementation recommendations.

The word *composer* is overloaded upstream. The editable chat box is `component/prompt/index.tsx` (`Prompt`). `routes/session/composer/index.tsx` (`Composer`) is the auxiliary Subagents / Shell / Terminals dock that can occupy the same bottom area. They must be modeled as separate components.

### Immutable source index

| ID | Source and relevant lines |
|---|---|
| C01 | [Prompt visual tree, bottom lip, rail, editor, 1674–1983](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/index.tsx#L1674-L1983) |
| C02 | [Prompt metadata row and fitting algorithm, 8–167](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/metadata.tsx#L8-L167) |
| C03 | [Prompt display, highlight, footer details, directory, scanner config, 1537–1672](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/index.tsx#L1537-L1672) |
| C04 | [Prompt footer feature plugin and fitting algorithm, 14–156](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/feature-plugins/prompt/footer.tsx#L14-L156) |
| C05 | [Context calculation and formatting, 46–66](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/util/session.ts#L46-L66), [latest usable assistant selection, 3–22](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/client/src/solid/usage.ts#L3-L22), [number formatting, 19–27](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/util/locale.ts#L19-L27) |
| C06 | [Working-directory hover and actions, 9–65](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/working-directory-actions.tsx#L9-L65) |
| C07 | [Auxiliary composer shell and tabs, 21–157](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/composer/index.tsx#L21-L157) |
| C08 | [Subagent dock, 26–259](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/composer/subagents-tab.tsx#L26-L259), [family traversal, 10–44](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/util/session.ts#L10-L44) |
| C09 | [Shell dock, 12–149](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/composer/shell-tab.tsx#L12-L149), [Terminals dock, 9–114](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/composer/terminals-tab.tsx#L9-L114) |
| C10 | [Prompt/dock/blocker priority, 1590–1658](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/index.tsx#L1590-L1658), [queue dock, 2485–2515](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/index.tsx#L2485-L2515) |
| C11 | [Prompt commands and interruption, 419–548](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/index.tsx#L419-L548), [shell/history handling, 963–1095](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/index.tsx#L963-L1095) |
| C12 | [Submission lifecycle, 1097–1404](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/index.tsx#L1097-L1404) |
| C13 | [Draft and focus lifecycle, 707–761](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/index.tsx#L707-L761), [extmark synchronization and stash, 764–939](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/index.tsx#L764-L939) |
| C14 | [Paste/attachments, 1406–1535](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/index.tsx#L1406-L1535), [serialized paste and image sizes, 372–406](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/index.tsx#L372-L406) |
| C15 | [Autocomplete trigger/key/geometry/rendering, 660–960](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/autocomplete.tsx#L660-L960), [anchor position, 93–132](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/autocomplete.tsx#L93-L132), [options, 407–504](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/autocomplete.tsx#L407-L504) |
| C16 | [v2 key definitions, 128–211 and 246–280](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/config/keybind.ts#L128-L280) |
| C17 | [Scanner frames and colors, 246–368](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/spinner.ts#L246-L368), [border glyphs, 1–21](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/border.ts#L1-L21), [metadata fade, 20–49](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/util/signal.ts#L20-L49) |
| C18 | [Agent selection and colors, 58–135](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/context/local.tsx#L58-L135), [model labels, 398–414](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/context/local.tsx#L398-L414) |
| C19 | [Agent dialog, 6–33](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/dialog-agent.tsx#L6-L33), [variant dialog, 7–43](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/dialog-variant.tsx#L7-L43), [model dialog](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/dialog-model.tsx) |
| C20 | [Usage-limit metadata label, 57–64](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/usage.tsx#L57-L64), [assistant retry notice, 2545–2569](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/index.tsx#L2545-L2569) |

## Release comparison and pin discipline

The selected checkout identifies its TUI package as 2.1.4 and pins OpenTUI core/solid 0.6.8 through the workspace catalog; v2.2.0 uses OpenTUI 0.7.0. Keep 0.6.8 in the reference harness. [Selected workspace catalog](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/package.json#L58-L60), [TUI package version](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/package.json#L1-L5). Identical JSX does not establish identical renderer behavior across an OpenTUI upgrade. The prompt visual tree, metadata algorithm, footer plugin, auxiliary composer/tab components, key definitions, scanner generator, agent/model label code, and usage selection have identical source between the selected pin and v2.2.0. The main session file has a turn-metrics helper refactor, so its later line numbers shift; its composer/queue/retry branches retain their behavior. The broad app-lifecycle suite also changes between revisions, so use the selected copy as the reference.

One input correctness change does matter: selected `src/prompt/part.ts` lines 4–9 expands copied placeholders with a sequential string replacement; v2.2.0 replaces it with document-order literal concatenation and adds a regression test for replacement metacharacters and embedded placeholder text. Keep this distinction explicit. A literal-safe Rust copy operation is a recommended correctness improvement, not a byte-for-byte reproduction of that earlier bug. Submitted text uses the separate range-based `expandTrackedPastedText` helper, which is unchanged. Source comparison: [selected helper](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/prompt/part.ts#L4-L27), [v2.2.0 helper and regression](https://github.com/anomalyco/opencode/blob/1097e2668b1e08ad63bdf0b1c4860b9bc01e420c/packages/tui/test/prompt/part.test.ts#L25-L40).

## 1. Exact editable composer layout

All numeric dimensions below are terminal cells or rows, not CSS pixels. The Prompt fills its allocated parent width. Its subtree does not itself impose the page margin; the session/home parent owns that placement. Preserve that ownership in Rust to avoid accidental doubled padding. [C01]

| Element, top to bottom | Source behavior |
|---|---|
| Full prompt anchor | Width 100%; hidden when `visible === false`. The autocomplete popup anchors to this rectangle. |
| Main left border | Left border only. `SplitBorder` uses `┃`; the main box overrides its bottom-left glyph to `╹`. Border color is the animated agent highlight. Do not add a full rounded rectangle or a top border. |
| Raised inner surface | `theme.decrease(theme.background.raised.base)`, not a hard-coded gray and not exactly the sidebar surface token. Width 100%, grow 1, shrink 0. |
| Horizontal inner padding | Left/right 2 cells normally; left/right 1 cell when terminal width is strictly less than 44. |
| Top padding | One row before the input or image previews. |
| Optional image preview strip | Appears only when `config.prompt.image_preview` is enabled and data-image attachments exist. Details below. |
| Textarea | Full available width; minimum 1 row; maximum `max(6, floor(terminalHeight / 3))` rows. The textarea's cap is not the entire composer's cap. |
| Metadata row | Beneath the textarea; `paddingTop=1`, horizontal direction, `gap=1`, no shrinking, space-between alignment. |
| Bottom lip | Separate one-row box. Left `╹` when prompt background is nontransparent; inner bottom border renders `▀` in the prompt background color. Transparent background substitutes spaces. |
| Status rail | Separate row below the lip, width 100%, horizontal, space-between, gap 2. It is outside the highlighted prompt surface. |

A normal one-line prompt with no image strip has a nominal six-row structure: top padding, text, spacer before metadata, metadata, half-block lip, footer rail. Measure the real parent+child renderer result for acceptance; do not hard-code six rows across wrapping, preview, or blocked states. [C01]

The text and focused text colors are `theme.text.base`, or `theme.text.muted` when leader mode/prompt muting applies. Focused background is transparent so it reveals the raised inner surface. Placeholder uses `theme.text.muted`. Cursor uses configured cursor style and text-base color; when disabled it uses `background.raised.base`. The textarea is a rich text input using the current syntax style, not a plain string drawn into a box. [C01]

### Metadata content and exact ordering

The normal order is:

`Agent [auto] · Model Provider [· Variant] [  Limit]`

These are separate text nodes with one-cell gaps. **There is no separator dot between model and provider.** `auto` appears when permission mode is `autoaccept`. The limit label has one additional left margin, yielding two spaces in the layout-measurement string. [C02]

| Field | Rendering |
|---|---|
| Agent | Titlecased selected agent ID; agent highlight color. This is data-driven and supports more than Build/Plan. |
| `auto` | Muted text, tied to agent reveal alpha. |
| Agent/model separator | Muted `·`, present only when both model and displayed agent exist. |
| Model | Human-readable catalog name. Falls back to `<model ID> (unavailable)`; without selection it is `No provider selected`. Text base unless prompt is muted; no wrapping; shrinkable, truncating. |
| Provider | Human-readable provider name or provider ID fallback; muted text; does not shrink. Missing selection produces `Connect a provider`. |
| Variant | Preceded by muted `·`; warning-color text and bold. |
| Workspace limit | Only when usage is low/reached; muted or feedback-error, no wrapping or shrinking. `Limit reached` is danger; otherwise formatted dollars remaining. This is distinct from context percentage. |

The metadata row has no click handlers for opening model/agent selectors in this source. Agent/model selection is through registered commands/dialogs. Adding mouse targets to these labels would be a product extension, not an observed selected-revision parity requirement. [C02, C18–C20]

### Exact responsive metadata algorithm

1. Set text budget to `max(0, terminalWidth - 9)` below 44 columns, or `max(0, terminalWidth - 13)` otherwise.
2. Below 44, omit agent and provider. Shell mode is separate: it displays Shell/agent and an empty model.
3. Try candidates in this order: all fields including `auto`; remove `auto`; shorten provider to the final segment after `" / "`; omit provider.
4. Select the first candidate whose measured terminal width fits the budget.
5. If no candidate fits, keep the remaining agent, variant, and limit; truncate model by cell width to at least a 9-cell budget after subtracting those prefix/suffix widths. Remove whitespace immediately preceding the final ellipsis.
6. There is an additional render-level truncation on the model text node. It matters when the available parent width is smaller than this terminal-width-derived budget.

The use of **terminal** width in this algorithm, rather than the component's measured width, is an observed upstream detail. A Rust cleanup that substitutes parent width can change the reference output when sidebars and rails consume space. For strict parity, keep the upstream behavior and test it before deciding on any intentional difference. [C02]

### Agent color and reveal lifecycle

Agent colors honor an explicit configured hex color. Otherwise they select from deduplicated categorical theme scales: scale step 400 under `new_designs`, step 200 otherwise; index is the visible-agent index modulo palette length. Main agent selection excludes hidden agents and agents whose mode is exactly `subagent`. Mention completion uses a different filter. [C18]

Muted prompt highlight is `border.base`; shell highlight is `text.action.primary.selected`; normal highlight is current agent color or border fallback. The border interpolates from `border.base` to that highlight by the agent reveal alpha. Agent/model/variant alpha uses a 0.16-second tween on first revelation. A WeakSet of local contexts prevents remounting the prompt from replaying its initial metadata reveal. The last resolved display remains visible during destination catalog loading, but submission still validates against the live location catalog. [C03], [C17]

## 2. Bottom rail: idle, busy, usage, live children, and overflow

The footer is extensible composition, not one string. The base Prompt provides `prompt.footer`, nested `prompt.footer.status` and `prompt.footer.file` slots; the built-in `opencode.prompt.footer` plugin appends the right-side live/usage/command content to `prompt.footer`. A port that copies only `Prompt` and omits the registered feature plugin loses the right half of the rail. [C01], [C04]

### Left status area: exclusive branches

| Priority | State | Exact presentation |
|---|---|---|
| 1 | Session status is running | Horizontal scanner with left margin 1, then gap 1, then `esc interrupt`. Directory is not shown at the same time. |
| 2 | Worktree move/create progress | One-row box with left padding 3; Spinner in accent-500 color, progress text, muted animated dots. |
| 3 | Pending new worktree | Left padding 3; accent text `(new worktree)`, truncating and unwrapped. |
| 4 | Idle/home directory | Abbreviate home directory, append `:<current branch>` when available, path-aware truncation to measured left-area width. Muted at rest, text-base on hover. |

On Home, directory comes from the current location reference, falling back to default location. An existing session's directory label is supplied only while that session is idle. A pending directory selection overrides the label. Directory display is clickable: open a `Working directory` select menu with Copy path, Open folder, and Workspaces when moving is supported. A nonempty current text selection prevents the menu opening. [C03], [C06]

The current editor file/range may occupy an additional footer slot. It is separately truncating, muted normally, and accent-colored when editor context is pending. It is not the working-directory label and must not overwrite it. [C01]

### Busy scanner and interruption

This prompt scanner is not the standard ten-frame Braille tool spinner. Its frame generator uses an eight-cell bidirectional scan of `■` / `⬝`, with 30 hold-start and 9 hold-end frames. `8 + 9 + 7 + 30 = 54` frames, at 40 ms/frame. It derives trail colors from agent color (border fallback), uses inactive factor 0.6 and minimum alpha 0.3; six trail steps are the generator default. Respect the full color-index/trail implementation in `ui/spinner.ts`, not just these constants. When animation is disabled, render `[⋯]`. [C03], [C17]

First Escape while a running prompt owns focus arms interruption. Its text becomes `esc again to interrupt`, both portions in warning color. Animation-enabled arming briefly blends toward a flash color over 0.22 seconds. Second Escape sends interruption with `resume: true`, then clears the armed state. The reset timer is 5000 ms. Autocomplete suppresses this command; in shell mode Escape exits shell mode first. [C11; `PromptInterruptStatus`, prompt index lines 109–145]

`showDetails` becomes false after arming only when terminal width is below 80; at 80 and above details remain. This reduces contention between the longer interruption hint and rail metrics. It does not hide the live child/shell label itself. [C03], [C04]

### Right content and data semantics

Normal-mode footer content has this priority and order:

1. If any family sessions are running or shell records exist, display the first configured child-picker shortcut followed by `N subagent(s)`, `N shell(s)`, separated by ` · ` when both exist. Family counting excludes the currently displayed session ID; do not replace this with a count of only direct children.
2. When details are enabled and width allows, append usage and positive session cost, joined with ` · `. Add a separator after live status if needed.
3. With no live status and no metrics, the first hint is the resolved agent-cycle shortcut plus `agents`.
4. If the shortcut budget fits and details are enabled, show the resolved command-palette shortcut plus `commands`.
5. Shell mode replaces the plugin's normal content with `esc exit shell mode`, shortened to `esc shell` below 44 columns.

Live-status hover changes its text from muted to base, and click dispatches `session.child.first`. Shortcut hints are disabled-colored and DIM when the session does not own interactivity, such as focus being in a side panel. [C04]

Shell-count nuance: this footer filters `data.shell.list(location)` only by `metadata.sessionID`. It does **not** independently filter `status === running`. In contrast, the Shell auxiliary dock explicitly does filter running state. Preserve the actual data API/list semantics, and test record removal rather than silently applying an extra UI filter. [C04], [C09]

### Context consumed is not lifetime tokens

`contextUsage` asks for the latest assistant message with token usage **after the last completed compaction and before the revert boundary**. An unknown revert boundary produces no usable message. It sums:

`input + output + reasoning + cache.read + cache.write`

If the sum is nonpositive, usage is absent. Percentage is `round(tokens / matchedModel.limit.context * 100)` when that message's model has a context limit; otherwise show the token count without a percent. The current selected model must not replace the usage-bearing message's model in this calculation. Cost is separately the session cost, shown only when positive, using en-US USD formatting. [C05]

Token formatting is exact: values below 1000 are plain; at least 1000 gives one decimal and `K`; at least 999950 gives one decimal and `M` to avoid `1000.0K`. Thus a fixture with 1000 tokens and a 10000 context limit displays `1.0K (10%)`. A zero-cost session omits cost instead of showing `$0.00`. [C05]

### Footer width algorithm

With no usage/cost metrics, shortcuts are eligible only at terminal width ≥44. Otherwise use:

`w = max(0, terminalWidth - 8)`

`available = max(0, w - min(28, floor(w / 2)))`

Measure usage parts joined by ` · ` and the configured command hint. Try usage + separator + shortcut together, then usage alone, then shortcut only if usage is empty. Otherwise hide both. This algorithm does not include the live-label length in its calculation; flex shrink/grow handles the actual row. Details being false suppresses usage and shortcuts even if the calculated fit is true. Reimplement the function faithfully and separately verify the resulting whole-row layout under long live labels. [C04]

## 3. Bottom-area state priority and auxiliary dock

The session page renders a queue dock above the input when queued prompts exist, the explicit auxiliary composer is closed, and the session is not disabled. It renders the auxiliary Composer when opened, or when viewing a child session without forms. Then it selects exactly one of these branches: auxiliary dock already owns area; permission prompt; form prompt; missing-location recovery; normal Prompt if enabled. Permission and question/MCP-form experiences are replacements for the input, not decorative widgets inside a permanently active input box. On a child session, closing the dock navigates to the parent. [C10]

Retry details live with the assistant message, using warning color, left padding 3, ceiling seconds-to-retry, attempt number, and error text. Do not invent a retry label in the idle-directory branch merely because it sounds plausible. Compaction/permission/form visuals need their own reference captures from the corresponding production components. [C20]

### Auxiliary Composer container

The dock uses a left SplitBorder in `border.base`, `background.raised.base`, padding left 1 / right 2 / top 1 / bottom 1. Its inner vertical gap is 1. Header has extra left padding 1 and space-between layout; tab labels are horizontal with gap 2. Active tab label is text-base bold; inactive tab labels muted. A single registered tab is a plain bold title. The right `esc` label is muted and clickable. Footer hints use row gap 2, left padding 1, with bold action text and muted shortcut. Show `tabs ←/→` only when there are multiple tabs. [C07]

Registration order is Subagents, Shell, optionally Terminals. Opening chooses a matching default tab or first registered tab. Left/right wraps tabs; Escape and Ctrl+C close. The tab labels themselves are text nodes without mouse-tab-switch handlers in this version. [C07]

### Subagents tab

- Reconstruct the family by walking to the root and traversing descendants. Preserve branch prefixes `├─`, `└─`, and `│`; root is not emitted by this descendant walk.
- Agent label is titlecased session agent, or parsed from legacy title `@... subagent`, or `Subagent`. Legacy annotation is removed from displayed title when possible; empty titles use timestamped fallback.
- Default filter is active-only (`status === running`). Ctrl+A toggles between running and nonrunning, not “active” and “all.” The filter is stored in TUI memory and survives route remounts until explicitly toggled.
- Max list height is 5; scrollbar hidden. On opening/navigation, select the current session if it is in the filtered list, otherwise the first; center selection. The source reapplies centering after layout with two animation frames because remount can reset scroll.
- Each row has left/right padding 1. Keyboard/mouse selection uses primary-action focused background/text and bold; a current-but-unfocused row uses selected background/text; others primary base.
- Main left text is family prefix + `Agent: Title`. The right status is literally `Running` for running rows and empty otherwise. This particular component has **no per-row spinner and no numeric progress percentage**.
- Up from index 0 closes; Down wraps. Enter navigates to selected subagent. Ctrl+D interrupts only a running selected entry. Hints show interrupt only when appropriate and show the action to toggle the filter.
- Empty copy distinguishes no active and no inactive subagents. [C08]

### Shell tab

List only session-owned running shells. One row per shell using only the first line of its command. Max height 5, hidden scrollbar, left/right padding 1. Selected row uses primary focused surface and bold text. Mouse move selects; click opens the output dialog. Up from the first item closes; Down wraps; Enter opens shell output; Ctrl+D removes/kills selected shell. Clamp selection after removals and scroll selected item into view. Footer offers `output` and `kill` only when a record is selected. Empty list has a muted no-shell-commands message. [C09]

### Terminals tab

Only register when the parent says terminals are enabled. List existing terminals plus a final `+ New terminal` row. Display foreground process, otherwise terminal title. Max height 5; hidden scrollbar; horizontal padding 1. Focused and currently visible terminal states use distinct focused vs selected colors. Up/down cycles through all records plus New terminal; choose existing or create new, close dock, report load failures through a toast. Text is unwrapped and truncating. [C09]

### Queue dock

Separate one-line content strip with left SplitBorder, bottom margin 1, vertical padding 1, left padding 2 and right padding 1. Base background is `background.raised.base`; hover background uses `theme.decrease` of it. Text starts with base-color `N queued`, followed by muted ` · ` and the first queued prompt with newlines replaced by spaces, unwrapped and truncating. Click opens queue management. Queue manager supports cancel/delete, restoring a queued prompt to input, and moving queued work into steer behavior. Blank Enter on an existing session first commits agent/model selection and then promotes the oldest queued prompt to steer. [C10, C12; session index lines 748–820]

## 4. Input behavior is part of visual parity

The UI cannot stay visually identical during use if its state and editing rules differ. Port the presentation state along with the cells. [C11–C15]

### Focus and key routing

Prompt disabled state combines explicit disablement and inherited interactivity. When invisible, disabled, or a dialog is open, the textarea blurs and becomes unfocusable. A slot/plugin update must not steal focus back from a dialog. It regains focus only after the dialog closes. Input traits are recomputed for normal/shell mode and autocomplete visibility. [C13]

Do not flatten the keymap into one global switch. The source has mode- and target-scoped command layers, priorities, configurable disablement, and input editing bindings. A selected dock or autocomplete layer takes precedence over textarea cursor movement.

| Action | Selected-revision default |
|---|---|
| Actual Enter submission | `input.submit = return`; the separately bindable `prompt.submit` command defaults to `none`. |
| Newline | Shift+Enter, Ctrl+Enter, Alt+Enter, Ctrl+J. |
| Queue prompt | Leader+Enter. |
| Cycle agent | **Shift+Tab**. Reverse cycle has no default. |
| Agent/model menus | Leader+A / Leader+M. |
| Cycle model variant | Ctrl+T. |
| Recent model cycle | F2 / Shift+F2. |
| Clear nonempty input | Ctrl+C. |
| Paste | Ctrl+V, with special default-prevention handling. |
| History previous/next | Up / Down, after textarea edge logic. |
| Interrupt running session | Escape with two-step arming. |
| Background blocking tools | Ctrl+B when the relevant command layer owns it. |
| Open child picker | Down when its session command is eligible. |
| Autocomplete previous/next | Up or Ctrl+P / Down or Ctrl+N. |
| Autocomplete select/complete | Enter / Tab; Tab expands a directory rather than immediately selecting a leaf. |
| Autocomplete hide | Escape. |
| View image attachments | Leader+I. |

The repository's README and some older web documentation say Tab cycles agents. The pinned `config/keybind.ts` and `test/keybind.test.ts` explicitly use Shift+Tab. Source and tests are the authority for this reproduction target. [C16]

### Shell and history

Pressing `!` at editor offset zero in normal mode, with autocomplete hidden, switches to shell mode. Escape exits shell mode; Ctrl+C exits it only with empty input; Backspace at offset zero also exits. The prompt displays Shell metadata and shell-specific placeholder/hints. After shell submission it returns to normal mode. [C11]

History does not immediately steal Up/Down from multiline navigation. At the first visual line, Up first moves to absolute offset zero, then older history can be selected. Down similarly reaches the editor end before advancing history. Restore the historical prompt's mode, structured attachments, extmarks, and correct cursor position. Use the renderer's editor-width measurement, not Rust byte counts, Unicode scalar counts, or JavaScript string lengths. [C11], [C13]

### Submission transaction

- Reject submission when disabled, worktree creation is active, autocomplete is visible, or an overlapping submission is already in progress.
- Native textarea submission is double-deferred for IME; immediately before sending, read authoritative editor text and synchronize tracked parts so the final composed character is included.
- Queued shell commands, exit words, argument-taking local slash commands, and queued pending editor selections are rejected with a warning, preserving input.
- Validate selected agent and location-specific model availability. Empty Enter in an existing session has meaningful behavior; do not globally discard all blank input.
- Snapshot the full input synchronously and clear it before asynchronous requests. New typing must remain in the emptied composer while the first request is in flight.
- On failure, restore the snapshot only if the user has not already typed new input; toast the error. A second Enter must not create a phantom empty session.
- Persist agent/model selection using the source's per-session/per-agent draft choices and acknowledge durable changes without flicker.
- Normal delivery defaults to steer. Queue is a distinct delivery, with visible pending state and later promotion.
- New-session creation is optimistic; navigation can occur before create/environment setup finishes. Recovery must restore the relevant draft and route safely. [C12]

### Drafts, stash, paste, and structured input

Draft ownership is captured when Prompt mounts because a lazily read route during cleanup could point at the next tab. Save text, structured parts, and cursor for the old tab; restore on mount. Do not overwrite an already nonempty prompt with a stashed draft. Explicit stash push/pop/list is separate from automatic tab draft retention. [C13]

File mentions, agent mentions, skill mentions, and folded paste are linked to virtual extmarks. Editing must move their ranges, remove references when deleted, retain mentionless attachments, and restore extmarks from history/stash. They have distinct `extmark.file`, `extmark.agent`, `extmark.skill`, and `extmark.paste` syntax styles. Skills are deduplicated. A Rust string with colored `@` substrings but no tracked semantic ranges will fail these behaviors. [C13–C15]

Paste is serialized; each asynchronous paste checks that session, mode, current editor text, and lifetime still match before committing. Normalize CRLF and CR to LF, decode bracketed paste properly, strip ANSI escape sequences while retaining supported control characters. Empty bracketed paste can mean clipboard image on older Windows Terminal and triggers clipboard fallback. Text that is at least three lines or more than 150 characters is normally folded as `[Pasted ~N lines]`; `prompt.paste = full` opts out. Clicking the folded extmark expands it. Pasting the same content adjacent to its matching folded marker expands it instead of creating a duplicate. Clipboard extraction and submission expand tracked paste to actual content. Local image/path/SVG attachment resolution has distinct handling. [C14]

When previews are enabled, preview height is `clamp(floor(terminalHeight / 4), 4, 8)` and nominal width twice that. Show at most three thumbnails with gap 1, strip height previewHeight+1, bottom padding 1; an overflow card is eight cells wide and shows the remaining count. Images use cover fit and auto terminal image protocol; failed preview shows muted `No preview`. Clicking image/overflow opens the preview dialog. [C01], [C14]

### Autocomplete panel

Autocomplete is an absolute overlay **above** the prompt, width and x aligned with the prompt anchor. Its top is anchor-relative y minus panel height; z-index 100; SplitBorder on left and right in border-base; surface `background.raised.high`. Height is `min(10, optionCount or 1, max(1, anchor absolute y))`, so it does not extend above the terminal. While visible it checks anchor x/y/width every 50 ms to follow layout changes. Scrollbar is hidden. [C15]

Rows have left/right padding 1. Focused row uses primary-action focused background/text; pending destructive confirmation uses destructive focused colors. Title is middle-truncated to a content budget reserving four cells and optional kind label width plus two. Description follows with normalized whitespace, muted unless selected, shrinkable. Kind label (`skill`, `agent`, `reference`) sits at the right after a minimum-two-cell spacer. Mouse move/down selects; mouse up accepts. Empty views distinguish no command match, searching, no file/agent/reference match, directory search failure, and file search failure. [C15]

Slash triggers occur at the start of input; `@` triggers at a valid mention boundary with no intervening whitespace; `/cd` argument completion switches to directory mode. Mention candidates include visible non-primary agents, skills, references, and files. This is intentionally a different agent set from the primary-agent picker. Preserve current filtered results while an asynchronous file search is loading only for the same completion mode. [C15]

## 5. Proposed Rust component and test boundaries

These are recommendations derived from the audited behavior, not claims about the user's existing Rust bridge.

Use small reusable units with explicit measured rectangles and presentation inputs: `PromptSurface`, `PromptEditor`, `PromptMetadata`, `PromptBottomLip`, `PromptStatusRail`, `LiveSessionStatus`, `ContextUsageFormatter`, `AutocompleteOverlay`, `QueueDock`, `AuxiliaryComposer`, `SubagentList`, `ShellList`, and `TerminalList`. Keep footer slot contributions composable. Treat theme tokens, text-width measurement, focus owner, selection, keybinding lookup, and clock as shared services.

Do not couple render components to raw network calls. Project incoming server/session state into a presentation model, reduce events and input actions into that model, and let components render it. The critical state transitions above require preserving identifiers and ownership, not copying Solid's internal mechanics. Input editing can remain in a rich-editor adapter while Rust owns application state. The bridge contract needs editor offsets/extmarks, raw grapheme-aware measurements, focus, pointer hit regions, scroll geometry, style spans, alpha/transparent colors, and controlled animation time.

### Source-backed implementation checklist with acceptance tests

All boxes are intentionally unchecked: this audit establishes the target, not implementation completion.

| ID | Checklist | Acceptance evidence to require |
|---|---|---|
| CP-01 | [ ] Separate editable Prompt from auxiliary Composer. | Render each in the same parent slot and confirm replacement, not duplication. C01/C07/C10. |
| CP-02 | [ ] Match left-only border and separate half-block bottom lip. | Cell+style snapshot proves `┃`, `╹`, `▀`, with transparent variant using spaces. C01/C17. |
| CP-03 | [ ] Match raised prompt surface through theme transformation. | Compare foreground/background RGBA on blank padding cells in dark/light/custom/transparent modes. C01/C03. |
| CP-04 | [ ] Match padding and editor height rules. | Widths 43/44 and heights 12/24/40; editor grows then scrolls; footer stays outside input. C01. |
| CP-05 | [ ] Place agent, model, provider, variant below textarea in exact order. | Assert coordinates and individual style spans, including no dot between model/provider. C02. |
| CP-06 | [ ] Port metadata candidate fitting exactly. | Fixtures for long provider chain, long model, auto, variant, low limit, wide Unicode, and parent narrower than terminal. C02. |
| CP-07 | [ ] Preserve data-driven agent selection and colors. | Custom agent, hidden agent, subagent-only agent, explicit color, palette wrap. C18. |
| CP-08 | [ ] Preserve display during catalog reload. | Delayed location catalog retains old labels but prevents unavailable-model submission. C03/C12. |
| CP-09 | [ ] Render initial reveal and muted/shell border states. | Fixed-clock frames at start/intermediate/end; no replay after tab remount. C03/C17. |
| CP-10 | [ ] Register built-in right-footer contribution. | Full app fixture, not Prompt in isolation, includes usage/cost/hints. C04. |
| CP-11 | [ ] Make running state replace idle directory. | Idle→running→idle snapshots assert mutually exclusive labels and exact geometry. C01/C03. |
| CP-12 | [ ] Port eight-cell scanner and reduced animation mode. | Deterministic sample across movement/holds, frame 0 through 53, 40-ms steps; static `[⋯]` fixture. C17. |
| CP-13 | [ ] Match interrupt arming, expiration, color flash, and detail suppression. | First/second Escape, 4999/5000 ms, widths 79/80, autocomplete and shell interception. C11/C03. |
| CP-14 | [ ] Match directory/branch formatting, truncation, hover, actions. | Home abbreviation, branch suffix, long path, selected text suppressing click, action menu. C03/C06. |
| CP-15 | [ ] Keep editor-file slot distinct. | Directory plus pending/attached editor file in one rail, with overflow and pending accent. C01. |
| CP-16 | [ ] Count family activity and shell records with actual upstream semantics. | Nested siblings, current child, inactive child, shell record status transition; compare source projection. C04. |
| CP-17 | [ ] Match context usage arithmetic and boundaries. | All five token categories; compaction; revert; unknown boundary; missing model limit; zero tokens. C05. |
| CP-18 | [ ] Match number/currency formatting. | 999, 1000, 999949, 999950; known/unknown context limit; zero and positive cost. C05. |
| CP-19 | [ ] Match footer priority and fitting. | Long labels; width 43/44; usage+shortcut fit/fail boundaries; live status plus metrics; showDetails false. C04. |
| CP-20 | [ ] Match footer hover and disabled hint styling. | Pointer enter/leave and focus moving to a side panel, preserving shortcut identity. C04. |
| CP-21 | [ ] Match queue dock dimensions and hover. | Zero/one/multiple prompts, multiline first prompt flattened, long text truncation, click opens queue management. C10. |
| CP-22 | [ ] Enforce dock/permission/form/location/prompt precedence. | Every priority combination; child session plus form; closing child dock navigates parent. C10. |
| CP-23 | [ ] Match auxiliary dock tabs, geometry, and active styles. | One/two/three registered tabs, left/right wrap, Escape/Ctrl+C, hint layout. C07. |
| CP-24 | [ ] Match subagent hierarchy, activity filter, and selection lifecycle. | Nested family, inactive toggle, route remount, deletion, current hidden by filter, more than five rows. C08. |
| CP-25 | [ ] Render subagent Running text accurately. | No invented progress percentage/spinner in that list; running/idle row captures. C08. |
| CP-26 | [ ] Match shell list and output actions. | Multiline command renders one row; running-only filter; selected removal; configured and disabled keys. C09. |
| CP-27 | [ ] Match terminal list with New terminal row. | Foreground-process/title fallback, visible versus focused row, select/create failure and toast. C09. |
| CP-28 | [ ] Respect v2 keymap defaults and layers. | Shift+Tab cycles agent; Tab completes; Enter sends once; configured disablement has no component fallback. C16. |
| CP-29 | [ ] Preserve dialog focus through prompt remount. | Open model dialog, update slot/catalog, type; text must enter dialog, not background prompt. C13. |
| CP-30 | [ ] Match shell entry/exit and multiline history edges. | `!` at offset zero vs middle; Escape/Backspace/Ctrl+C; history restores full parts/mode/cursor. C11. |
| CP-31 | [ ] Preserve IME and duplicate-submit behavior. | Compose final Korean character; race native Enter and command dispatch; assert exactly one submitted prompt. C12. |
| CP-32 | [ ] Preserve synchronous snapshot/clear and conditional error restoration. | Type during in-flight send; fail request; assert new draft survives and old text only returns if field empty. C12. |
| CP-33 | [ ] Match queue vs steer and blank-Enter promotion. | Running/idle, nonempty/empty input, forbidden queue types, agent/model changed before promotion. C12/C10. |
| CP-34 | [ ] Preserve per-tab draft ownership and explicit stash. | Switch A→B→A with distinct text, cursor, attachment spans; pop/list stash, clear significant draft to history. C13/C14. |
| CP-35 | [ ] Implement tracked input parts through edits. | Reordered/duplicate/prefix mentions; deletion; expanded paste shifting mentions; mentionless attachment retention. C13. |
| CP-36 | [ ] Match paste normalization and folding boundaries. | 2/3 lines, 150/151 characters, CR/CRLF, escape sequences, full-paste setting, repeat paste expansion. C14. |
| CP-37 | [ ] Serialize asynchronous paste safely. | Paste resolves after tab switch, mode switch, typing, disabled focus, or unmount; no stale insertion. C14. |
| CP-38 | [ ] Match optional image previews and failure states. | 0/1/3/4 images; terminal heights 12/24/40; decode failure; overflow click. C01/C14. |
| CP-39 | [ ] Position autocomplete above actual prompt anchor. | Resize, long prompt growth, sidebar toggle, near-top prompt, ten-plus candidates, short terminal. C15. |
| CP-40 | [ ] Match autocomplete styles and routing. | Keyboard vs hover selection, directory expansion, kind label, destructive confirmation, searching/error/empty. C15/C16. |
| CP-41 | [ ] Use renderer-compatible offsets and cell widths. | Tamil, CJK, combining accents, emoji ZWJ, tabs, wide chars, surrogate differences; no UTF-8 byte-offset assumptions. C13–C15 and existing display tests. |
| CP-42 | [ ] Preserve same-surface styling during streaming and retry. | Network retry message remains in transcript; prompt busy/idle state independently correct. C20/C01. |

### Existing upstream tests worth porting, not merely copying assertions

- `test/feature-plugins/prompt-footer.test.tsx`: simultaneous subagent/shell/cost separation; hover color and dispatch; hiding usage and hints; known `1.0K (10%)` context fixture.
- `test/cli/tui/prompt-interrupt-status.test.tsx`: armed hint retains visible warning text.
- `test/cli/tui/composer-keymap.test.tsx`: disabled bindings do not fall through to private component defaults; filter only changes with toggle; shell command is one line; configured bindings work even with a focused textarea; Ctrl+C closes dock.
- `test/prompt-offset.test.tsx`: agent mention after a tab replaces only the intended range; history cursor reaches end of wide text.
- `test/prompt-paste.test.tsx`: retain supported control characters while dropping escape sequences.
- `test/prompt-shell-failure.test.tsx`: failed shell command restores draft and shows error.
- `test/prompt/display.test.ts`, `mention.test.ts`, `part.test.ts`: grapheme/editor-width offsets, deleted/reordered/duplicate references, tracked paste expansion using source ranges. The literal clipboard placeholder regression test was added after the selected pin; see the delta note below.
- `test/prompt/draft-stash.test.ts`, `history-provider.test.tsx`, `history.test.ts`: session/home ownership and history behavior.
- `test/prompt/autocomplete.test.ts`, `local-attachment.test.ts`, `attachment.test.ts`: directory completion and attachment rules.
- `test/command-selection.test.tsx`, `test/context/local-selection.test.tsx`: custom command bindings and selection synchronization.
- `test/cli/tui/prompt-move.test.tsx`: location/workspace changes and recovery.
- `test/app-lifecycle.test.tsx`: full-app delayed data, failure, and narrow-terminal behavior.

Use full-frame cell/style goldens for geometry, precise event-driven assertions for behavior, and controlled time for animations. A text-only screenshot cannot prove prompt/background/hover colors, and a pure-function test that repeats the ported formula cannot prove parent layout. Render the real production components through the Rust bridge in the acceptance harness. Do not substitute `test/mini/*` fixtures: the mini frontend is a different visual contract.


[C01]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/index.tsx#L1674-L1983
[C02]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/metadata.tsx#L8-L167
[C03]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/index.tsx#L1537-L1672
[C04]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/feature-plugins/prompt/footer.tsx#L14-L156
[C05]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/util/session.ts#L46-L66
[C06]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/working-directory-actions.tsx#L9-L65
[C07]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/composer/index.tsx#L21-L157
[C08]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/composer/subagents-tab.tsx#L26-L259
[C09]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/composer/shell-tab.tsx#L12-L149
[C10]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/routes/session/index.tsx#L1590-L1658
[C11]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/index.tsx#L419-L548
[C12]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/index.tsx#L1097-L1404
[C13]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/index.tsx#L707-L761
[C14]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/index.tsx#L1406-L1535
[C15]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/autocomplete.tsx#L660-L960
[C16]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/config/keybind.ts#L128-L280
[C17]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/spinner.ts#L246-L368
[C18]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/context/local.tsx#L58-L135
[C19]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/dialog-agent.tsx#L6-L33
[C20]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/usage.tsx#L57-L64
