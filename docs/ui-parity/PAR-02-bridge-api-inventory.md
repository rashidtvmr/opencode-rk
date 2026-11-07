# PAR-02 — Rust bridge/component API inventory

**Base:** `7088398873e835bbe303cec43dcd4ad9f3606712`
**Upstream oracle:** `/Users/mymac/Projects/tmp/opencode` at
`450e7b92412c780a7b5a75226bb8d49aaca24e22`
**Scope:** source inventory for the capability rows in
`docs/ui-parity/opencode-ui-parity-kit/audit-implementation.md:85-96`.

This is an evidence inventory, not an inferred design. `PRESENT` means a Rust
API exists in this checkout; `CONSUMED` means a production CLI call site was
located. `PARTIAL` means the named capability has a concrete subset but the
row's proof requirement is not fully represented by the API. `ABSENT` means no
Rust API/consumer was found after inspecting the bridge modules and CLI search;
it is not a claim about the native library's private implementation.

## Exported bridge surface

`crates/opentui-bridge/src/lib.rs:1-21` declares the complete bridge module set;
`lib.rs:23-31` re-exports `CellBuffer`, `Editor`, `EditorView`, `Rgba`, image
models, `MarkdownDoc`, `RendererSpec`, `Renderer`, `BridgeError`, and
`ScrollBox`.

| Capability / actual Rust API | Live consumer | Status and evidence | Relevant tests |
|---|---|---|---|
| Native ABI and handle ownership: `BridgeHandle::claim/live/close` (`handle.rs:32-89`), `Arena<T>` generation handles and owner-thread checks (`handle.rs:226-401`), `native_identity::identify/probe_installed` (`native_identity.rs:272-355`), ABI declarations in `renderer.rs:72-130` | `tui_entry.rs:9676-9757,9833-9864` verifies identity and creates the renderer; `terminal_host.rs:256-299` owns the renderer lease | **PRESENT + CONSUMED**, with explicit native artifact identity and safe handle boundary. Rust cannot prove native-side size/alignment beyond the checked ABI tests. | `renderer::tests::renderer_spec_size_stable`, `build_options_layout_matches_extern_struct`; CLI `verified_artifact_passes_boot_with_identity` (`tui_entry.rs:9701+`) |
| Retained scene and layout: `Scene::{new,insert,remove,set_style,set_text,set_clip_children,layout,measure,hit_test,paint}` (`scene.rs:331-845`); `Style` supports width/height/padding/border/gap/direction/offset (`scene.rs:125-153`); row/column constraints `split_row/split_col` (`layout.rs:276-289`) | No production CLI call to `Scene` was found; live CLI layout uses `native_layout::SessionRegions::compute` (`native_layout.rs:254-320`) and paints via `NativeRenderer` in `tui_entry.rs:5961-6210` | **PRESENT, consumer not established for Scene**. The component geometry API exists, but it is not evidence that the live page is scene-backed. | `scene` module tests around layout/focus/hit/paint; `native_layout::tests::lay_01_regions_tile_standard_viewports` |
| Text/editor measurement: pure `display_width/wrap_cells/clip_lines` (`text.rs:23-65,222-259`); `Editor` editing/selection/cursor/offset APIs (`editor.rs:115-421`); `EditorView::{rows,visible_rows,visual_cursor}` (`editor.rs:630-812`) | No CLI use of bridge `Editor`/`EditorView`; CLI has its own prompt/layout modules (`native_prompt_layout.rs`, `native_prompt_pane.rs`) and uses `NativeRenderer` | **PRESENT, bridge consumer not established**. Grapheme/cell helpers are available; no bridge call site proves the live editor uses them. | `editor::tests::ffi05_editor_view_wraps_wide_chars`; `text` boundary/wrap tests |
| Styling: `Rgba::{from_hex,from_f32,from_i32}` (`color.rs:137-211`), `Cell`/`CellGrid::set/fill/blit` (`buffer.rs:71-260`), native styled drawing `Renderer::{draw_text_styled,fill_rect,set_cursor_style}` (`safe_renderer.rs:906-1033`) | `tui_entry.rs:5961-6210` passes styled render commands; `native_theme_runtime.rs` builds semantic palettes | **PRESENT + CONSUMED** for RGBA, cells, and renderer styling. Semantic role selection is CLI-owned, not a bridge theme API. | `safe_renderer::tests::styled_text_rejects_controls_and_out_of_bounds`; buffer/color tests; CLI theme tests |
| Events: typed `TerminalInput::{feed,next_event,flush_escape}` and `TerminalEvent` (`terminal_input.rs:241-383`); `input::{InputEvent,FocusRing}` (`input.rs:50-149`) | `native_input_decoder.rs:1473-1646` compares the CLI decoder with `TerminalInput`; main input path constructs CLI decoder at `tui_entry.rs:10216` | **PRESENT + CONSUMED** for byte framing/parsing comparison. FocusRing has no production CLI consumer found. | `native_input_decoder::tests::ffi11_*`; `terminal_input` parser tests; CLI SGR/paste/Kitty tests |
| Transcript scrolling: `ScrollBox::{new,set_viewport,set_content_size,scroll_to_y,scroll_by,wheel,page,home,end,content_row_at,vertical_thumb,horizontal_thumb}` (`scroll.rs:76-345`); CLI `TranscriptPage` and anchored pages (`native_transcript.rs:313-510`) | `native_view.rs:7758-7766` directly uses bridge `ScrollBox`; live transcript rendering uses `native_transcript` (`tui_entry.rs` call sites) | **PRESENT + CONSUMED**. Bridge provides primitive scroll geometry; transcript policy/anchors remain CLI-owned. | `native_view` scroll test; `native_transcript::tests::anchored_page_is_pinned_by_line_identity`, `page_pins_to_bottom_and_scrolls` |
| Markdown/highlighting: `MarkdownDoc::{new,set_source,blocks,unstable_count}` (`markdown.rs:151-247`); `HighlightTargets::{create,set_source,request,complete,frame_text,styled_runs}` (`highlight.rs:683-926`); CLI `MarkdownFlow` and plugin registry (`native_markdown_flow.rs:477-1175`) | `native_markdown_flow.rs:71-75,620-760,1031-1039`; chat creates/configures flow at `native_view_chat.rs:3047-3079` | **PRESENT + CONSUMED**. Identity producer and streaming parser are wired; native Markdown FFI is explicitly not claimed (`markdown.rs:15-20`). | `native_markdown_flow::tests::ffi06_bridge_doc_is_block_identity_producer`; `md_001`–`md_007`; highlight stale-result tests |
| Animation/frame scheduling: `FrameController::{request,prepare,paint,admit_output,drain_output,present,request_shutdown,complete_shutdown}` (`frame.rs:210-466`) | No production CLI call to `FrameController` found. CLI has frame/tick handling in `native_host.rs:590-610` and `tui_entry.rs` | **PRESENT, consumer not established**. The strict bridge lifecycle is tested but does not establish live animation wiring. | `frame::tests::happy_path_is_a_strict_phase_sequence`, output backpressure and shutdown tests |
| Capture/comparison: `native_capture::{Frame,FrameCell,Cursor,HitTarget,Capture,compare}` (`crates/cli/src/native_capture.rs:114-376`) | capture/test adapters call `Frame::paint/canonical/compare`; this is CLI-side, not bridge-side | **PRESENT + CONSUMED as CLI harness**, but no bridge API exports a portable styled-cell capture. `safe_renderer::snapshot_text` (`safe_renderer.rs:1089-1127`) is text-only. | `native_capture::tests::par04_style_only_differences_do_not_hide_behind_text_equality`, cursor/hit/clipping tests |
| Terminal lifecycle: `Renderer::{create/create_memory,setup_terminal,restore_terminal_modes,suspend,resume,enable_mouse,disable_mouse,enable_kitty_keyboard,disable_kitty_keyboard,resize,clear_terminal,close}` (`safe_renderer.rs:497-792`); input and title/cursor APIs at `safe_renderer.rs:813-1070` | `tui_entry.rs:9864-10216` and renderer paint helpers; `terminal_host.rs:256-299` enforces ownership | **PRESENT + CONSUMED** for renderer lifecycle, mouse, Kitty keyboard, resize, cursor, title, and memory rendering. Clipboard/OSC links and image protocol are not exposed by this Renderer API; no such bridge consumer was found. | `safe_renderer::tests::render_once_content_present`, lifecycle/error tests; `terminal_host` ownership tests |

