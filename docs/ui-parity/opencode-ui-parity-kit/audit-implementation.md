# Rust implementation and acceptance plan

## Outcome and evidence boundary

The deliverable is a source-backed specification and test plan for reproducing the full-screen OpenCode v2 terminal UI. The strongest recommendation is to treat the upstream component tree, presentation state, and renderer behavior as one contract. Reusing native OpenTUI is a valuable foundation, but it does not automatically port the application-specific layout, Markdown composition, interaction controllers, or session projections.

The user's project instructions at `/Users/mymac/Projects/opencode-rk-recovery-20261005/AGENTS.md`, lines 337–349, select upstream commit `450e7b92412c780a7b5a75226bb8d49aaca24e22`. This was verified by fetching that exact public Git object into a separate temporary checkout. Its package version is 2.1.4, dated 2026-10-25, using Bun 1.4.4, Solid 1.9.15, and OpenTUI 0.6.8. The initial v2.2.0 audit was compared against this authoritative snapshot; the release comparison is a separate part of the evidence, not a silent target upgrade.

The project directory inventory confirms `crates/opentui-bridge` and `crates/cli` exist. Initial reads inspected project instructions, README/workspace metadata, and directory inventory. Attempts to inspect the bridge implementation were blocked by the connector routing to inconsistent tool catalogs; its explicit diagnostic says two connectors share the same Secure Tunnel ID. Consequently this report does **not** claim a current bridge capability inventory, a Rust code review, or measured gaps in the installed application. The proposed Rust names below are design seams, not assertions that these APIs already exist. A bridge-specific API map remains open until those source files can be read reliably.

All checklist entries describe work to verify in the Rust implementation. None is marked passing merely because upstream source or a Rust file exists. Upstream test execution is reported separately in `upstream-test-results.md`; it is evidence about the reference harness, not evidence of Rust parity.

## 1. Freeze the comparison target before changing placement

Use these independent inputs for each reference capture:

| Input | Required recorded value |
|---|---|
| Product source | Full OpenCode commit, not only `v2` |
| Renderer | OpenTUI native/TS package version and Git commit |
| Runtime | Bun version and Solid version for the reference; Rust build SHA and native library identity for the port |
| Frontend | Full-screen `packages/tui`; mini and desktop are different surfaces |
| Theme | Theme document/hash, mode, system palette if used, and `new-designs` flag |
| Terminal | Columns, rows, width method, Kitty-keyboard/mouse capability, image protocol |
| User-visible settings | Tab mode/layout/scope/indicator style, sidebar, scrollbar, grouping, thinking, verbosity, Markdown mode, image previews, animations |
| Data | Fixed session/message/part IDs, timestamps, model/provider catalog and context limits, token categories, MCP/permission/form/shell/terminal fixtures |
| Time | Injected monotonic clock and exact checkpoint |
| Interaction | Focus target, text selection, pointer coordinates/button history, scroll anchor, expansion state |

The approved baseline is the project-selected commit. Use v2.2.0 only in explicitly labeled upgrade fixtures. In particular, do not combine the selected OpenCode snapshot with OpenTUI 0.7.0 and call that the original renderer. `opencode2` is an optional theme under a separate experiment; it is not a synonym for OpenCode version 2. [R01], [R02]

There is no meaningful universal screenshot equality across arbitrary fonts, terminal emulators, image support, and user themes. A strong, testable meaning of “100% exact” is **zero unexplained differences across the locked reference matrix**, with separately stated environment and extension coverage. Cell-level equality is the primary geometry/style contract; same-terminal screenshots validate the final appearance, including the cursor and half-block edges.

## 2. Reuse the current Rust project; introduce component boundaries only where needed

First inventory the current implementation using `rg --files crates/opentui-bridge crates/cli/src`, then find the owners of tabs, prompt, transcript, sidebar, styles, focus, width conversion, and event processing. Read any more-specific AGENTS files before edits. Trace one real frame from session data to native scene submission. Compare this handoff with existing code and preserve already-correct behavior. Do not build a second UI framework or duplicate a working controller simply to match the suggested names below.

