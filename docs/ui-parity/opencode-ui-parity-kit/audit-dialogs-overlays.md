# OpenCode v2: dialogs, secondary screens, and overlays

**Baseline:** `450e7b92412c780a7b5a75226bb8d49aaca24e22`, `packages/tui`, OpenTUI 0.6.8. These are source observations and proposed parity checks, not claims of an implemented Rust port. Normal behavior below assumes `experimental["new-designs"]` is off. The experiment changes layout and interaction independently of the selected theme. Source links are immutable; dimensions are terminal cells.

## 1. Shared modal shell

Build the shared controls before building individual pickers. Their common behavior determines alignment, keyboard isolation, selection preservation, and asynchronous updates across the application. The proposed Rust boundaries are `DialogHost`, `DialogLayout`, `SelectController`, `SelectView`, `DialogPrompt`, `DialogFooter`, and independent overlay controllers; these are recommendations, not verified names in the private bridge.

| Shell property | Exact source contract |
| --- | --- |
| Medium / large / xlarge width | 60 / 88 / 116; maximum `terminalWidth−2` |
| Placement | Horizontally centered; normal top padding `terminalHeight/4`; explicit centered mode vertically centers |
| Backdrop | Full terminal, absolute origin, black alpha 150/255, `zIndex=3000` |
| Panel | Dialog-surface background, top padding 1; no generic rounded border |
| Standard layout | Left/right inset 3, bottom padding 1, gap 1 between header/body/footer |
| Header | Title and right-aligned clickable `esc`; normal title bold; close label cannot shrink |
| List/header alignment constant | `DIALOG_LIST_INSET=4`, one more than standard inset |

The host blurs and remembers the original focused renderable when opening its first dialog. Closing restores focus after 1 ms only if the node remains alive, remains in the renderer tree, and no other dialog is open. Replacement resets size to medium and centering to false, invokes prior close callbacks, and mounts the replacement once. It does not recreate the screen whenever a reactive condition changes. [D01]

Modal mode isolates shortcuts. Escape clears terminal text selection before closing. Ctrl+C first clears selection, then clears a nonempty focused editor, and only then closes. A custom Back action can replace Escape behavior. The backdrop dismisses on release, except when a selection was already present at press; inner releases stop propagation unless they need to reach copy-on-select handling. Port this event ordering, rather than making every click outside dismiss immediately. [D01]

Normal footer actions are text labels: secondary group left, primary group right, inter-action gap 2. At terminal width below 60 the secondary group stacks vertically. Missing configured shortcuts hide command-backed footer actions. Disabled actions neither dispatch nor appear enabled. With new-designs, the footer becomes a raised-high band, bleeding 3 cells through the layout sides and 1 row through its bottom, with vertical padding 1. Hoverable actions add 1 cell on each side; secondary actions wrap and primary actions retain width. [D02]

## 2. Searchable lists: geometry and state

`DialogSelect` is the common picker, but supports custom search, category grouping, asynchronous option replacement, dynamic actions, custom gutters, and detail rows. Treat selected focus and the current saved choice as separate state. [D03]

The normal header has its own extra 1-cell inset, a title row, and a search input after 1 row of top padding. Search focuses after 1 ms and reports editor trait `FILTER`. List scrollbars are hidden; scroll acceleration uses the same configured policy as the transcript. The viewport limit is **`floor(terminalHeight/2)−6`**: 6 rows at height 24, 14 at height 40. Source does not clamp this formula to a positive minimum, so extremely short terminals need renderer fixtures, not an invented minimum.

Each option contributes one row plus `details.length`. A category contributes one heading row, and later categories add another blank row. Normal list scroll padding is 1 cell per side. Row left padding is 1 when current or custom-guttered, otherwise 3; right padding is 3; row gap is 1. The title text also explicitly declares left padding 3, no wrapping, hidden overflow, and truncation default 61 characters through `Locale.truncate` (left truncation is optional). Preserve the actual render tree and validate with OpenTUI; reducing these declarations to a guessed single inset can change the paint. Footer content does not shrink. Supplementary details use middle truncation and calculated content budget `dialogWidth−12`. [D03]

