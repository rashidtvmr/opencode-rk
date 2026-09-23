# BRG-TEXTBUF scratchpad

## Claim
- Task `BRG-TEXTBUF`, session `ses_brg_textbuf`, status `in-progress` (ledger verified).
- Owned file ONLY: `crates/opentui-bridge/src/text.rs`. No commit/push (task says report only).

## Source evidence (exact)
- `/Users/mymac/Projects/opentui/packages/core/src/text-buffer.ts:44-88` — `TextBuffer.setText/setStyledText/append`, length/byteSize via native.
- `text-buffer-view.ts:124-152,209-212` — `setWrapWidth/setViewport/measureForDimensions -> MeasureResult|null`.
- `editor-view.ts:56-69,311-315` — `setViewportSize/setViewport/getViewport`, `measureForDimensions->{lineCount,widthColsMax}|null`.
- `edit-buffer.ts:84-95` — `setText` resets state; `getText` 1MB max.
- `lib/styled-text.ts:10-34,46-78` — `StyleAttrs`, `StyledText{chunks}`, `applyStyle` merges fg/bg + attrs OR.
- `syntax-style.ts:206-244` — `mergeStyles(...names)`: later fg/bg win, attrs union via `createTextAttributes`.
- `types.ts:8-17` — `TextAttributes` bits: BOLD1 DIM2 ITALIC4 UNDERLINE8 BLINK16 INVERSE32 HIDDEN64 STRIKETHROUGH128.
- `types.ts:206-212` — `Highlight{start,end,styleId,priority?,hlRef?}`; `214-225 LineInfo`; `zig.ts:2359-2362 MeasureResult{lineCount,widthColsMax}`.
- `utils.ts:4-37` — `createTextAttributes` maps bool flags to bits.
- Bridge precedent: `safe_renderer.rs:30` `MAX_TEXT_BYTES=64*1024`; `layout.rs:1` `#![forbid(unsafe_code)]` + std-only standalone pattern (`renderable.rs:1-5` compiled via `rustc --test`).

## Observed scenario
- `text.rs` (219 lines) currently NOT standalone: `crate::safe_renderer` imports fail `rustc --test`; has `unsafe extern` FFI block incompatible with required `forbid(unsafe_code)`. Nothing else in `crates/` references `text::` (verified grep) so self-containment is safe.
- Existing helpers to keep: `check_text_bytes`, `clip_chars`, `wrap_text`, `clip_lines` (+ their 5 tests); `NativeHandle/INVALID_HANDLE` aliases kept, FFI `extern` block removed (decls live in `buffer.rs`/`renderer.rs`).

## Target boundary
- Pure owned model: `TextBuffer{set_text,set_styled_text,highlights,tab_width,destroy-guard}`, `StyledText/TextChunk/Style+merge_styles`, `TextBufferView{wrap,viewport,visible_lines,measure_for_dimensions}`, display-width fns (`char_width/display_width/clip_to_width/wrap_by_width/slice_by_width`), local `MAX_TEXT_BYTES/BridgeError`, `forbid(unsafe_code)`, std-only, all inputs bounded.

## Tests
- RED (frozen, appended as `textbuf_red_tests`, 10 tests): set_text roundtrip/oversize, styled spans, highlights, CJK wrap, viewport slice, measure (+zero→None), merge_styles, width-clip-not-char-count, destroyed guard.
- RED cmd: `rustc --edition 2021 --test crates/opentui-bridge/src/text.rs -o /tmp/opencode/brg_textbuf_test && /tmp/opencode/brg_textbuf_test`
- RED result: compile FAIL on missing `TextBuffer/StyledText/...` (expected).

## Decisions
- Tab in width math expands to next tab stop (matches `setTabWidth` semantics); `clip_to_width` never splits a wide char (partial col → stop before).
- `measure_for_dimensions(text,w,h)->Option<Measure>`: None on zero dims/destroyed; line_count uncapped (reports need, caller clamps to viewport).
- Word wrap: greedy by display width, over-long word falls back to char split.

## Remaining unknowns
- None for lane scope. Native width-method (`wcwidth` vs `unicode-wide`) differences stay behind FFI; pure table documented as approximation.
