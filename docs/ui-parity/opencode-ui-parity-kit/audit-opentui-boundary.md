# OpenTUI 0.6.8: the exact native / TypeScript / Rust port boundary

## Scope and pins

This is a source audit, not a runtime verification of the user's Rust bridge. It identifies what that bridge must expose or reproduce; it makes no claim that a particular capability is missing from the existing implementation.

The authoritative OpenCode project snapshot is `450e7b92412c780a7b5a75226bb8d49aaca24e22`. Its root catalog pins both `@opentui/core` and `@opentui/solid` to **0.6.8**.[P1] The public OpenTUI `v0.6.8` tag resolves to commit **`8d6065a3e60039d16c6ee697692dcfbc46d3c786`**, checked out at `/tmp/opentui-selected-audit`. Every OpenTUI link below uses that immutable commit.

**The practical conclusion:** a Rust binding to OpenTUI can reuse far more than cell drawing, because this release has a native retained scene, Yoga layout, text/editor resources, terminal rendering, and session/output management. However, binding those primitives does not automatically supply the TypeScript implementations of Markdown composition, asynchronous syntax highlighting, diff construction, scrolling policy, input dispatch, or the application widgets. Those layers need an explicit implementation owner.

## 1. Ownership map derived from source

| Facility | Native implementation at this pin | TypeScript / Solid implementation at this pin | Consequence for Rust parity |
|---|---|---|---|
| Scene and layout | Context-owned retained node tree; native Yoga calculation; geometry, clip propagation, opacity, viewport filtering, paint order | `NativeScene` stages mutations and coordinates sparse host hooks; `Renderable` exposes properties/lifecycle | Reuse native scene and the same layout semantics; implement a Rust host/controller layer. |
| Cells and terminal output | Optimized buffers, renderer, sessions, presentation/output state | Host stream integration, scheduling, output completion/backpressure | A successful draw call is distinct from completed terminal presentation. |
| Text and editing | Text buffers/views, styled content, selection and editor resources | Renderable wrappers, props, events and application editing policy | Verify wrappers expose native text capabilities, not just `draw_text`. |
| Markdown | Paints the text/boxes/views composed by the host | Marked GFM parser, incremental token reuse, structured blocks, tables, custom block renderer dispatch | Port parser/compositor behavior or retain an explicit parser host. |
| Code coloring | Stores/paints styled text and syntax-style resources | Tree-sitter worker client, grammar/query assets, async generations, concealment and chunk production | Binding a syntax-style handle alone does not create highlighted Markdown. |
| Diffs | Paints native descendant resources | `diff` patch parsing plus Code/LineNumber/Text composition and split/unified policy | A reusable Rust diff controller is needed if this TS layer is removed. |
| Scrolling | Clip/viewport geometry and native terminal split-scrollback machinery | ScrollBox structure, scrollbars, sticky edges, wheel acceleration, drag autoscroll | Transcript scrolling and terminal scrollback are different facilities. |
| Events and component lifecycle | Native hit geometry, focused control paint, owner-thread resource rules | Byte framing, key/mouse/paste routing, callbacks, Solid reconciliation | Match observable input, mount/update/move/unmount behavior independently of syntax. |

The table is supported by the native API overview/header, native scene, Markdown/Code/Diff/ScrollBox classes, stdin parser, renderer dispatch and Solid reconciler.[N1], [N2], [N3], [M1], [M3], [D1], [S1], [E1], [E2], [F1]

## 2. What is genuinely native

### Scene, Yoga and measurement

`packages/native/include/opentui.h` defines roots, boxes, plain text, sliders, arrows, editors, custom nodes, text views and images. Its style interface uses **terminal display cells** for numeric dimensions and **0–100** for percentages. The scene surface requires box geometry and explicitly excludes Yoga `display:contents`. Native `scene.zig` calls `yogaNodeCalculateLayoutChecked`; it reads computed geometry, applies translations, propagates opacity, intersects clipping rectangles with border insets, and skips nodes outside configured viewport filters. Native build configuration compiles the bundled Yoga C++ sources and the Zig/C++ bridge.[N2], [N3], [N4], [N9]

