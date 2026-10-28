# OpenCode v2 → Rust UI parity kit

Research date: **28 October 2026**.

This kit specifies the full-screen OpenCode UI at the exact source already selected by your project: **450e7b92412c780a7b5a75226bb8d49aaca24e22** (package 2.1.4, OpenTUI 0.6.8). It separately compares release v2.2.0. The user-facing report is `opencode-ui-parity-guide.html`; all detailed chapters are also plain Markdown for an implementation agent.

## What is included

- **223 numbered acceptance obligations**, all initially unverified for Rust.
- **196 deterministic fixture specifications** covering viewport boundaries, state transitions, Unicode, streaming, tools, forms and theme rules. They await upstream golden capture and Rust execution.
- **220 validated immutable source URLs**, with hashes and line counts in `source-map.json`.
- Full inventory of **298 TS/TSX files** under upstream `packages/tui/src`, explicitly separating cited regions, unexamined indexed files, mini frontend and developer stories.
- Executed reference evidence: **224 passed / 0 failed across 28 upstream files**, 2 snapshots and 1,143 assertions. This is not a Rust parity result.
- A source-based native/TS ownership map and reusable Rust implementation plan.
- `IMPLEMENTATION_BRIEF.md`, ready to give your coding agent together with the kit.

## Start here

1. Read the implementation/acceptance plan, then layout, theme and composer chapters.
2. Use the transcript/tool and dialog appendices for the remaining surfaces.
3. Read the OpenTUI boundary before changing the bridge.
4. Have the agent inspect your actual current Rust APIs and identify demonstrated gaps. Bridge internals could not be read reliably in this session because two connectors share a Secure Tunnel ID; current capability claims remain unknown.
5. Capture the pinned upstream production components using the provided fixtures, implement one behavior slice, and compare styled cells and action traces against the same fixtures.

## Files

| File | Purpose |
|---|---|
| `baseline.lock.json` | Product/renderer/runtime/theme target and evidence boundary |
| `audit-implementation.md` | Rust component boundaries, sequence, differential test design and completion criteria |
| `audit-layout.md` | Exact root/home/session geometry, tabs, sidebar, scrolling and responsive rules |
| `audit-theme-runtime.md` | Concrete colors, semantic tokens, syntax/diff, cursor and Unicode |
| `audit-composer.md` | Prompt tree, metadata fitting, rail, scanner, usage, docks and editing |
| `audit-messages.md` | Transcript grouping, tools, Markdown, code, subagents, execute, forms, diffs and motion |
| `audit-dialogs-overlays.md` | Shared dialogs, selectors, commands, menus and lifecycle overlays |
| `audit-opentui-boundary.md` | Verified native versus TS responsibilities and bridge capability gates |
| `checklist.json` | Machine-readable acceptance obligations and fixture cross-links |
| `fixtures.json` | Semantic scenario inputs/actions/expected rules; not upstream wire payloads or captured goldens |
| `ui-source-inventory.json` | Source coverage inventory; avoids claiming every indexed file was deeply inspected |
| `source-map.json` | Immutable links, path/line validation and source hashes |
| `upstream-test-results.md` | Exact executed command, counts, environment adjustment and limits |
| `upstream-focused-tests-final.log` | Final successful reference-test output |
| `upstream-dependency-normalization.json` | Four ignored temporary dependency symlink changes used to remove duplicate module identity |
| `IMPLEMENTATION_BRIEF.md` | Agent-ready implementation assignment |

## Interpretation

This kit is a specification and reference audit, not a completed Rust implementation. No user project files were changed. Source-backed checklists do not prove equality of the installed app. “100% exact” means zero unexplained differences across a declared locked fixture matrix, with reviewed installed-terminal captures; optional themes, plugins and approved extensions need their own profiles.

The HTML guide is self-contained and includes a searchable checklist. Checkbox selections are only your manual local browser notes; they do not change the authoritative `unverified_rust` statuses or claim test evidence. The guide can export those notes separately.

Original prose is written for this audit. Source references concern MIT-licensed OpenCode and OpenTUI; their notices are included. Source snippets and fixture-derived rules retain attribution through immutable links. No full third-party repository or user source code is bundled.