Normal active rows use focused-action foreground/background and bold title. Current-but-inactive rows use selected-formfield text and `●`; inactive background is transparent. A custom gutter replaces the current marker. Description equal to category is omitted. Empty unfiltered lists and unmatched filtered lists have different views. Disabled options are **removed** before filtering, rather than rendered as disabled rows. [D03]

Default fuzzy search weights title score twice, category and supplemental search text once. Categories retain insertion order. `flat` flattens only while searching; a custom `search` always flattens. A nonempty query normally selects the first result, while clearing restores the current choice where applicable. Identity comparisons are deep value equality, not row index equality. Option updates reconcile selection and reveal a moved/current item after layout. `preserveSelection`, `focusTarget`, and `focusCurrent=false` control distinct behaviors; temporary empty results must not lose intended selection. [D03]

Arrow keys/Ctrl+P/Ctrl+N move; PageUp/PageDown move by **10 options**, not viewport height; Home/End select endpoints; Enter submits. Crossing a boundary jumps to the opposite endpoint, rather than modulo-wrapping arbitrary overflow. Optional Alt+Up/Down visits section starts. Pointer motion changes active selection; press selects; release dispatches the option's callback and screen callback. `locked` suppresses navigation, filter-state updates, mouse selection and actions. Dialog-level actions can run with no results; item actions require a selected option. [D03], [D04]

New-designs changes the list: optional search replaces the title; category headings become faint; rows extend to edges with a separate active `┃` gutter; current marker becomes `▪`; active titles lose bold. Overflow reserves an `…` row only while more content remains below, then returns that row to the list at the bottom. If any option has a selected-detail view, its region stays reserved while moving through choices to avoid height jumps. These are separate fixtures, not default v2 styling. [D03]

## 3. Shared prompt and confirmation controls

`DialogPrompt` is a one-row, nonwrapping textarea, initialized at line end, with modal submission priority above the generic editor layer. Busy state suspends and blurs editing, disables submission/extra actions, and displays a spinner; the normal footer becomes a processing label. New-designs reserves a larger footer gap. Caller-specific cancellation and provider-level Escape/Ctrl+C still require testing together. [D05]

Normal `DialogConfirm` starts with Confirm selected; Left/Right toggles Cancel/Confirm and Enter invokes the selection. Its buttons have 1-cell side padding. New-designs directly maps Enter to confirm and Escape to cancel with footer actions. `DialogAlert`/Help normal OK controls have 3-cell side padding; Help derives its command-palette hint from configured shortcuts. Do not use one hard-coded button geometry everywhere. [D06]

## 4. Screen-specific behavior inventory

Unless stated otherwise, screens use medium `DialogSelect` and inherit the rules above.

