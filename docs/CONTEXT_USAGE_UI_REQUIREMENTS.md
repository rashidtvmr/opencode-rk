# User-selected `/context` interface

Explicit user requirement, 2026-10-26: reproduce the Claude Code `/context`
layout supplied in the conversation text and attached screenshot. This is an
approved OpenCode Rust extension and supersedes the earlier inventory-only
presentation. Ordinary OpenCode parity remains pinned to
`450e7b92412c780a7b5a75226bb8d49aaca24e22`.

## Presentation

- Heading: **Context Usage**.
- Colored category usage grid, approximately 20 columns by 10 rows at the
  supplied desktop-terminal width; adapt to available terminal cells.
- Beside the grid: actual selected model identity and reported context capacity,
  then used/capacity tokens and utilization percentage.
- **Estimated usage by category**: System prompt, System tools, MCP server
  instructions, Memory files, Skills, Messages, and Free space. Each category
  has a consistent grid/legend color, token count and capacity percentage.
- Below the summary: **MCP tools · /mcp**, **Memory files · /memory**,
  **Skills · /skills** with built-in grouping when applicable, followed by
  **Suggestions** supported by actual observed usage.
- Use actual names and paths in bounded tree-style detail lists, as illustrated
  by the user's screenshot. Do not hardcode the example model, counts, skills,
  MCP names, AGENTS.md size or suggested savings.
- `/context` and clicking the context indicator open the same display. There is
  no `/contextclick` command. Preserve composer draft, cursor and session binding.

## Accounting

Categories describe the actual assembled request, including tool schemas and
instruction sources, rather than only the currently fetched inventory page.
Label estimates and their provenance. Keep provider-reported usage distinct from
estimated category totals. Inclusive provider totals must not add cache/reasoning
again. Avoid duplicate counting sources across categories.

Unknown values are **unreported**, not fabricated zero. Unknown capacity must
retain known usage and must not produce an invented percentage or free space.
Explicit measured zero is distinguishable from missing data. Partial coverage
must be visible; do not present a partial category sum as the whole request.

Suggestions must derive from captured evidence. No fabricated savings percentages
or redundant file-read claims. Retain authentication, bounded responses, safe
text rendering and existing context-selection protection.

## Verification

Check wide and narrow layouts, category/grid consistency, Unicode cell widths,
unknown and zero values, capacity overflow, measured-versus-estimated provenance,
and real slash/mouse entry points. A source patch or snapshot definition alone
does not establish installed acceptance.
