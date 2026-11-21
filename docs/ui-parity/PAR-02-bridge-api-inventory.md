# PAR-02 — Rust bridge/component API inventory

**Task:** `UI-PARITY-PAR24-BRIDGE-INVENTORY-20261121`
**Base SHA:** `6aee80ce8f6535eaf2806676ae3e416c87e7bf29` — named by the dispatch and
attested in-workspace by the lease record `.delivery-owner.json:1` (`"base"` field
together with `"task": "UI-PARITY-PAR24-BRIDGE-INVENTORY-20261121"` and
`"feature_ref": "refs/heads/recovery/feature-23"`). The two agree. This task did
**not** re-read `git rev-parse HEAD`, because the worktree Git metadata directory
(`/Users/mymac/Projects/opencode-rk/.git/worktrees/author-feature-23`) is outside
this workspace's readable scope; the base is therefore labelled lease-attested,
not self-verified. The verifiable, in-repo provenance for the *upstream pin* is
the compiled-in constants in §1 (`native_identity.rs:34-40`). What this workspace
**cannot** attest is the two task-cited gap-evidence files
(`evidence/session-20261121/GAPS_ACTIONABLE.md`, `verify/batch-02-PAR-13-24.md`),
which are absent here (see §5).
**Selected OpenCode pin:** `450e7b92412c780a7b5a75226bb8d49aaca24e22` — the
pinned reference checkout at `/Users/mymac/Projects/tmp/opencode` (not readable
from this workspace; §4 records that this task did not re-inspect it).
**Selected OpenTUI pin:** `8d6065a3e60039d16c6ee697692dcfbc46d3c786` (0.6.8)
**Scope:** the ten bridge capabilities in
`docs/ui-parity/opencode-ui-parity-kit/audit-implementation.md:85-96`, plus the
remaining exported component APIs of the native bridge crate.

This is a **source inventory**, produced by reading the bridge crate source
directly from disk — the crate is readable in this workspace, so the historical
connector routing limitation that motivated PAR-24 does not apply (see §5). It is
**not** captured-golden or installed-terminal acceptance: PAR-23 keeps Rust parity
statuses `unverified` until executed against the real integrated build.

## 1. Pin verification (exact-pin evidence)

The pin is compiled into the crate, not merely asserted in this document:

| Fact | Exact source evidence |
| --- | --- |
| OpenCode revision = `450e7b92…` | `crates/opentui-bridge/src/native_identity.rs:38` `OPENCODE_COMMIT` |
| OpenTUI revision = `8d6065a3…` | `native_identity.rs:36` `OPENTUI_COMMIT` |
| OpenTUI version `0.6.8` | `native_identity.rs:34` `OPENTUI_VERSION` |
| Checked C ABI version 1 | `native_identity.rs:40` `ABI_VERSION` (`context-abi.zig`, experimental ABI 1) |
| Native artifact pins (3 triples, byte + FNV-1a fingerprint) | `native_identity.rs:44,59-78` `ArtifactPin` / `ARTIFACTS`; full SHA-256 custody in `crates/opentui-bridge/native/artifacts.json:52-141` |
| Required native exports checked at boot | `native_identity.rs:85` declares `REQUIRED_SYMBOLS: [&str; 54]` with 54 string entries at `:86-139`. The artifact producer separately records `required_symbol_names_checked: 53` at `native/artifacts.json:160`: **54 declared / 53 checked by the artifact producer** — an acknowledged producer-side divergence to be resolved, not a claim that 54 symbols were verified. The producer's scope is `native/artifacts.json:159-162` "Symbol-name presence only; not full ABI semantic verification." |
| Boot-time identity probe | `native_identity.rs:272` `identify`, `:328` `probe_symbols`, `:355` `probe_installed` (which calls `identify` at `:362`); **production consumer** `crates/cli/src/tui_entry.rs:10082` `verify_native_identity(native_identity::probe_installed(native_identity::HOST_TRIPLE))` inside `native_loop` (defined `:10060`, invoked `:20298` and `:20341`), gated by `#[cfg(feature = "native")]` at `:10079`. The `#[test]` at `tui_entry.rs:9923-9924` `verified_artifact_passes_boot_with_identity` and the probe test at `native_identity.rs:487/493` are test-only callers and do **not** count as consumers. |