| Screen | Required behavior and exceptions |
| --- | --- |
| Commands | Groups registered palette-enabled commands; excludes itself. Empty search duplicates suggested commands in a Suggested section. Settings join the searchable pool only while searching. Threshold 0.7; commands clear the dialog before dispatch; setting results open Settings at that setting. [D07] |
| Agent / variant | Agent IDs with descriptions, current agent marker; selection updates local agent then closes. Variant starts with Default mapped to undefined, excludes duplicate default; new-designs removes its filter. [D08] |
| Model | Favorites, Recent excluding favorites, then provider groups; deprecated general models excluded; server ordering retained within grouping. Free is based on nonempty cost tiers whose input costs are all zero. Search uses model-specific filtering. Ctrl+A integrations, Ctrl+F favorite, Ctrl+U usage when available. Selecting a model retains a valid variant, otherwise opens variant choice if any exist. [D09] |
| Model experimental details | New-designs uses stable favorites-at-open ordering, favorite `*`, a selected model facts/price/limit region, and a dedicated free picker when no provider is connected. Facts include input modalities, context size, reasoning support. Enabling the experiment does not change the default theme automatically. [D09] |
| Sessions | Large width 88. Server search debounced 150 ms, up to 50 root sessions, newest first. Current-directory/all-project preference persists; Ctrl+A switches scope. Groups include Pinned when tabs are disabled, Today, then update date. Attention overrides running spinner. Ctrl+D first arms an inline destructive confirmation; moving cancels it; repeat deletes. Ctrl+R opens prefilled rename, rejects empty trimmed titles, closes on success, toasts failure. [D10] |
| Open | Large width 88. Sessions and projects, attention prioritized; normally up to 8 recent candidates, expanded to include all attention cases. Already-open tabs are omitted unless background attention needs action; searching includes them. Exact session IDs can fetch absent sessions. Right drills into project worktrees, Left/Back restores filter and selection, Ctrl+N creates a worktree in that view. Late loads must preserve intervening deletions/moves. [D11] |
| MCP / Status | MCP sorts server names; dynamic Space action connects/disconnects/retries/signs in. Enter opens error details or integration authentication where appropriate. Pending calls cannot duplicate. Status is a simple wrapped diagnostic list, not the interactive MCP picker and not an LSP dashboard. [D12] |
| Integrations | Categories MCP/Popular/Services; MCP prioritized, then explicit integration priority/name. Environment methods are not interactive connect methods. Existing credentials open account management, with Add account first, active marker, rename and two-step delete. Single connect method skips the method picker. API key, OAuth automatic/code, command and external flows remain distinct. [D13] |
| Integration progress | API-key/code errors remain inline. OAuth and command pending states poll at 500 ms; closing an unsettled attempt cancels it. Automatic OAuth has copy/open actions, copying device code if present, else URL. Command progress uses large width, an edge-spanning raised-high output band with side padding 3 and vertical padding 1. Conditional forms include hidden defaults, optional Skip, custom answers and multiselect; they need dedicated fixtures. [D13] |
| Settings / Experiments / Themes | Settings grouped by Appearance, Session, Tabs, Diffs, Input, Alerts, Terminal, Debug. Enter/Left/Right cycle enum values or clamp numeric steps; async saving blocks repeated writes. New-designs Theme Enter opens picker while arrows still cycle. Experiments are boolean toggles with permanent empty-state infrastructure. Theme picker previews persist immediately; cancel restores original selection. [D14] |
| Usage | Distinct loading, signed-out, no-applicable-limits and model-list views. Selecting model opens remaining amount, reset and individual limits; reached/out uses error color. Selected-row footer shows remaining dollars. Alerts warn once per model/level while running. [D15] |
| Stash | Newest first; first-line preview capped at 50, relative timestamp, multiline count. Choosing removes the stash entry before restoring it. Delete is two-step, canceled by movement. [D16] |
| Worktrees | Xlarge width 116; minimum height `max(8,min(16,H−floor(H/4)−2))`, error state 5. Current/Other grouping, current root first, home abbreviation, left path truncation, muted subdirectory suffix. Initial failure locks empty view; failed refresh retains the existing list. Loading/removal locks interaction. Create/move/delete/refresh have separate commands. Forced removal surfaces file-change confirmation; canonical roots/subdirectories cannot use delete action. [D17] |
| File-change confirmation | Medium dialog, at most 8 file rows, hidden scrollbar, edge-spanning raised-high band, A/D/M column width 2, path shrinks, count area minimum 7. Normal Yes initially selected with clamped Left/Right; new-designs uses explicit confirm key such as D for delete. [D17] |
| Plugins | Large width 88; TUI/Server categories, internal entries initially hidden except failures; update spinner/error gutter, update-aware footer. TUI enable/disable and server package updates are different operations; shared error-details screen. [D18] |

## 5. File diff viewer is a separate panel/screen

The diff viewer's **full-layout boundary is width 88** (`30+4+floor(108/2)`). Docked panels are always compact, even when wide. Compact means no file tree, flush cards, and a footer; wider full screens use 2-cell card insets and optionally a raised-base tree. Tree width is `max(30,min(40,floor(width/4)))`, shown only when enabled and files exist. Split patches require **patch-pane width ≥100**, after tree and insets; otherwise unified is forced while preferences remain. Added/deleted files render unified. [D19]

