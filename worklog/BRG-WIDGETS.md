# BRG-WIDGETS scratchpad

Claim: BRG-WIDGETS, ses_brg_widgets, in-progress.
Worktree: /Users/mymac/Projects/opencode-rk-bridge. Owned file: crates/opentui-bridge/src/widgets.rs.

## Source evidence
- Box: `opentui/packages/core/src/renderables/Box.ts:46` BoxRenderable (border, title, fill); border chars `core/src/lib/border.ts:39-84` (single/double/rounded/heavy, exact glyphs above).
- Text: `Text.ts:13` TextRenderable extends TextBufferRenderable; wrap/align live in text-buffer-view (wrapMode char/word/none).
- Input: `Input.ts:46` InputRenderable extends TextareaRenderable, single-line height=1, wrapMode none, newlines stripped (`:66`), maxLength enforced (`:119+`).
- Select: `Select.ts:28,66` SelectRenderable, actions move-up/down/fast/select-current, wrapSelection, selectedIndex+scrollOffset.
- TabSelect: `TabSelect.ts:27` actions move-left/right; horizontal tabs (fold into SelectWidget variant).
- ScrollBox: `ScrollBox.ts:58` ScrollBoxRenderable extends Box; scrollX/scrollY, stickyScroll, viewport culling via getObjectsInViewport.
- ScrollBar: `ScrollBar.ts:20` ScrollBarRenderable wraps SliderRenderable; scrollSize/scrollPosition/viewportSize (`:52-72`), clamped `max(0, min(max, size-viewport))`.
- Slider: `Slider.ts:20` SliderRenderable orientation value/min/max/viewPortSize, value clamped (`:49`), onChange.
- TextTable: `TextTable.ts:99` TextTableRenderable, cellPadding, showBorders/outerBorder, content[][]; widths via `text-table-width.ts:15` allocateProportionalColumnWidths (proportional water-fill).
- FrameBuffer/Markdown/Diff: out of scope (Phase-1 cut below).

## Scope boundary
Port: Box, Text, Input (single-line+cursor), Select/TabSelect, ScrollBox+ScrollBar, TextTable, Slider.
Cut (recorded): Image, EmbeddedTerminal, ASCIIFont, Code, Three/audio, Markdown, Diff, FrameBuffer. Reason: Phase-1 set only per task.

## Tests (frozen after RED)
box borders, text wrap/align, input cursor+insert/delete, select highlight move, scrollbox viewport, table widths incl wide chars, slider fill. >=9 tests.

## Decisions
Self-contained Grid/Cell in widgets.rs, no sibling imports, `#![forbid(unsafe_code)]`, std-only, bounded (MAX 256x256).
Wide chars: treat char width 2 for CJK range (U+1100..U+115F, U+2E80..U+9FFF, U+AC00..U+D7A3, U+F900..U+FAFF, U+FF00..U+FF60, U+FFE0..U+FFE6, emoji U+1F000..U+1FAFF).
Grid stores String per cell; render_to_grid dispatches enum Widget.

## Verification
- RED run: 10 passed / 4 failed (input cursor semantics, 2 row-width literals).
- GREEN: `rustc --edition 2021 --test crates/opentui-bridge/src/widgets.rs -o /tmp/opencode/brg_widgets_testG4 && /tmp/opencode/brg_widgets_testG4` → 14 passed, 0 failed (`/tmp/opencode/wbuildG4.log`), zero warnings.
- Standalone rustc, no sibling imports (only `mod tests` + comment match), 1083 lines, 14 tests, markers: BoxWidget/TextWidget/InputWidget/SelectWidget/ScrollBoxWidget/render_to_grid all present.
- Cut recorded: Image/EmbeddedTerminal/ASCIIFont/Code/Three/audio/Markdown/Diff/FrameBuffer.

## Unknowns: none.