The vendored artifacts are wired per target by
`crates/opentui-bridge/build.rs:11-65` (only when the `native` feature is on;
`Cargo.toml:12-14`).

### Verdict vocabulary

`PRESENT` = the Rust API exists in this checkout. `CONSUMED` = a **production**
CLI call site was located; test-only callers, `#[cfg(test)]` items and `mod tests`
sub-trees do not count. `PARTIAL` = a concrete subset exists but the row's full
proof requirement is not represented. `ABSENT` = no Rust API/consumer was found in
the inspected files; it makes no claim about the native library's private
implementation.

**Consumer-evidence correction.** An earlier draft of this inventory miscited
the `#[test]` at `tui_entry.rs:9923-9924` (`verified_artifact_passes_boot_with_identity`)
as the boot-identity consumer. That is a test-only caller and is disqualified by
the `CONSUMED` definition above. The production consumer is the `native_loop`
call at `tui_entry.rs:10082` (gated by `#[cfg(feature = "native")]` at `:10079`);
all references in this document now cite that call site. That call exercises
**pin identity only** (`native_identity::probe_installed`), so it promotes the
identity half of the ABI row to `CONSUMED`, not the `BridgeHandle`/`Arena`/
`SharedResource` handle-ownership half, which remains `PRESENT; consumer not
established` (§2 row 1(b), matching FFI-03 at `PARITY_STATUS.md:849`). The same
scope rule is applied mechanically to every row in §2 and §3 (see §7 for the
reproduction method): a symbol reference found only inside a `#[cfg(test)]` item
is recorded as test-only and never promoted to `CONSUMED`.

The Rust crate is `opencode_rk_opentui_bridge` at directory
`crates/opentui-bridge`. Its full export surface is declared at
`crates/opentui-bridge/src/lib.rs:1-31`: 21 `pub mod`s and the re-exports
`CellBuffer`, `Editor`, `EditorView`, `Rgba`, `BridgeHandle`, `Gallery`,
`ImageData`, `ImageRenderable`, `ImageState`, `MarkdownDoc`, `RendererSpec`,
`Renderer`, `BridgeError`, `ScrollBox`.

## 2. Capability table (kit audit-implementation.md:85-96)