Tree top edge is 1 row; body side padding 2, bottom padding 1; source/count header is 1 row with bottom margin 1. Tree rows are 1 row; optional list layout is 2 rows plus gap 1. Folder rails use `│ ` with bounded depth, folder disclosure uses `▸`/`▾`, files use `≡`, status occupies 1 fixed cell. Hover uses raised-high, selected name bold, reviewed muted/checkmarked. Selection reveals after layout. [D20]

Patch header is 2 rows with bottom padding 1 and sticky translation bounded by its own card. Reviewed files collapse content and change surface; card separators use half blocks. Patch scrollbars are hidden. Source picker distinguishes All, Committed, Uncommitted and Last turn, plus base selection when supported. Docked footer includes half-cell transition, prompt-focus hint, help, bottom blank row. File menu is a distinct 19-column, 1-row absolute menu at `zIndex=2600`, clamped below click, with mark-complete/incomplete; Escape/Ctrl+C/outside press closes. Do not substitute the general 60-column dialog. [D19], [D20]

## 6. Transient overlays and failure screens

| Surface | Placement, timing, and behavior |
| --- | --- |
| Startup | Waits 500 ms; once visible, remains at least 3 seconds. Text changes from plugin loading to finishing startup. Centered bottom rail at bottom 1, z5000, raised-base and side padding 1. [D21] |
| Reconnecting | Initial connection grace 5 seconds; retry grace 1 second; connected hides immediately. Full black-alpha150 backdrop, z10000. Centered width 48/max90%, raised-base, padding 2 horizontal/1 vertical, gap 1; managed and remote copy differ. [D21] |
| Migration | First status query after 1 second, polls each second. Running badge top 1/right 2, z10000, raised-high, info-colored left split border, padding 2/1, spinner and optional numerator/denominator. Failure produces a 10-second error toast. [D21] |
| Toast | Top 1/right 2, maximum width `min(60,W−6)`, raised-high, side split borders colored by variant, padding 2/1. Optional bold title has bottom gap 1; action or close affordance stays fixed, pending count adds a row. Default duration 5 seconds. Hover pauses remaining lifetime. New arrivals normally replace current; while paused or already queued they enqueue. Activating dismisses before action; selection release does not activate. No explicit z-index in ToastSurface: verify overlay ordering instead of assuming toasts are always topmost. [D22] |
| Error details | Wrapping text in hidden-scrollbar body, max height 20; arrows by 1, PageUp/Down by 20, Home/End. C copies details, I prefills a new prompt for investigation, Escape returns to caller. [D23] |
| Root crash | Independent safe palette, because theme context may have crashed. Full canvas, content width `min(84,max(24,W−4))`; explanatory subtext at H≥18, footer at H≥20. Error panel, horizontally selectable Copy/Restart/Quit, scrollable stack with minimum height 3. C/R/Q and Tab/arrows/Enter work; Ctrl+C exits. [D23] |

## 7. Acceptance checklist

- [ ] **DIALOG-01** Medium/large/xlarge width, terminal−2 cap, quarter-height and centered placements match at odd/even sizes.
- [ ] **DIALOG-02** Shared shell, layout gaps, list row padding, footer bands and 59/60-column responsive behavior match both experiment states.
- [ ] **DIALOG-03** Modal replacement does not remount on reactive updates; close restores only a surviving original focus target.
- [ ] **DIALOG-04** Escape/Ctrl+C/selection/editor clearing/backdrop release dispatch in source order.
- [ ] **DIALOG-05** List viewport at heights 23/24/25/40, category gaps/details/empty states match; document extreme-height behavior.
- [ ] **DIALOG-06** Search weighting, custom search, flattening and disabled-option removal match.
- [ ] **DIALOG-07** Current/active/gutter distinctions, truncation, fixed footer and dynamic option replacement match.
- [ ] **DIALOG-08** Page movement is ten options; wrap endpoints, section navigation, locked state and pointer behavior match.
- [ ] **DIALOG-09** Experiment-only ellipsis/detail reservation/search-in-title/markers remain independently gated.
- [ ] **DIALOG-10** Prompt busy/focus/submission priority and each confirmation keyboard/button variant match.
- [ ] **DIALOG-11** Command suggestions/settings search and customized shortcut hints remain coherent.
- [ ] **DIALOG-12** Model/provider/variant transitions, favorites/recents/free state and limit details match.
- [ ] **DIALOG-13** Session debounce, stale response handling, scope persistence, rename and armed deletion match.
- [ ] **DIALOG-14** Open/worktree drill-down preserves query/selection and late loads cannot resurrect deleted/moved entries.
- [ ] **DIALOG-15** MCP errors/auth/pending and integration account/method/form/progress/cancel states have separate fixtures.
- [ ] **DIALOG-16** Settings numerical limits, save lock, experiments empty state and theme preview/cancel match.
- [ ] **DIALOG-17** Stash consume/delete and worktree loading/refresh/removal/file-change confirmations match.
- [ ] **DIALOG-18** Diff 87/88 full-layout and 99/100 patch-split boundaries, sticky headers, review collapse and file menu match.
- [ ] **DIALOG-19** Startup/reconnect/migration timers and overlapping layers match using a deterministic clock.
- [ ] **DIALOG-20** Toast replace/queue/pause/resume/action ordering and crash/error-details controls match.

