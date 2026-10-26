# Context information and inspection parity

## Current user requirement — 2026-10-26

The user reports that context information displays **Unavailable**. Repair the
actual data path and match OpenCode's context accounting and presentation.
Clicking context information and running `/context` must open the same detailed
inspection experience, with the requested Claude Code-style context breakdown.

This is an active product repair requirement, not a completion claim or an
operational recovery-state record.

## Repair checklist

- [ ] Reproduce and identify why the displayed context information is unavailable.
- [ ] Inspect the user-selected OpenCode reference at
  `450e7b92412c780a7b5a75226bb8d49aaca24e22` for usage, limits, labels and layout.
- [ ] Inspect available Claude Code context references for the requested detail
  view; distinguish this requested extension from ordinary OpenCode parity.
- [ ] Connect real session/provider usage and selected-model context limits to
  the visible context information, including cache accounting without double
  counting.
- [ ] Make context clicks and `/context` open the same session-bound detail view.
- [ ] Show the actual available breakdown of system instructions, conversation,
  tool definitions/results, loaded files and memory, with source/provenance and
  measured-versus-estimated values clearly identified.
- [ ] Keep the selected session, draft, cursor and history intact while opening,
  refreshing and closing inspection; reject stale results after a tab switch.
- [ ] Test missing usage, fresh sessions, model changes, cached usage, compaction,
  tab changes and genuine backend failures without manufacturing zeroes or
  silently hiding errors.
- [ ] Verify the rendered labels and positions against the reference and execute
  click plus `/context` journeys on the exact integrated release revision.

## Starting points

- `crates/cli/src/tui_entry.rs`: `request_context_inspection`, Context page fetch,
  command routing and pointer handling.
- `crates/cli/src/native_context.rs`: inspection response parsing and rendering.
- `crates/cli/src/native_view_status.rs`: visible usage and context summary.
- `crates/server/src/context_http.rs`: session-bound inspection response.
- `docs/upcoming-features/tui-composer-context-memory.md`: historical proposal;
  its statements about the absence of a TUI are obsolete and are not current
  implementation status.

## Evidence required

Retain the observed failure, reference file/symbol evidence, meaningful regression
tests, independent review, and installed evidence. Until those checks execute,
accurate accounting and visual parity remain unproven.