| Capability / actual Rust API (exact evidence) | Live consumer / status | Tests / boundary |
| --- | --- | --- |
| **Native ABI and handle ownership — (a) pin identity.** `native_identity::{identify,probe_symbols,probe_installed}` (`native_identity.rs:272,328,355`; `probe_installed` calls `identify` at `:362`); pins `OPENCODE_COMMIT`/`OPENTUI_COMMIT`/`OPENTUI_VERSION`/`ABI_VERSION` (`:34-40`), `ArtifactPin`/`ARTIFACTS` (`:44,59-78`), `REQUIRED_SYMBOLS: [&str; 54]` (`:85`); ABI constants and `repr(C)` mirrors (`DEST_*`/`REMOTE_*`, `RendererSpec`, `ExternalBuildOptions`, `CursorStyleOptionsAbi`) in `renderer.rs:13-62` and the `unsafe extern "C"` native declarations in `renderer.rs:70+` | **PRESENT + CONSUMED.** Production boot identity check `tui_entry.rs:10082` (`verify_native_identity(native_identity::probe_installed(native_identity::HOST_TRIPLE))`) inside `native_loop` (`:10060`, invoked `:20298`/`:20341`), gated by `#[cfg(feature = "native")]` at `:10079`. The `#[test]` `verified_artifact_passes_boot_with_identity` (`tui_entry.rs:9923-9924`) and the probe test (`native_identity.rs:487/493`) are test-only callers, not the consumer. | `renderer::tests::renderer_spec_size_stable` (`renderer.rs:145`), `build_options_layout_matches_extern_struct` (`:161`); CLI test `verified_artifact_passes_boot_with_identity` (`tui_entry.rs:9924`) |
| **Native ABI and handle ownership — (b) handle ownership.** `BridgeHandle::{claim,live,is_live,close}` (`handle.rs:32,47,61,70,84`); generation handles `GenHandle` + `Arena<T>` with `is_owner_thread` (`handle.rs:126,226,302`, `insert/get/get_mut/borrow/swap/remove` at `:268,307,317,336,353,384`), `SharedResource` (`:442`), `Borrowed` (`:516`), `NativeHandle`/status constants (`:5-20`) | **PRESENT; consumer not established.** No production CLI use of the bridge handle/arena/`SharedResource` types was found: `rg 'BridgeHandle\|GenHandle\|Arena<\|SharedResource\|NativeHandle\|is_owner_thread\|handle::' crates/cli/src` returns only the **unrelated** CLI-local `SyntaxStyleArena` (`native_theme_runtime.rs:2471`, used `tui_entry.rs:10779`), which is not `opentui_bridge::handle`. This agrees with `PARITY_STATUS.md:849` FFI-03 `REAL_GAP` "`BridgeHandle` unused by cli". The boot-identity consumer in row (a) is pin **identity**, not handle ownership: Rust's safe handle boundary exists but is not exercised by the live page. | `handle` module tests (generation/owner-thread/`SharedResource` bounds) |
| **Retained scene and layout.** `Scene::{new,insert,remove,set_style,set_text,set_clip_children,set_colors,layout,measure,set_focus,focus_next,focus_prev,hit_test,paint}` (`scene.rs:331,342,397,442,456,461,471,491,577,672,735,776,781,807,845`); `Style` width/height/padding/border/gap/direction/offset (`scene.rs:125`, `content_rect` `:203`); `Dimension` percent/fixed (`:46`); `Direction` (`:116`); row/col split `split_row/split_col` (`layout.rs:276,289`), `Constraint` (`:179`), `ShellRegions` (`:84`) | No production CLI call to `Scene` was found (grep for `Scene::` / `scene::` in `crates/cli/src` returns no non-test consumer). Live CLI layout uses `native_layout.rs:254` `SessionRegions` / `:320` `compute`, painted via `NativeRenderer`. **PRESENT; consumer not established for Scene** — the component geometry API exists, but its presence is not evidence that the live page is scene-backed. | `scene` module tests (layout/focus/hit/paint); `native_layout::tests::lay_01_regions_tile_standard_viewports` |
| **Text and editor measurement.** Pure `display_width`, `wrap_cells`, `clip_lines`, `char_cells` (`text.rs:224,234,65,105`); `Editor` edit/selection/cursor/offset APIs (`editor.rs:97-535`: `set_text` `:134`, `selection` `:213`, `undo/redo` `:258/273`, `position_to_offset` `:359`, `offset_to_position` `:368`, `insert_text` `:421`); `EditorView::{rows,total_rows,visual_cursor,visible_rows}` (`editor.rs:619,703,749,755,798`) | No CLI use of bridge `Editor`/`EditorView` (grep `opentui_bridge::editor` / `EditorView` in `crates/cli/src` finds no non-test consumer); the CLI owns `native_prompt_layout.rs`/`native_prompt_pane.rs` and paints with `NativeRenderer`. **PRESENT; bridge consumer not established** (wide-cell helpers are available, but no call site proves the live editor uses them). | `editor::tests::ffi05_editor_view_wraps_wide_chars` (`editor.rs:967`); `text` width/wrap/clip boundary tests |
| **Styling.** `Rgba::{from_hex,from_f32,from_i32}` (`color.rs:137,183,200,211`), `RgbaU16` + `rgb`/`TRANSPARENT`/`pack_meta`/`intent_of`/`slot_of` (`:10,19,24,28,46,52`), theme slots `SLOT_*` (`:229-241`); `Cell`/`CellGrid::{set,fill,blit,lines}` (`buffer.rs:71,110,194,209,240,260`) with `BorderStyle` (`:369`); styled native drawing `Renderer::{draw_text_styled,fill_rect,set_cursor_style}` (`safe_renderer.rs:906,957,1033`) | **PRESENT + CONSUMED** for the `Rgba` type and `Renderer` drawing calls: `tui_entry.rs:98` imports `Rgba` (gated by `#[cfg(feature = "native")]` at `:97`), and production paint functions carry `Rgba` foreground/background values into the renderer — `draw_horizontal_rule` (`:6090`) calls `renderer.draw_text_styled(..., fg, Some(bg), 0)` at `:6104`; `paint_rail_row` (`:6190`) calls `renderer.fill_rect(start, y, width, 1, background)` at `:6212` and `renderer.draw_text_styled(start, y, &text, foreground, Some(background), 0)` at `:6213`; the live paint path further calls `draw_text_styled`/`fill_rect` (`tui_entry.rs:6386,6818,7084-7101`, …). `Rgba::rgb` **literal construction** appears only inside `#[cfg(test)]` modules in `tui_entry.rs` (e.g. `:3023-3024` in a test asserting at `:3028`; `:4092-4094` inside `#[cfg(all(test, feature = "native"))] mod chat_surface_inset_tests` at `:4086`), so it is not counted as consumption — production colours come from `native_theme_runtime` (`paint_color(ctx.palette, …)`, `tui_entry.rs:7082`). `Cell`/`CellGrid` are bridge-internal buffers reached through `Renderer`, not directly constructed by the CLI (no `CellGrid`/`CellBuffer` identifier in `crates/cli/src`). Semantic role selection is CLI-owned (`native_theme_runtime.rs`), not a bridge theme API. | `safe_renderer::tests::styled_text_rejects_controls_and_out_of_bounds`; buffer/color tests; CLI `native_capture::tests::par04_style_only_differences_do_not_hide_behind_text_equality` (`native_capture.rs:543`) |
| **Events.** `TerminalInput::{feed,next_event,flush_escape,pending}` and `TerminalEvent`, `KeyEvent`, `MouseEvent`, `CapabilityResponse` (`terminal_input.rs:299,327,366,383,322,261,186,230,241`); `input::{Key,MouseButton,InputEvent,FocusRing}` (`input.rs:9,42,50,67`), `events::EventBus::{push,drain}` (`events.rs:29,68,86`) | **PRESENT; consumer not established** for `TerminalInput`/`TerminalEvent`. The only reference in the CLI is a **test-only** differential caller: `#[cfg(test)] mod ffi11_shared_subset_equivalence_tests` (`native_input_decoder.rs:1470-1471`) imports `terminal_input::{…, TerminalInput}` at `:1482` and feeds identical byte frames to both adapters (`:1490-1492`); this pins framing equivalence, it does not wire the bridge parser into the live input loop. A repo-wide search for `terminal_input::` returns only `native_input_decoder.rs:1473,1482,1486` (all inside that test module). The **live** input path is the CLI's own `TerminalInputDecoder` (`native_input_decoder.rs:137`, constructed at `tui_entry.rs:10464` inside `native_loop`). So the bridge `TerminalInput` parser has **no live consumer**, while the live *behavior* requirement (paste atomicity, kitty/CSI-u keys, SGR mouse, focus, IME) is **already met by the CLI decoder**. Accordingly `FFI-10` was re-verified `ALREADY_CORRECT` at `PARITY_STATUS.md:605`; the `REAL_GAP` row at `PARITY_STATUS.md:813` is the **stale wave-44** entry that the `:605` re-verification supersedes, and this inventory does not re-open it. (Those two line numbers were re-resolved against `PARITY_STATUS.md`'s current bytes by this task; the first draft's `:585`/`:798` were stale offsets — `:585` is blank and `:798` is the FFI-09 row.) `FocusRing`/`EventBus` have no production CLI consumer found. | **Equivalence pins, not consumption:** `native_input_decoder::tests::ffi11_shared_subset_equivalence_tests` (`native_input_decoder.rs:1470-1492`); `terminal_input` parser tests; CLI SGR/paste/Kitty tests |
| **Transcript scrolling.** `ScrollBox::{new,set_viewport,set_content_size,scroll_to_y,scroll_by,wheel,page,home,end,content_row_at,vertical_thumb,horizontal_thumb,autoscroll}` (`scroll.rs:76,95,149,161,181,193,205,253,259,263,287,303,315,328`), acceleration/thumb constants (`:28-39`); CLI `TranscriptPage` (`native_transcript.rs`) | **Production** helper `native_view.rs:8251` `pub(crate) fn scrollbar_thumb` constructs a bridge `ScrollBox` (`:8260`) and reads `vertical_thumb` (`:8263`) so the painted bar matches bridge semantics; it is called from the live transcript paint at `native_view.rs:5104` and `:5464`. Transcript anchoring lives in `native_transcript.rs`. **PRESENT + CONSUMED.** Bridge supplies scroll geometry; transcript policy/anchors remain CLI-owned. | `native_transcript::tests::anchored_page_is_pinned_by_line_identity` (`native_transcript.rs:859`), `page_pins_to_bottom_and_scrolls`; scroll tests |
| **Markdown/highlighting.** `MarkdownDoc::{new,set_source,blocks,unstable_count,display_text}` (`markdown.rs:151,160,198,170,187,247`), `UNSTABLE_TAIL=2` (`:27`); `HighlightTargets::{create,set_source,request,complete,cancel,frame_text,styled_runs,snapshot,has_pending}` (`highlight.rs:683,696,720,745,782,817,830,846,910,916`), `CaptureClass` + `CAPTURE_CLASSES=14` (`:49,67`), `StaleReason`/`Completion` (`:359,376`). Boundary: **no native Markdown entry point** (`markdown.rs:15-20`) and **no exported highlight request/completion or grammar/query** (`highlight.rs:18-33`) — these are Rust models, not native FFI. | **PRESENT + CONSUMED** as a block-identity producer. Production chain: `native_view_chat.rs:3157` `MarkdownFlow::new(flow_options(markdown_mode))` inside `push_block_markdown` (`:3143`), reached from the live chat paint at `native_view_chat.rs:3386`; `MarkdownFlow::new` constructs `BridgeMarkdownDoc::new()` at `native_markdown_flow.rs:638` and `MarkdownFlow::rebuild` (`:1031`) drives `doc.set_source(...)` (`:1034`) for streaming-stable block ids. The bridge imports are at `native_markdown_flow.rs:71-73`. Native Markdown FFI and Tree-sitter grammar parity are explicitly not claimed. | `ffi06_bridge_doc_is_block_identity_producer` (`native_markdown_flow.rs:2738`) — **test**; `md_001`–`md_007`; highlight stale-result tests |
| **Animation / frame scheduling.** `FrameController::{new,request,push_hook,take_hook,prepare,paint,admit_output,drain_output,present,fail_output,request_shutdown,complete_shutdown}` (`frame.rs:232,312,335,354,365,376,388,410,419,443,455,466`), `FrameTicket` (`:40`), `FrameReason`/`FramePhase` (`:53,69`), `HostHook` (`take_hook` return), output caps (`:32-36`) | No production CLI call to `FrameController` found (grep `FrameController` in `crates/cli/src` has no non-test consumer); the CLI has its own tick/frame handling in `native_host.rs`/`tui_entry.rs`. **PRESENT; consumer not established** — the strict phase lifecycle is unit-tested but does not establish live animation wiring. | `frame::tests::happy_path_is_a_strict_phase_sequence` (`frame.rs:501`), output backpressure + shutdown tests |
| **Capture and comparison.** CLI-side portable frame model `native_capture::{Frame,FrameCell,Cursor,HitTarget,Capture,compare}` (`crates/cli/src/native_capture.rs:114-376`); bridge `CellGrid::lines` (`buffer.rs:260`) and `Renderer::snapshot_text` (`safe_renderer.rs:1089`) are **text-only** | **PRESENT; test/harness-only consumer.** `native_capture` is declared at `main.rs:69` (gated by `#[cfg(feature = "native")]` at `:68`) and its `Frame::{new,paint,canonical}` are exercised only from `native_capture.rs`'s own `#[cfg(test)]` module (`mod tests` at `:480`, tests through `:688`); no production render loop calls it. No bridge API exports a portable *styled-cell* capture; text-only snapshots cannot detect a wrong sidebar gray or invisible cursor. | `par04_style_only_differences_do_not_hide_behind_text_equality` (`native_capture.rs:543`), cursor/hit/clipping tests |
| **Terminal lifecycle.** `Renderer::{create,create_memory,close,setup_terminal,restore_terminal_modes,suspend,resume,enable_mouse,disable_mouse,enable_kitty_keyboard,disable_kitty_keyboard,clear_terminal,resize,frame,set_cursor,set_cursor_style,set_title,input_ready,read_input,terminal_size}` (`safe_renderer.rs:497,502,577,589,609,651,685,706,723,741,758,778,792,813,994,1033,1070,341,384,431`); `BridgeError::{classify}` (`:232,265`) | `tui_entry.rs:98` imports `Renderer as NativeRenderer` (gated by `#[cfg(feature = "native")]` at `:97`); the `native_loop` render path (`tui_entry.rs:9864-10216` and paint helpers) drives renderer lifecycle, mouse, Kitty keyboard, resize, cursor, title and memory rendering. Terminal-host ownership in CLI. **PRESENT + CONSUMED.** Clipboard/OSC links and image protocol are **not** exposed by this Renderer API; no such bridge consumer was found. | `safe_renderer::tests::render_once_content_present`, lifecycle/error tests; `native_terminal_lifecycle.rs` (integration test) |