Existing upstream suites to adapt include `test/cli/tui/dialog-select.test.tsx`, `dialog-prompt.test.tsx`, `dialog-session-list.test.tsx`, `dialog-open.test.tsx`, `dialog-mcp.test.tsx`, `dialog-integration.test.tsx`, `plugins-dialog.test.tsx`, `toast.test.tsx`, `diff-viewer.test.tsx`, `diff-viewer-file-tree.test.tsx`; also `test/component/dialog-variant.test.tsx` and `dialog-error-details.test.tsx`. Inspected representative list/selection and toast tests and indexed these suites; no tests executed for this addendum.

## 8. Coverage boundary

**Implementation examined:** shared dialog/layout/footer/select/controller/prompt/confirm/alert/help; command, agent, variant, model/free-model/model-detail, sessions/rename, MCP/status, settings/experiments, stash, startup/reconnect/migration/toast. Theme picker was examined in the theme audit.

**Targeted behavior/render paths examined, not every helper:** `dialog-open.tsx`, `dialog-workspaces.tsx`, `dialog-workspace-file-changes.tsx`, `dialog-integration.tsx`, `usage.tsx`, `dialog-error-details.tsx`, `error-component.tsx`, `feature-plugins/system/plugins.tsx`, `diff-viewer.tsx`, `diff-viewer-file-tree.tsx`, `diff-viewer-file-menu.tsx`. Integration custom-form internals and diff navigation/data-fetch helpers need further implementation-level inspection when ported.

**Indexed but not individually audited here:** component dialogs `debug`, `go-upsell`, `highlights`, `image-preview`, `pair`, `shell-output`, `skill`, `update`; UI dialogs `export-options`, `export-result`; session dialogs `execute`, `fork`, `timeline`, `message`; `theme-error-toast.tsx`; `diff-viewer-image.tsx`. This inventory prevents treating “uses the shared shell” as proof that each specialized flow has been ported or tested. Message/tool/image audits may cover some separately.

## Immutable source map