## Additional concrete component APIs

These APIs are in the bridge and are relevant to the kit even where they do
not complete one of the ten capability rows:

* Diff model/layout: `FilePatch::parse`, `DiffLayout::{new,resize,visible,row_line}`
  (`diff.rs:188-1188`) are consumed by `native_diff_view.rs:420-472,2531`.
* Images: `ImageData`, `ImageRenderable`, and `Gallery` (`image.rs:68-412`)
  provide validation, cell sampling, paint, navigation, and click outcomes;
  no CLI bridge call site was found in the inspected production path.
* Native frame schema: CLI `Frame::{new,paint,canonical}` and hit/cursor fields
  are at `native_capture.rs:114-221`; this is deliberately separate from the
  bridge's native renderer buffer.
* Retained resources: `SharedResource` and `Arena` (`handle.rs:226-485`) provide
  bounded generation-checked ownership; they are not a substitute for proof
  that every component is mounted in the live page.

## Upstream mapping

The upstream responsibilities were checked at the pinned checkout, including
`packages/tui/src/routes/session/message-parts.tsx` (Markdown composition),
`packages/tui/src/plugin/markdown.ts` (renderer/plugin map), and the OpenTUI
core under `packages/core`. The Rust mapping above intentionally distinguishes
component models from live consumers: a bridge module's existence is never
reported as a wired parity capability without a call-site trace.

## Result

PAR-02 is **IMPLEMENTED as an inventory artifact**: every capability row from
the kit table is mapped to an actual Rust symbol, a live consumer (when found),
or an explicit “consumer not established” result. No status is based solely on
filenames or the earlier audit's connector limitation.