## 3. Additional concrete component APIs (bridge modules)

These are exported/inspectable and relevant to the kit even where they do not
complete one of the ten rows.

* **Diff model/layout** — `FilePatch::{parse,old_path,new_path,rows,hunks,gutter,split_pairs}` (`diff.rs:188,293,410,415,420,425,449,483`), `Hunk` (`:123`), `Gutter` (`:545`), `DiffLayout::{new,resize,visible,row_text,row_line}` (`:973,1018,1144,1175,1182,1188`), `visible_chunks` (`:1196`), `split_added_patch` (`:799`), `SPLIT_MIN_WIDTH=100` (`:38`). **CONSUMED (parse only):** the production consumer is `native_diff_view.rs:472` (`FilePatch::parse(&patch[hunk_starts[0]..])`) inside `fn bridge_classified` (`:419`), reached via `classify_patch_for_paint` (`:396`) → `classification_for_paint` (`:411`) → the live paint fns `paint_rows_with_gutter` (`:119`), `paint_rows_with_gutter_wrapped` (`:155`) and `paint_rows_split` (`:928`), which are called by the live diff surfaces (`native_diff_inline.rs:91-102`, `native_view_chat.rs:3660/3702`, `native_view_dialogs.rs:3837`). The bridge `DiffLayout` has **no** CLI consumer: the `DiffLayout` at `native_view_dialogs.rs:6674` is a CLI-local struct of the same name, not `opentui_bridge::diff::DiffLayout`. The `FilePatch::parse` call at `native_diff_view.rs:2965` is a **test-only** producer probe inside `#[cfg(test)] mod ffi08_tests` (`:2831-2832`) and is not consumption evidence.
* **Images** — `ImageData::{new,solid,from_pixels,sample}` (`image.rs:68,76,97,106,143`), `ImageRenderable::{set_state,cells,paint}` (`:234,257,274,294`), `Gallery::{new,move_by,click,title}` (`:325,334,375,402,412`), half-block cell sampling (`HALF_BLOCK` `:41`). Bridge consumer: `native_image_preview.rs:38` imports `ImageData, ImageError, ImageRenderable, ImageState`; `decode_image` (`:41`) decodes PNG/JPEG into the bounded RGBA model and the module is used by `native_view_pages::Page::ImagePreview` (`native_view_pages.rs:804`). Rasterizing a real image still depends on the external terminal image protocol/pass-through for display (see `PARITY_STATUS.md` MEDIA-001).
* **Shapes** — `Bar`, `Sparkline`, `Menu`, `Card` (`shapes.rs:28,68,112,149`) with bounded `MAX_*` constants (`:5-17`); pure render helpers, no live consumer located.
* **World/camera** — `Camera::{set_zoom,pan}` (`world.rs:45,65,74`), `Viewport` (`:92`), `world_to_screen`/`screen_to_world` (`:106,115`), `ShellRegions::{paint_calls,has_overlap}` (`:169,221,234`); pure geometry, no live consumer located.
* **Events** — `EventBus::{push,drain}` (`events.rs:68,86`), bounded `MAX_EVENTS=256` (`:9`); no live consumer located.
* **Retained resources** — `Arena<T>` + `SharedResource<T>` + `Borrowed` (`handle.rs:226,442,516`) provide bounded generation-checked ownership; they are not proof that every component is mounted in the live page.