This matters for exact padding and wrapping: replacing this machinery with a different flex implementation is a behavioral change even when both use the word “flex.” A faithful Rust wrapper should retain the native style vocabulary, measurement semantics, terminal-cell rounding, dirty propagation and viewport behavior, then compare geometry with upstream fixtures.

`NativeRenderable` binds a Yoga node to a native text-buffer view or editor view for measurement. A Rust scene can therefore ask the same native text implementation for wrapped intrinsic dimensions instead of approximating height with string length divided by width.[N5]

### Checked resources and frame/output protocol

The checked C interface is **experimental ABI version 1**. Upstream explicitly requires matching headers and artifacts from the same revision. Versioned records require exact structure size/version, zero reserved fields and valid flags. Context creation captures the creating OS thread; destruction from another thread fails. Handles contain context ID, slot and generation; copying a handle does not retain it. Context destruction reports busy while required owned state remains.[N1], [N2], [N6]

Native frames are a protocol, not a single unqualified paint callback: the header describes request tickets, preparation yields, host hook records, painting and admitted output. The host must acknowledge the exact request and respect phase restrictions. TypeScript's `NativeSession` separately coordinates writable-stream callbacks, drain/backpressure, output tickets and presentation completion.[N7], [N8]

**Rust recommendation:** generate or verify ABI layouts against this exact header; wrap handles with explicit ownership and borrowing rules; keep native mutation on its owner thread; translate every status into a meaningful error; prevent unwinding across callbacks; and make frame completion/output acknowledgement first-class. Keep this layer independent of OpenCode widgets so other Rust TUIs can reuse it.

## 3. What still lives above the native ABI

### Markdown is a parser plus a retained composition engine

`MarkdownRenderable` imports Marked's lexer and creates Code, Box, Text and TextTable renderables. Options include top-level versus coalesced blocks, streaming, concealment, table layout and custom render callbacks. The incremental parser reuses matching `token.raw` prefixes and, by default, keeps the final **two tokens** unstable so an unfinished token can change type. It falls back to a complete parse on incremental failure. Matching only final Markdown output misses these transition semantics.[M1], [M2]

OpenCode uses this high-level behavior for streamed responses. The Rust replacement needs stable block identities, incomplete fence/table/list handling, in-place updates when possible, final stream settlement, list-marker alignment, source-versus-rendered text mapping, selectable content and plugin blocks. Upstream tests explicitly assert object reuse while appending list items and preservation of the previous visible text while highlighting is pending.[T1]

### Highlighting spans native storage and a TypeScript worker

`CodeRenderable` is integrated as a native `text_view`, but its TypeScript controller owns highlight dirtiness, content snapshot IDs, reruns, pending promises, streaming state, concealment and conversion to styled chunks. For streaming typed content with unstyled drawing disabled, updating content invalidates highlights and requests a frame without first publishing raw unstyled text. This is part of the no-flicker contract.[M3]

The worker imports `Parser`, `Query`, `Tree` and `Language` from `web-tree-sitter`; the client owns worker startup, requests, versions and lifecycle. The native text buffer does not substitute for those grammars, highlight queries, injection handling or asynchronous cancellation rules.[M4], [M5]

**Rust recommendation:** a Rust Tree-sitter implementation can replace the worker, but parity requires the corresponding grammar/query assets, capture-to-theme mapping, injection behavior, offset conversions and stale-result rejection. Preserve the last valid styled frame until the next valid result is ready. Do not assume any Markdown crate plus a generic code highlighter yields identical wrapping, colors or streaming behavior.

### Diffs, scroll controllers and terminal scrollback