| Proposed reusable boundary | Owns | Must not own |
|---|---|---|
| `ui::theme` | Semantic color roles, mode resolution, hue provenance, state priority, syntax and diff styles | Hard-coded per-screen gray constants |
| `ui::layout` | Measured rectangles, cell constraints, clamps, breakpoint decisions | Provider requests or durable session mutation |
| `ui::session_frame` | Root rails, active right pane, route padding, overlay and focus policy | Per-tool output formatting |
| `ui::tabs` | Pure allocation model plus tab state and rendered controls | Transcript contents or unrelated global shortcuts |
| `ui::prompt` | Editable document, tracked parts, metadata, own layout, focus lifecycle | Parent route margins or lifetime-token totals |
| `ui::status_rail` | Idle/running/location branches and registered footer contributions | Recreating the prompt editor |
| `ui::sidebar` | Ordered Context/Limit/MCP/Todo contributions and pinned footer | An assumed always-present LSP/file section |
| `ui::transcript` | Stable row projection, grouping, expansion, anchor restoration, append/backfill | A full-screen redraw per token |
| `ui::markdown` | Incremental blocks, syntax requests, stable identity, concealment, plugin handlers | Blind string concatenation into one text node |
| `ui::tool_views` | Shared InlineTool/BlockTool primitives and typed renderers | Fake progress percentages or child-session assumptions for execute metadata |
| `ui::blockers` | Permission/form/location replacement states and scoped focus | An active background prompt under a modal |
| `ui::commands` | Registered action IDs, keymap scopes/priorities, disabled bindings | Hard-coded fallback shortcuts in leaf widgets |
| `ui::runtime` | Renderer clock, input dispatch, frame scheduling, lifetimes | App-specific context arithmetic |
| `opentui_bridge` | Checked native handles, ABI translation, scene updates, output/capture | Claiming native APIs supply every TS controller automatically |

These are logical boundaries, not a requirement for thirteen new crates. Keep them as modules within the current architecture when that is sufficient. Pure allocation, projection and usage functions can be tested independently; production widgets must still be rendered together to catch doubled margins, conflicting minimum sizes, clipping and focus errors. The actual upstream boundaries are documented in the layout/composer/transcript appendices and native/TS boundary appendix. [R03], [R04]

## 3. Keep layout ownership explicit

Each component should have a short contract containing: parent-owned outer rectangle, component-owned padding, minimum/preferred/maximum size, shrink/grow behavior, overflow policy, layer/z-order, focus eligibility, and semantic color roles. Copy the upstream numbers and predicates before attempting an abstraction.

For example, session route owns horizontal inset 1 below terminal width 44 and 2 otherwise; Prompt owns its separate inner inset; message cards own their own split border and interior padding. The right sidebar is 42 cells; the left tab rail merely defaults to the same width and is independently resizable. The ordinary prompt's metadata and bottom rail are distinct rows and use different fitting algorithms. A shared `padding = 2` at every nested container would multiply spacing and reproduce the user's present placement problem.

Do not replace cell arithmetic with equal percentage columns. Do not normalize the source's different width domains: metadata uses terminal width, inline diff uses content width, and the dedicated diff viewer uses patch-pane width. Port each predicate faithfully and then test its assembled geometry. The user may approve deliberate fixes later; record them as differences instead of silently calling them exact parity. [R03], [R05]

Keep optional space even where source does: the one-row area above the prompt for history/jump-to-latest exists when empty. Preserve the composer's separate half-block bottom lip and status rail. Freeze text and clipping bounds before painting; model/provider labels must not push the context rail into a different row because a generic container was allowed to wrap.

## 4. State model and event flow

A reusable Rust view model should distinguish durable data from local presentation state. Suggested concepts are `SessionView`, `PromptDraft`, `TabPresentation`, `TranscriptProjection`, `RightPane`, `BottomSurface`, `FocusTarget`, and `AnimationClock`. These are illustrative types, not a replacement wire protocol.