## 4. Upstream mapping

**Not independently re-inspected by this task.** The upstream checkout
`/Users/mymac/Projects/tmp/opencode` at pin
`450e7b92412c780a7b5a75226bb8d49aaca24e22` is **not readable from this
workspace** — external-directory reads under `/Users/mymac/Projects/tmp` are
denied here, so this document does not verify that checkout's SHA or that the
upstream paths below exist at that pin. What *was* read directly in this
workspace is the bridge crate's own doc comments, reproduced here verbatim:

- `crates/opentui-bridge/src/markdown.rs:6-24` cites OpenTUI
  `renderables/Markdown.ts:1-165` and `markdown-parser.ts:1-79` (pin
  `8d6065a3`) and OpenCode `packages/tui/src/plugin/markdown.ts:6-21` (pin
  `450e7b9`).
- `crates/opentui-bridge/src/highlight.rs:6-33` cites OpenTUI
  `packages/core/src/renderables/Code.ts:47-170` (pin
  `8d6065a3e60039d16c6ee697692dcfbc46d3c786`) and OpenCode
  `packages/tui/src/component/patch-diff.tsx:127-156` (pin `450e7b9`).

The full upstream boundary analysis remains in
`docs/ui-parity/opencode-ui-parity-kit/audit-opentui-boundary.md` §3 (also not
re-inspected here). The mapping intentionally separates *component models* from
*live consumers*: a bridge module's existence is never reported as a wired
parity capability without a call-site trace.