`DiffRenderable` parses patches with the JavaScript `diff` package and composes Code, LineNumber and Text renderables. It owns logical lines, added/removed/context classification, gutters and unified/split presentation. Native primitives supply the painting substrate; the hunk model and composition are host responsibilities.[D1]

`ScrollBoxRenderable` supplies wrapper, viewport, content and scrollbars. Its TypeScript controller maps Shift-wheel to the other axis, applies acceleration, retains fractional deltas until whole cells can be scrolled, maintains sticky/manual-scroll state and performs time-based selection autoscroll. Native viewport clipping/culling complements these behaviors.[S1] Separately, native `SplitScrollback` tracks published terminal rows and tail columns for split output. It is not the conversation list's scroll-position/anchor model.[S2]

### Input and Solid

The stdin parser explicitly owns byte framing and protocol recognition, while `KeyHandler` and the renderer own dispatch. It produces key, SGR/X10 mouse, paste and terminal-response events. Renderer dispatch distinguishes capability responses from keys and routes paste bytes separately. Reimplementing only printable characters and arrow keys would leave major behavior unmatched.[E1], [E2]

Solid creates core renderable instances, applies reactive properties, maps focus and event handlers, inserts/moves children, and cleans up removed instances. Rust need not reproduce JSX syntax or Solid itself. It must supply equivalent stable identity, update ordering, property removal, event replacement and resource cleanup semantics.[F1]

## 4. Reusable Rust seams to verify

These are proposed architecture boundaries, not claims about the existing bridge:

1. **Native adapter:** checked ABI bindings, Context/Session/Scene handles, resources, staged mutations, frame requests, output transport and test capture.
2. **Widget foundation:** style values, layout nodes, text views/editors, focus/event propagation, clocks and scheduling, scroll controller, selection and clipboard.
3. **Document widgets:** incremental Markdown tree, syntax service, styled chunks, tables, diff/gutter renderer, plugin block interface.
4. **OpenCode presentation:** typed transcript/tool/subagent rows, composer, status rails, sidebar, tabs, dialogs and domain-event reducers.

Keep parsing and backend work off the owner thread if useful, but publish immutable results through a versioned message back to the owner. Use explicit byte offsets, Unicode scalar offsets, grapheme boundaries and terminal-cell positions; never let a generic `usize` hide which unit an API expects. The header's display-cell rules and context/thread restrictions make these distinctions consequential.[N2]

### Capability checklist

- [ ] **FFI-01** The selected library binary, header, generated layouts and captured baseline all identify the exact OpenTUI commit.
- [ ] **FFI-02** Rust bindings expose native scene creation, ordered moves, destruction, styles, measurement, clip/viewport configuration, focus, text views and editors.
- [ ] **FFI-03** Owned resources, shared resources, borrows, stale generations and failed construction have explicit lifetime behavior.
- [ ] **FFI-04** Host hook/frame requests, backpressure, output failure and shutdown are implemented and tested as state transitions.
- [ ] **FFI-05** Wrapped measurement uses native text views or has demonstrated equivalent Unicode/cell results.
- [ ] **FFI-06** Markdown reuses stable blocks and handles streaming changes without blank frames or raw-marker flashes.
- [ ] **FFI-07** Highlighting preserves grammar/query behavior, semantic colors, concealment, links and source mappings; late results cannot update replaced/destroyed views.
- [ ] **FFI-08** Diff parsing, line numbers, gutters, selection and resize behavior match the reference component.
- [ ] **FFI-09** Scroll position, stick-to-bottom, manual detachment, resize, wheel acceleration, Shift-wheel and drag selection are independently verified.
- [ ] **FFI-10** Raw input or the chosen typed-input adapter preserves paste, modifiers, terminal protocols, mouse movement and capability responses.
- [ ] **FFI-11** Component replacement removes old handlers, focus restores correctly and keyed moves preserve component state.

## 5. Verification that proves this boundary works

Use upstream test semantics as the contract, then replay the same input/state/clock/viewport fixtures against the Rust host. Source inspection is evidence of intended behavior; it is not proof that the Rust implementation already passes.