Session data should carry real session identity, parent/family links, execution status, unread/viewed timestamps, outcome, latest usable assistant usage, and session cost. Message parts need stable IDs and typed streaming/running/completed/error payloads. Code-mode child calls are metadata entries; subagents are child sessions. Tool completion and child-session completion may occur at different times.

Presentation state owns tab order and history, per-session draft and tracked attachments, selected agent/model/variant, expansion IDs, semantic scroll anchor, active dock, hovered/pressed item, focused pane, interrupt arming deadline, and dialog state. Do not clear all of it when new text arrives. Preserve native node identity for in-place streaming updates; restore saved presentation state when the source intentionally remounts a session frame keyed by session ID. Native object identity need not survive switching between different session frames.

Use a deterministic update path: receive an event or user action, update the appropriate model, derive visible projection, update only affected scene resources, measure, then paint. Side effects should be emitted once by the controller, not from a widget's paint function. This supports a test that sends an Enter action and checks both the resulting frame and exactly one outgoing prompt command. The same separation makes tab changes, queue-to-steer transitions, and asynchronous paste races reproducible. [R04], [R06]

Respect bottom-surface priority from source: auxiliary dock, permission, form, unavailable location, normal prompt. A child session forces the subagent dock except when a form is present. Prompt focus follows those branches; having the correct visible box while keystrokes still go to the hidden textarea is a failed port.

## 5. Native bridge capability review to complete on the user's source

The OpenTUI appendix establishes that the matching native core includes a retained scene, native Yoga layout, text and editing resources, selection, rendering, and output machinery. It also establishes substantial TS ownership of Markdown composition, async highlighting, diff composition, ScrollBox policy, input framing, and Solid reconciliation. Therefore review capabilities rather than asking whether the bridge can “draw a box.” [R07]

For each capability, locate the existing Rust API and test, record the native symbol/header version it uses, mark present/partial/missing, and map the dependent UI checks. Do not mark missing merely because this audit could not read the source.

| Capability to inspect | Proof required |
|---|---|
| Native ABI and handle ownership | Exact native header/artifact pin; size/alignment/status checks; safe lifetime and owner-thread rules |
| Retained scene and layout | Parent-child tree, row/column flex, basis/grow/shrink, min/max, percent sizing, padding/margin/gap, absolute overlays and clipping |
| Text and editor measurement | Native-equivalent grapheme/cell width, tabs, range conversion, cursor and extmarks |
| Styling | RGBA/transparent semantics, attributes, semantic token provenance, borders/half-blocks, syntax styles and gutters |
| Events | Press/release/move/wheel, drag capture, hover, focus, scoped keymaps, paste/IME/Kitty sequences |
| Transcript scrolling | Sticky bottom, manual history lock, semantic anchors, resize reflow, independent panes and wheel forwarding |
| Markdown/highlighting | Streaming block identities, tables/concealment, worker completion, stale-result rejection, Mermaid/math handlers |
| Animation | Injected time, exact frame selection, visibility/culling, frame requests, timer/resource cleanup |
| Capture and comparison | Character and styled-cell dumps, cursor, layout bounds, focus/hit targets, deterministic time checkpoints |
| Terminal lifecycle | Resize, suspend/resume if supported, shutdown, clipboard/OSC links, image capability fallback, output acknowledgements |

The safe implementation sequence is to fill demonstrated bridge gaps, then port reusable widgets, then compose the page. Replacing the renderer or keeping TS around is not required by this recommendation; a Rust implementation of the remaining controllers can reproduce the behavior.

## 6. A differential parity harness

Run the same semantic fixture through the selected upstream production components and the Rust production components. Use an offline in-process service fixture or replayed typed events. No live model credentials, user sessions, or real tool execution are needed to verify placement and interaction.