## 5. PAR-24 disposition

PAR-24 requires a completed bridge-source inspection **after** correcting the
"duplicate Secure Tunnel ID routing problem". That routing problem was a
historical tooling limitation of the original audit session, recorded in
`audit-implementation.md:9`, `baseline.lock.json:45`, `README.md:22` and
`opencode-ui-parity-guide.html:9`: two connectors shared one Secure Tunnel ID, so
bridge reads hit inconsistent tool catalogs and the original report deliberately
claimed no bridge inventory.

Per the task grant, that duplicate-ID fix is **treated as already handled and is
out of scope here — no re-fix was attempted**. PAR-24's remaining half, the
bridge-source inspection itself, is satisfied by this document: it was produced by
reading `crates/opentui-bridge/src/*` (and `native/artifacts.json`, `build.rs`,
`Cargo.toml`) directly from the on-disk checkout, which does not pass through that
connector. No behavior/product code or acceptance test was changed.

**Evidence provenance.** The task cites
`evidence/session-20261121/GAPS_ACTIONABLE.md` and
`evidence/session-20261121/verify/batch-02-PAR-13-24.md` as the PAR-24 gap record,
but neither path exists in this workspace (a workspace-wide search under
`evidence/session-20261121/` returns nothing, i.e. the directory is absent), so
the gap claim could not be cross-checked against those specific files. The
corroborating kit files that *do* exist —
`audit-implementation.md:9`, `baseline.lock.json:45`, `README.md:22`,
`opencode-ui-parity-guide.html:9` — together with the direct on-disk bridge source
read above are the basis of this inventory. The missing evidence paths are stated
rather than papered over; no content was inferred from them. Any earlier commit
identifier referenced only in prose (e.g. `019ecafa`) is likewise **not verified**
by this documentation task (no Git history inspection was performed here) and is
not treated as evidence.