| Test family | Specific acceptance cases | Upstream starting points |
|---|---|---|
| Native ownership | stale handles, shared text lifetime, pooled resource reset, wrong-thread failures, busy teardown | Native context tests and checked ABI fixtures.[T2], [N2] |
| Layout | borders/padding/gaps, percentages, wrap, resize, measurement callbacks, callback failure isolation | `yoga-upstream/` generated tests and `yoga-callback-boundary.test.ts`.[T3] |
| Streaming documents | stable list objects, previous text visible while highlights wait, no unconcealed frame, complete final table row | Markdown tests; record intermediate frames, not only final screenshot.[T1], [T4], [T7] |
| Asynchronous lifetime | clear/replace Markdown while highlight request is pending; destroy before completion; custom block throws midway | Native-scene Markdown tests assert old nodes destroyed and registry cleanup.[T5] |
| Scrolling lifecycle | wheel/key/content-shrink callback destroys controller; retained native child does not call destroyed owner | Native-scene scroll lifecycle tests.[T6] |
| Framework semantics | insert, move, remove, portals, focus, events and dynamic collections | Solid reconciler plus corresponding `packages/solid/tests/` fixtures.[F1] |

Compare terminal **cells and their styles**, geometry, selected/copied text, active focus, hit targets and event results. Use a manual clock and controllable highlight completion so transient frames are deterministic. Add terminal-output integration coverage for transport/backpressure separately from cell snapshots. Only call the port equivalent after these layers pass the same fixtures at the target widths and theme settings.

No upstream test suites, Rust tests or interactive terminals were run for this boundary appendix. The statements above come from the pinned source and inspected test bodies.

## Immutable references

[P1]: https://github.com/anomalyco/opencode/blob/450e7b92412c780a7b5a75226bb8d49aaca24e22/package.json#L50-L65
[N1]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/native/README.md#L1-L75
[N2]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/native/include/opentui.h#L10-L105
[N3]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/native/src/scene.zig#L1096-L1106
[N4]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/native/build.zig#L377-L392
[N5]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/native/src/native-renderable.zig#L8-L100
[N6]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/native/src/context-abi.zig#L35-L101
[N7]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/native/include/opentui.h#L532-L627
[N8]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/core/src/NativeSession.ts#L760-L812
[M1]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/core/src/renderables/Markdown.ts#L1-L165
[M2]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/core/src/renderables/markdown-parser.ts#L1-L79
[M3]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/core/src/renderables/Code.ts#L47-L170
[M4]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/core/src/lib/tree-sitter/parser.worker.ts#L1-L45
[D1]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/core/src/renderables/Diff.ts#L1-L120
[S1]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/core/src/renderables/ScrollBox.ts#L510-L712
[S2]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/native/src/split-scrollback.zig#L1-L87
[E1]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/core/src/lib/stdin-parser.ts#L1-L100
[E2]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/core/src/renderer.ts#L3420-L3488
[F1]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/solid/src/reconciler.ts#L29-L290
[T1]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/core/src/renderables/__tests__/Markdown.test.ts#L1553-L1639
[T2]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/native/src/tests/context_test.zig#L57-L120
[T3]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/core/src/tests/yoga-callback-boundary.test.ts#L1-L22
[T4]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/core/src/renderables/__tests__/Markdown.test.ts#L3842-L3876
[T5]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/core/src/tests/renderer.native-scene-markdown.test.ts#L16-L100
[T6]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/core/src/tests/renderer.native-scene-scroll-lifecycle.test.ts#L49-L128
[N9]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/native/src/scene.zig#L1543-L1668
[M5]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/core/src/lib/tree-sitter/client.ts#L40-L147
[T7]: https://github.com/anomalyco/opentui/blob/8d6065a3e60039d16c6ee697692dcfbc46d3c786/packages/core/src/renderables/__tests__/Markdown.test.ts#L4170-L4203