- **D01:** `ui/dialog.tsx:24–90,306–483` — modal geometry, lifecycle, keyboard and copy handling.
- **D02:** `ui/dialog.tsx:92–303` — headers, standard layout, bleed, footer responsiveness and hover.
- **D03:** `ui/dialog-select.tsx:125–993` — filtering, reconciliation, input, row layout and experimental variants.
- **D04:** `ui/select-controller.ts:1–11`; `config/keybind.ts:261–283` — movement and defaults. [movement helper](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/select-controller.ts#L1-L11).
- **D05:** `ui/dialog-prompt.tsx:34–184` — prompt lifecycle, priority and busy states.
- **D06:** `ui/dialog-confirm.tsx:20–104`; `ui/dialog-alert.tsx:11–51`; `ui/dialog-help.tsx:5–40`. [alert](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/dialog-alert.tsx#L11-L51), [help](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/dialog-help.tsx#L5-L40).
- **D07:** `component/command-palette.tsx:14–74`.
- **D08:** `component/dialog-agent.tsx:6–33`; `component/dialog-variant.tsx:7–43`. [variant](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/dialog-variant.tsx#L7-L43).
- **D09:** `component/dialog-model.tsx:19–253`; `dialog-model-free.tsx:10–77`; `model-options.tsx:6–120`. [free models](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/dialog-model-free.tsx#L10-L77), [model grouping and facts](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/model-options.tsx#L6-L120).
- **D10:** `component/dialog-session-list.tsx:27–325`; `dialog-session-rename.tsx:7–37`. [rename](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/dialog-session-rename.tsx#L7-L37).
- **D11:** `component/dialog-open.tsx:27–485`.
- **D12:** `component/dialog-mcp.tsx:20–177`; `dialog-status.tsx:6–50`. [status](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/dialog-status.tsx#L6-L50).
- **D13:** `component/dialog-integration.tsx:69–407,474–938` — selection/account/method/auth rendering; helpers continue to 1286.
- **D14:** `component/dialog-config.tsx:27–482`; `dialog-experiments.tsx:17–97`; `dialog-theme-list.tsx:7–55`. [experiments](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/dialog-experiments.tsx#L17-L97), [theme picker](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/dialog-theme-list.tsx#L7-L55).
- **D15:** `component/usage.tsx:87–245`.
- **D16:** `component/dialog-stash.tsx:9–90`.
- **D17:** `component/dialog-workspaces.tsx:39–405`; `dialog-workspace-file-changes.tsx:19–187`; `dialog-worktree-name.tsx:6–40`. [file-change confirmation](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/dialog-workspace-file-changes.tsx#L19-L187), [worktree naming](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/dialog-worktree-name.tsx#L6-L40).
- **D18:** `feature-plugins/system/plugins.tsx:26–267`.
- **D19:** `feature-plugins/system/diff-viewer.tsx:44–53,306–368,650–1189`. [diff constants](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/feature-plugins/system/diff-viewer.tsx#L44-L53).
- **D20:** `feature-plugins/system/diff-viewer-file-tree.tsx:30–274`; `diff-viewer-file-menu.tsx:7–87`. [file menu](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/feature-plugins/system/diff-viewer-file-menu.tsx#L7-L87).
- **D21:** `component/startup-loading.tsx:5–63`; `reconnecting.tsx:5–40`; `migration-overlay.tsx:11–73`; `app.tsx:1376–1400`. [reconnection view](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/reconnecting.tsx#L5-L40), [migration badge](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/migration-overlay.tsx#L11-L73), [reconnection timing](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/app.tsx#L1376-L1400).
- **D22:** `ui/toast.tsx:20–196`; `ui/border.ts:15–21`. [split border](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/border.ts#L15-L21).
- **D23:** `component/dialog-error-details.tsx:17–130`; `error-component.tsx:10–218`. [error details](https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/dialog-error-details.tsx#L17-L130).

[D01]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/dialog.tsx#L24-L483
[D02]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/dialog.tsx#L92-L303
[D03]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/dialog-select.tsx#L125-L993
[D04]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/config/keybind.ts#L261-L283
[D05]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/dialog-prompt.tsx#L34-L184
[D06]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/dialog-confirm.tsx#L20-L104
[D07]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/command-palette.tsx#L14-L74
[D08]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/dialog-agent.tsx#L6-L33
[D09]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/dialog-model.tsx#L19-L253
[D10]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/dialog-session-list.tsx#L27-L325
[D11]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/dialog-open.tsx#L27-L485
[D12]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/dialog-mcp.tsx#L20-L177
[D13]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/dialog-integration.tsx#L69-L938
[D14]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/dialog-config.tsx#L27-L482
[D15]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/usage.tsx#L87-L245
[D16]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/dialog-stash.tsx#L9-L90
[D17]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/dialog-workspaces.tsx#L39-L405
[D18]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/feature-plugins/system/plugins.tsx#L26-L267
[D19]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/feature-plugins/system/diff-viewer.tsx#L306-L1189
[D20]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/feature-plugins/system/diff-viewer-file-tree.tsx#L30-L274
[D21]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/startup-loading.tsx#L5-L63
[D22]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/ui/toast.tsx#L20-L196
[D23]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/error-component.tsx#L10-L218