## 6. Result and limitations

PAR-02 is **complete as an inventory artifact**: every capability row from the kit
table is mapped to an actual Rust symbol with exact file/line evidence, to a live
production consumer where one was located, or to an explicit "consumer not
established" result.

Limitations, stated honestly:

- This is **source inspection only**. No styled-cell/cursor captures, PTY runs, or
  installed-terminal goldens were produced by this task, so no status here is
  runtime-proven (PAR-23 remains `unverified`).
- The vendored native artifact's `artifacts.json:164-170` records
  `runtime_load_test:false`, `Rust_link_test:false`, `ABI_semantic_verification:false`,
  `PTY_gate:false`; identity checks are symbol-name + byte-fingerprint, not full ABI
  semantics. The producer reports `required_symbol_names_checked: 53`
  (`artifacts.json:160`) against `REQUIRED_SYMBOLS: [&str; 54]`
  (`native_identity.rs:85`) — 54 declared / 53 checked, an open divergence.
- "Consumer not established" is a statement about the files inspected, not proof
  of absence elsewhere; it is deliberately not upgraded to `CONSUMED` without a
  production call-site trace outside `#[cfg(test)]`.
- The duplicate Secure Tunnel ID fix is outside this grant and was not re-applied
  or re-verified (see §5).
- No product behavior changed, so no new runtime test was added: this task's
  deliverable is documentation. All existing semantic tests are unmodified. Note
  that the controller's trusted check `native_view_applies_authenticated` runs a
  pre-existing Rust test (`native_view_applies_authenticated_session_mcp_projection_to_sidebar`,
  `crates/cli/src/native_view.rs:9058`); this documentation task does not itself
  produce or claim that check's output.

## 7. Method and reproduction

Every capability row was classified with a two-pass, scope-aware rule rather than a
per-row symbol grep:

