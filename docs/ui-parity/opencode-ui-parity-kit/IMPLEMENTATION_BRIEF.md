# Agent handoff: exact OpenCode v2 UI in the current Rust application

## Objective

Bring the existing Rust application's full-screen terminal UI into verified parity with the selected OpenCode source. Work on the real production UI and reuse the existing native OpenTUI bridge. Preserve user work and the current project's implementation/integration rules. This is an implementation assignment after you have read the provided source audit; the audit itself does not establish that the Rust UI passes.

## Fixed reference

- OpenCode: `450e7b92412c780a7b5a75226bb8d49aaca24e22` (package 2.1.4, 2026-10-25).
- OpenTUI: `v0.6.8`, commit `8d6065a3e60039d16c6ee697692dcfbc46d3c786`.
- Reference runtime: Bun 1.4.4, Solid 1.9.15.
- Full-screen source: `packages/tui`, including registered built-in plugins and their dependencies. Do not use mini, desktop, older screenshots, or a later moving branch as the oracle.
- v2.2.0 is comparison-only unless the user changes the target. Its OpenTUI 0.7.0, tab-pulse pacing and clipboard placeholder fix are documented differences.

The user's repository is `/Users/mymac/Projects/opencode-rk-recovery-20261005`. Read its current AGENTS files and inspect the current Git status before edits. This handoff does not override its single-writer or validation constraints. Do not replace the existing upstream lock or overwrite unrelated changes.

## Read in this order

1. `README.md` and `baseline.lock.json`.
2. `audit-implementation.md` for bridge boundaries and acceptance design.
3. `audit-layout.md`, then `audit-theme-runtime.md`.
4. `audit-composer.md`.
5. `audit-messages.md` and `audit-opentui-boundary.md`.
6. `checklist.json`, `fixtures.json`, and `upstream-test-results.md`.

Source labels in the reports link to immutable files and relevant lines. Inspect those source functions and original test fixtures while implementing; do not rely on prose alone where the behavior is algorithmic.

## Start with actual code discovery

Find current bridge APIs, renderer/tree submission, layout, theme, tabs, prompt/editor/metadata/footer, transcript projection, tool views, sidebar and focus controllers. The research session could inspect repository instructions and inventory, but detailed bridge reads were blocked by conflicting connector tunnel routing. Treat its current capabilities as unknown until you read them. Do not invent an API map or declare modules absent based on this gap.

Create a small capability table: upstream responsibility, existing Rust owner/API, status, precise gap, affected checklist IDs, and focused test. Retain already-correct implementations. Prefer logical modules within the current crates instead of introducing a replacement UI framework.

## First acceptance slice

Capture the actual upstream idle session in standard opencode dark mode at 160×48 and 80×24, then an identical Rust fixture. Include session tabs when enabled, sidebar when eligible, one user message and one assistant Markdown message, a one-line draft, agent/model/provider metadata, and context/cost fixture.

Match root/sidebar/composer surfaces #0a0a0a/#141414/#1e1e1e via semantic tokens, all outer and inner insets, the prompt's left-only split border, separate bottom lip and footer rail, reserved jump row, text and cursor cells. Repeat at terminal widths 43/44 for prompt spacing. Test sidebar eligibility at terminal widths 120/121 with horizontal tabs, or 162/163 with a 42-cell left rail; its predicate uses remaining width after that rail. Then exercise running status, interrupt arming and the return to idle. A static screenshot alone does not finish the slice.

## Non-negotiable presentation rules

- Preserve the source's separate terminal/content/pane width domains and exact strict/inclusive comparisons.
- Sidebar defaults to Context→Limit→MCP→Todo plus pinned footer; LSP or always-visible changed files are not baseline built-ins in this snapshot.
- Prompt metadata sits below textarea; provider has no preceding separator dot. Status rail sits below the bottom lip, outside the prompt fill.
- Folder/path and running scanner use exclusive left-rail branches. Usage comes from the latest eligible assistant after compaction and before revert, using its model context limit; cost is separate.
- Shift+Tab cycles agents; Tab is completion. Respect scoped commands and disabled bindings. Enter must submit once.
- Subagent status is indeterminate; execute child calls come from metadata and are not subagent sessions.
- Stable message/group/block IDs and semantic scroll anchors survive streaming, resize, backfill and tab switching.
- Markdown includes incremental fences, tables, concealment, colors/attributes and registered Mermaid/math handlers.
- Permission/form/dock states replace the normal prompt and own focus.
- Theme hue provenance and transparent state resolution must survive the Rust/native boundary.

## Testing and completion

Use fixture data and production components on both sides. Capture canonical styled-cell frames plus cursor/layout/focus information; replay input and compare semantic action traces. Inject time, await highlights, and do not run live providers or real custom scripts merely to render their cards. Treat the fixture catalog as proposed tests awaiting golden capture.

Implement one behavior slice, run its focused checks, then integrate according to the project rules. Do not weaken assertions, update every golden blindly, or count a module compiling as visual acceptance. Record deliberate source-bug fixes separately from exact baseline behavior.

Return the exact integrated Rust SHA, upstream/renderer pins, changed paths, checklist IDs actually exercised, commands/results, matched frame/capture paths, remaining differences and environmental limits. Leave unexecuted checks unverified. “100% exact” is permitted only for the declared locked matrix with zero unexplained differences and reviewed installed-terminal captures.
