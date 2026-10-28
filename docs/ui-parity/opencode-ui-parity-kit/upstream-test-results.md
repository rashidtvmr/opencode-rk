# Executed upstream UI baseline tests

## Result

**224 tests passed, 0 failed, across 28 existing upstream test files; 2 snapshots and 1,143 assertions. Bun reported 29.65 seconds for the final complete run. Exit status: 0.**

This validates a focused baseline of the selected OpenCode implementation in this Linux test environment. It does **not** validate the user's Rust bridge/runtime, establish cross-implementation visual equality, or prove coverage of every OpenCode UI state. The Rust acceptance suite still needs the comparison fixtures and cell/style captures specified in the implementation guide.

## Exact target and execution context

- Upstream commit: `450e7b92412c780a7b5a75226bb8d49aaca24e22` (the user's project-selected v2 source).
- Source checkout: `/tmp/opencode-selected-audit`.
- Test working directory: `/tmp/opencode-selected-audit/packages/tui`.
- TUI package version: `2.1.4`.
- OpenTUI core, solid, keymap: `0.6.8`.
- Bun: `1.4.4`, executable `/tmp/opencode-audit-bun/bun-linux-x64/bun`.
- Test command honors upstream package's 30,000 ms per-test timeout and reads its normal preload files.
- Upstream `.opencode/skills/tui-tests/SKILL.md` and applicable AGENTS instructions were read before execution. Tests ran from the package directory, without changing assertions or production components.
- Required fixture-server permission was granted for disposable in-process `Bun.serve({port: 0})` listeners. No user model session or API billing was involved in these synthetic fixtures.

## Scope and per-file results

The selection exercises parent geometry; horizontal, vertical and compact tabs; tab state and mouse behavior; right sidebar and footer; prompt editing/key routing; assistant Markdown streaming; plugin Markdown; inline tools and Code Mode summaries; session row grouping; theme resolution; panel focus; and bounded long-history behavior. Existing tool snapshot fixtures cover narrow wrapping and expanded errors. The final run uses the real production components selected by those tests.

| Existing file, relative to `packages/tui` | Passed | Failed | Sum of reported test milliseconds |
|---|---:|---:|---:|
| `test/command-selection.test.tsx` | 1 | 0 | 3090.21 |
| `test/session-panel.test.tsx` | 16 | 0 | 7392.86 |
| `test/panel.test.ts` | 4 | 0 | 0.29 |
| `test/plugin-markdown.test.tsx` | 3 | 0 | 167.25 |
| `test/prompt-paste.test.tsx` | 1 | 0 | 170.14 |
| `test/prompt-shell-failure.test.tsx` | 1 | 0 | 331.47 |
| `test/session-home.test.tsx` | 17 | 0 | 9967.85 |
| `test/prompt-offset.test.tsx` | 2 | 0 | 280.39 |
| `test/markdown-streaming.test.tsx` | 2 | 0 | 34.99 |
| `test/component/session-tabs-marquee.test.ts` | 4 | 0 | 1.53 |
| `test/component/session-tabs-mouse.test.tsx` | 4 | 0 | 469.76 |
| `test/component/tab-pulse.test.tsx` | 8 | 0 | 661.01 |
| `test/component/session-tabs-rail.test.tsx` | 1 | 0 | 359.86 |
| `test/component/session-tabs-status.test.tsx` | 2 | 0 | 875.72 |
| `test/ui/layout.test.ts` | 3 | 0 | 0.10 |
| `test/context/session-tabs.test.tsx` | 25 | 0 | 4321.01 |
| `test/context/session-tabs-model.test.ts` | 33 | 0 | 4.27 |
| `test/context/local-selection.test.tsx` | 9 | 0 | 242.51 |
| `test/feature-plugins/sidebar-footer.test.tsx` | 5 | 0 | 87.66 |
| `test/feature-plugins/sidebar-context.test.tsx` | 2 | 0 | 8.13 |
| `test/feature-plugins/prompt-footer.test.tsx` | 2 | 0 | 45.91 |
| `test/util/tool-display.test.ts` | 11 | 0 | 0.29 |
| `test/theme/v2/resolve.test.ts` | 21 | 0 | 72.83 |
| `test/cli/tui/session-rows.test.ts` | 25 | 0 | 3.93 |
| `test/cli/tui/collapse-tool-output.test.ts` | 1 | 0 | 0.16 |
| `test/cli/tui/composer-keymap.test.tsx` | 7 | 0 | 302.98 |
| `test/cli/tui/inline-tool-wrap-snapshot.test.tsx` | 13 | 0 | 26.33 |
| `test/cli/tui/prompt-interrupt-status.test.tsx` | 1 | 0 | 3.51 |

Per-test durations do not sum exactly to total wall time because import/setup/runner overhead is separate. Time is a run observation, not a performance acceptance threshold.

## Reproduction command

Run from the working directory above after installing the selected checkout's dependencies. This was the final executed command:

```bash
/tmp/opencode-audit-bun/bun-linux-x64/bun test --timeout 30000 \
  test/ui/layout.test.ts \
  test/context/session-tabs-model.test.ts \
  test/panel.test.ts \
  test/component/session-tabs-marquee.test.ts \
  test/component/session-tabs-status.test.tsx \
  test/component/session-tabs-rail.test.tsx \
  test/component/session-tabs-mouse.test.tsx \
  test/context/session-tabs.test.tsx \
  test/feature-plugins/sidebar-context.test.tsx \
  test/feature-plugins/sidebar-footer.test.tsx \
  test/feature-plugins/prompt-footer.test.tsx \
  test/cli/tui/composer-keymap.test.tsx \
  test/prompt-offset.test.tsx \
  test/prompt-paste.test.tsx \
  test/prompt-shell-failure.test.tsx \
  test/cli/tui/prompt-interrupt-status.test.tsx \
  test/command-selection.test.tsx \
  test/context/local-selection.test.tsx \
  test/markdown-streaming.test.tsx \
  test/plugin-markdown.test.tsx \
  test/cli/tui/inline-tool-wrap-snapshot.test.tsx \
  test/cli/tui/collapse-tool-output.test.ts \
  test/cli/tui/session-rows.test.ts \
  test/theme/v2/resolve.test.ts \
  test/component/tab-pulse.test.tsx \
  test/session-panel.test.tsx \
  test/session-home.test.tsx \
  test/util/tool-display.test.ts \
  > /workspace/scratch/0e4e09d29100/upstream-focused-tests-final.log 2>&1
```

## Environment failures found and resolved before the final run

The first full run, inside the default sandbox, reported **184 pass / 40 fail** in **10.58 seconds**. Thirty-eight failures came from `EPERM: operation not permitted, listen` at the fixture's `Bun.serve` call. These were environment restrictions, before the intended UI assertions could run. The other two were RGBA constructor-identity assertions in theme resolution.

A six-file retry with local-listener permission exposed the same dependency-identity problem in actual app rendering: the prompt scanner received a color object from another loaded copy of OpenTUI; its `instanceof RGBA` check failed, and it attempted hex-string parsing (`hex.replace is not a function`). This redundant run was stopped with SIGINT after identifying the shared cause. It is not counted as successful validation.

The filtered isolated dependency installation contained two physical copies of `@opentui/core@0.6.8` and two copies of `@opentui/solid@0.6.8`, under different peer-dependency identities. Before adjusting generated dependencies, every package file was compared by SHA-256:

- Both core copies: 197 files each, identical file sets and content, both version 0.6.8.
- Both solid copies: 49 files each, identical file sets and content, both version 0.6.8.
- Keymap: one canonical 0.6.8 package, 94 files.

Only four ignored `node_modules` symlinks were normalized so all application packages used the same constructor/renderable identities. No package bytes, production sources, tests, lockfile, or snapshots were altered. The changes were:

| Generated symlink | Adjustment |
|---|---|
| `packages/theme/node_modules/@opentui/core` | Point to TUI's existing canonical core 0.6.8 copy (`core@0.6.8+d25f280c761c3e91`). |
| `packages/plugin/node_modules/@opentui/core` | Point to that same canonical core copy. |
| `packages/plugin/node_modules/@opentui/solid` | Point to TUI's existing canonical solid 0.6.8 copy (`solid@0.6.8+80b887512fe9ffc6`). |
| The other solid copy's `node_modules/@opentui/core` link | Point to the same canonical core copy. |

The complete original targets and replacements are recorded in `upstream-dependency-normalization.json`. After this environment-only correction, the same 28-file gate was run once in full and passed all 224 tests, including both original theme failures and all fixture-server cases. This is a conditional baseline under the normalized disposable installation; a future reproduction should ensure one loaded OpenTUI module identity, rather than hiding these errors or weakening tests.

## Repository integrity and retained evidence

After installation, dependency normalization and all tests, `git status --porcelain=v1` returned no entries, `git diff --numstat` was empty, and `git rev-parse HEAD` still returned the selected commit above. Thus the tracked upstream checkout remained unchanged.

Evidence retained alongside this report:

- `upstream-focused-tests-final.log`: full successful run.
- `upstream-focused-tests.log`: initial sandbox/constructor failures.
- `upstream-fixture-retry.log`: interrupted diagnostic retry exposing constructor-identity impact.
- `upstream-dependency-normalization.json`: package checks and four exact symlink adjustments.

No broad whole-workspace test run, Rust build, Rust visual test, private-project mutation, upstream-code fix, or baseline snapshot update was performed by this test task.
