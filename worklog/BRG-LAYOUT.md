# BRG-LAYOUT — flexbox subset (yoga web-default semantics)

Claim: BRG-LAYOUT / ses_brg_layout / 2026-09-23. Owned file ONLY: crates/opentui-bridge/src/layout.rs.

## Source evidence
- `/Users/mymac/Projects/opentui/packages/core/src/yoga.ts`: enums Align..Wrap ~9-79; `Node.createForOpenTUI` ~469 (native `yogaNodeCreateForOpenTUI` via zig.ts ~4815); `calculateLayout(w,h,LTR)` ~559 (NaN=auto via normalize); `setMeasureFunc` ~961 + `setDirtiedFunc` ~983 (single slot, shared native/JS); `parseValue`: number->Point, "auto"->Auto, "N%"->Percent, undefined->Undefined.
- `lib/yoga.options.ts`: `parseFlexDirection` default Column (~188); row/col/reverse.
- `Renderable.ts`: default flexGrow 0 (~724-728), flexShrink web-default 1 BUT forced 0 when explicit numeric w/h (~730-740); setters width/height/margin/padding/min/max/flexBasis ~631-1081; `calculateLayout()` = yoga calc with (width,height,LTR) ~1863.
- Native `createForOpenTUI` zig source not in repo (no packages/zig dir); web-default semantics assumed = yoga defaults: Column, Stretch align, grow 0, shrink 1, basis auto.

## Target boundary
Native flexbox subset in layout.rs (extended in place, existing Rect/Constraint/ShellRegions untouched). Markers: `LayoutNode`, `calculate_layout`, `set_measure_func`, `FlexDirection`, `LayoutRect`. No new deps, std-only, forbid(unsafe_code). Standalone: `rustc --edition 2021 --test`.

## Design (web-default)
- Dim {Undefined, Auto, Point(f32), Percent(f32)} mirrors yoga Value/Unit.
- Style: direction default Column, grow 0, shrink 1, basis auto, w/h auto, min/max undefined, margin/padding 0.
- Layout per axis: parent-imposed flex length wins (already folded style-basis+grow/shrink) > own style pct/pt (resolved once vs parent avail) > measure > fill avail. Cross default stretch to inner. Margin offsets, padding insets. min/max clamp per axis. Grow uses max-clamp redistribution loop; shrink weighted shrink*basis. Reverse mirrors positions.
- Dirty: style setters + set/unset_measure_func + mark_dirty set dirty + fire dirtied; calculate_layout clears to clean.
- Fixes during GREEN: (1) max-clamp must redistribute surplus to unfrozen growers, not just clamp in place; (2) percent resolved exactly once by the node itself (parent passes inner avail, child imposes flex length) — earlier double-resolve shrank pct children.

## Tests (19 total: 11 pre-existing + 8 new flex_tests)
column stacking, row grow, shrink overflow, padding inset, measure leaf + dirtied recompute, min clamp, max clamp, percent+auto. RED run: type errors (LayoutNode etc. undeclared, expected — impl absent). GREEN: 19 passed, 0 failed, zero warnings.

## Decisions
- Measure sig simplified to Fn(f32,f32)->(f32,f32) (avail w/h -> size); no MeasureMode enum.
- No wrap/justify/align-self/absolute/border/gap/aspect-ratio/RTL/Display (follow-up).
- f32 layout separate from u32 Rect solver (kept, untouched).

## Uncovered yoga errata (follow-up, NOT implemented)
Errata enum (StretchFlexBasis, AbsolutePositionWithoutInsetsExcludesPadding, AbsolutePercentAgainstInnerSize), ExperimentalFeature::WebFlexBasis, wrap, justify/align-content/self, absolute positioning, aspect-ratio, border/gap, RTL direction, Display::None/Contents. No test goldens cover them.