`fixtures.json` specifies inputs, actions, expected rules and checkpoints. It is a test specification, not a set of captured golden frames. Capture the upstream goldens first using the original renderer. A screenshot drawn by an AI, a recreated mock widget, or an expected frame generated by the same Rust code under test is not an independent oracle.

Export a portable frame representation rather than serializing native memory. A proposed record contains dimensions; one record per cell with cluster text, display width/continuation status, foreground/background and terminal-default flags, text attributes and hyperlink identity; cursor cell/shape/blink/visibility; clipping and component bounds; focus target; scroll anchor; and emitted semantic actions. Native handles and pointers must not appear in the portable format. Normalize fixture time/IDs and protocol-internal details only where the contract explicitly allows it.

Comparison layers:

1. **Pure behavior:** upstream-derived allocation examples, context arithmetic, grouping and state priorities. These isolate defects but do not alone prove layout.
2. **Production component frames:** every glyph, cell coordinate, RGB/alpha/default color, border, attribute, gutter and clipping boundary. Require zero unexplained differences.
3. **Event traces:** actual input sequence, focus changes, scroll/selection behavior, one emitted action with the expected semantic arguments. Verify disabled commands stay disabled.
4. **Temporal frames:** ManualClock checkpoints for spinner, scanner, pulse, marquee, retry and double-Escape. Test animation-off separately.
5. **Installed-terminal journey:** same terminal emulator, font/size, zoom, theme, dimensions and fixtures. Compare captures for half-block seams, cursor placement, image support and real input parsing.

Text-only snapshots cannot detect the wrong sidebar gray or an invisible cursor. Pixel-only snapshots cannot establish command dispatch or unread acknowledgement. Pair them where those aspects matter.

### Test robustness rules from upstream

Set width and height explicitly. Enable Kitty keyboard mode in modified-key fixtures. Await the signal that makes content ready; a quiet frame can still be waiting on asynchronous tree-sitter highlighting. Await highlighting completion before capturing Markdown/code. Inject clocks; avoid real five-second waits for interrupt timeout. Destroy timers/renderers before deleting temporary fixtures. Use counts of mounted rows or rendered lines for bounded-work checks rather than noisy wall-clock speed limits. Run package tests from `packages/tui`, never the repository root. These rules come from the source repository's TUI testing skill. [R08]

### Prove the tests can fail

Run small negative controls in an isolated candidate: shift a parent inset by one cell; change a strict sidebar `>120` predicate to `>=120`; swap sidebar and prompt backgrounds; let Enter reach two handlers; make a stale highlight update a destroyed block. Each should fail its relevant gate. Revert the mutation immediately. This is more useful than hundreds of tests that duplicate implementation formulas and all accept the same mistake.

## 7. Implementation order and observable exit criteria

| Stage | Implement or repair | Exit criterion |
|---|---|---|
| A. Baseline and bridge | Lock pins, inventory current components, add deterministic frame adapter | Native and Rust fixture frames can be captured with comparable metadata; ABI tests pass |
| B. Geometry and surfaces | Root/session/home frame, theme provenance, rail constraints, sidebar shell | Empty/idle fixtures match all boundary widths, dark and light |
| C. Composer and footer | Editor geometry, metadata fit, bottom lip, scanner/directory, usage footer | One-line/multiline/narrow/running/armed states match; Unicode cursor and single-submit tests pass |
| D. Tabs and sidebar data | Horizontal/vertical/compact tabs, hover/drag/menus/unread; Context/Limit/MCP/Todo | Overflow/reflow/persistence/focus actions match; section gates match |
| E. Transcript | Stable grouping, user/assistant rows, Markdown/code/tables, anchors | Streaming and backfill preserve text/node identity/reading position and exact styled frames |
| F. Tools and blockers | All tool renderers, execute metadata/details, subagents/docks, permission/forms, diffs | Lifecycle transition frames and action traces match, including missing/error data |
| G. Integration | Theme change, resize, tab/session changes, reconnect, optional features | Full deterministic fixture matrix passes on the exact integrated Rust SHA; installed captures reviewed |