1. **Enumeration.** For each bridge export named in a row, content-search the CLI
   crate (`crates/cli/src`) and the bridge crate (`crates/opentui-bridge/src`), and
   capture each hit plus its enclosing item, file line and gate.
2. **Admission.** A hit counts toward `CONSUMED` only if it is lexically **outside**
   any `#[cfg(test)]` item or `mod tests` subtree **and** its enclosing non-test
   `pub fn` reaches a run entry point. Test-only callers are recorded as
   "equivalence pins" and never promoted to `CONSUMED`.

Exact searches used (run from the repository root):

```text
rg -n 'terminal_input::' crates/cli/src
#   -> only native_input_decoder.rs:1473,1482,1486 (inside
#      #[cfg(test)] mod ffi11_shared_subset_equivalence_tests, :1470)

rg -n 'classify_patch_for_paint|bridge_classified|classification_for_paint' crates/cli/src
#   -> production chain native_diff_view.rs:396/411/419; FilePatch::parse at :472;
#      :2965 is inside #[cfg(test)] mod ffi08_tests (:2831)

rg -n 'MarkdownFlow::new|BridgeMarkdownDoc|push_block_markdown' crates/cli/src
#   -> production native_view_chat.rs:3157 inside push_block_markdown (:3143),
#      live caller :3386; producer native_markdown_flow.rs:638/:1031;
#      :2738 is a #[test]

rg -n 'FrameController|opentui_bridge::editor|EditorView|Scene::|EventBus|FocusRing' crates/cli/src
#   -> no production consumer for these bridge models

rg -n 'scrollbar_thumb' crates/cli/src
#   -> bridge ScrollBox consumer native_view.rs:8251, called live at :5104/:5464

rg -n 'draw_text_styled|fill_rect' crates/cli/src
#   -> live renderer styling calls (tui_entry.rs:6104,6212,6386,6818,7084-7101, ...)

rg -n 'native_capture' crates/cli/src
#   -> only main.rs:69 `mod native_capture;` plus native_capture.rs's own tests

rg -n 'DiffLayout|opentui_bridge::diff' crates/cli/src
#   -> bridge FilePatch consumer native_diff_view.rs:420/:472; bridge DiffLayout
#      has no CLI import; native_view_dialogs.rs:6674 is a CLI-local DiffLayout
```

Per-row admission checklist:

- [x] Every `CONSUMED` line asserted to be outside `#[cfg(test)]` and reach a run
      entry (pin identity → `native_loop` `tui_entry.rs:10082`; styling →
      `tui_entry.rs:6104+`; scrolling → `native_view.rs:5104/5464`; markdown →
      `native_view_chat.rs:3386`; diff → `native_diff_inline.rs:91-102`;
      terminal lifecycle → `native_loop` render path). **Handle ownership is
      *not* in this list:** the same `tui_entry.rs:10082` call is pin identity,
      not `BridgeHandle`/`Arena`/`SharedResource` use, so handle ownership is
      recorded `PRESENT; consumer not established` (matches FFI-03 `REAL_GAP` at
      `PARITY_STATUS.md:849`).
- [x] Every `#[test]`-only caller demoted to "test/equivalence" (Events
      `native_input_decoder.rs:1482`; Diff `native_diff_view.rs:2965`; Markdown
      `native_markdown_flow.rs:2738`; Capture `native_capture.rs:480-688`; FFI-01
      `tui_entry.rs:9923-9924`).
- [x] `PRESENT; consumer not established` used wherever only a test or no caller was
      found (Scene, editor, frame scheduling, events, `FocusRing`/`EventBus`,
      bridge handle ownership).
- [x] Pin row distinguishes 54 declared (`native_identity.rs:85`) from 53 checked
      (`artifacts.json:160`).

Method note: every claim above is source inspection of the on-disk bridge crate;
no styled-cell capture, PTY run or installed-terminal golden was executed by this
documentation task (see §6).

Re-verification note (2026-11-22): base provenance is the workspace lease record
`.delivery-owner.json:1` (`"base": "6aee80ce…"`, matching the dispatch), pin
constants are `native_identity.rs:34-40`, and every `PARITY_STATUS.md`
cross-reference in this document was re-resolved against that file's current
bytes (`:849` FFI-03, `:605` FFI-10 `ALREADY_CORRECT`, `:813` stale FFI-10
`REAL_GAP`). Nothing in this document asserts a runtime or captured-golden result.