A useful first visible milestone is an idle session at 160×48 with the standard theme, correct left/right rails where applicable, a one-line prompt and a context/footer fixture. The next milestone must exercise the same screen while running and after resize. Do not call the first static frame completion; it establishes the reference frame for subsequent stateful work.

The detailed checklists are grouped by observable behavior rather than by source file. Integrate one coherent slice at a time, retaining the user's canonical-writer and validation constraints. This handoff does not create a new orchestration framework, change upstream locks in the user project, or authorize overwriting existing work.

## 8. Cross-cutting acceptance checklist

- [ ] **PAR-01** Record selected product, renderer, runtime and theme pins for every golden; reject mixed-version captures.
- [ ] **PAR-02** Inventory actual Rust bridge/component APIs and map them to the capability table; no guessed existing/missing statuses.
- [ ] **PAR-03** Capture the real upstream and Rust production components using identical deterministic fixture data.
- [ ] **PAR-04** Compare text, positions, styles, clipping, cursor and hit targets; text-only equality is insufficient.
- [ ] **PAR-05** Preserve wide-cell continuation and grapheme semantics in canonical frame serialization.
- [ ] **PAR-06** Record exact baseline terminal/font environment for pixel captures and isolate terminal-specific cases.
- [ ] **PAR-07** Require event traces as well as frames for navigation, queue/steer, permissions, forms and input submission.
- [ ] **PAR-08** Freeze time and use explicit checkpoints for every motion effect and deadline.
- [ ] **PAR-09** Await asynchronous syntax completion; do not bless blank transient code blocks as goldens.
- [ ] **PAR-10** Exercise streaming→complete, complete→final-content, resync, backfill and stale async-result orders.
- [ ] **PAR-11** Preserve node identity during in-place updates; preserve logical IDs, saved group expansion and semantic scroll anchors across keyed session remounts and tab switches.
- [ ] **PAR-12** Match upstream keymap scope, priority and reachability in popups, docks, modals and focused side panels; suppress background actions only where upstream suppresses them, while retaining intentionally global commands.
- [ ] **PAR-13** Verify there are no duplicate sends or unintended tool actions from pointer release/drag selection.
- [ ] **PAR-14** Test narrow/short terminals and every exact predicate boundary, not only one 160-column screenshot.
- [ ] **PAR-15** Resolve full light/dark/transparent/stateful theme values, preserving hue provenance.
- [ ] **PAR-16** Separate baseline, optional configuration, experimental design and approved extension fixtures.
- [ ] **PAR-17** Keep selected-pin bugs and deliberate fixes explicit; do not silently merge v2.2.0 behavior.
- [ ] **PAR-18** Verify teardown cancels timers, rejects late workers, releases resources once and restores terminal state.
- [ ] **PAR-19** Use mounted/rendered work counts for large transcript/diff bounds; preserve visible geometry.
- [ ] **PAR-20** Run negative controls that prove one-cell, color, threshold and duplicate-action regressions fail.
- [ ] **PAR-21** Preserve existing semantic tests; investigate source disagreements before changing expected output.
- [ ] **PAR-22** Attach source/fixture hashes, exact test commands, failures and installed captures to the final Rust SHA.
- [ ] **PAR-23** Keep all Rust parity statuses unverified until executed against the real integrated Rust build.
- [ ] **PAR-24** Complete bridge-source inspection after correcting the duplicate Secure Tunnel ID routing problem.

## Source references

[R01]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/package.json
[R02]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/theme/index.ts#L20-L46
[R03]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/session-frame.tsx#L29-L408
[R04]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/context/session-tabs.tsx#L35-L498
[R05]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/metadata.tsx#L8-L167
[R06]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/packages/tui/src/component/prompt/index.tsx#L1097-L1404
[R07]: https://github.com/anomalyco/opentui/tree/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/core
[R08]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/.opencode/skills/tui-tests/SKILL.md
