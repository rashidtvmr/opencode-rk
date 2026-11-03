# OpenCode V2 UI parity — consolidated status

- **Canonical line:** `main-v2` @ `98bf3ac2` (remote verified)
- **Upstream oracle:** `/Users/mymac/Projects/tmp/opencode` @ `450e7b92412c780a7b5a75226bb8d49aaca24e22`
- **Checklist:** `docs/ui-parity/opencode-ui-parity-kit/checklist.json` (223 items)
- **Method:** source inspection of the live Rust path (implementation + a
  production consumer), cross-checked against the pinned upstream component.
  This is **not** captured-golden acceptance: no matched styled-cell/cursor
  captures were produced. Verdicts below are source-backed, not runtime-proven.

## Meaning of the verdicts

| Verdict | Meaning |
| --- | --- |
| `ALREADY_CORRECT` | The live Rust path renders the upstream behavior; implementation and a focused test exist, and a production consumer was traced. No change needed. |
| `REAL_GAP` | Implementation + test exist but **no production consumer** wires them into the live TUI. Fixable by wiring, not by new backend data. |
| `BLOCKED` | The requirement needs a **producer/transport that does not exist** (contract field, server event, or config key). A UI-only patch would render nothing. |
| `UNVERIFIED` | Not yet finalized in this pass. |

## Summary

- The large majority of items are `ALREADY_CORRECT` on source inspection.
- The remaining work is **not** parallelizable UI work: it is a small set of
  live-wiring gaps and a smaller set of backend-producer gaps. No item is
  blocked on "more agents"; each blocked item names the exact missing seam.

## Wave-44 verification pass (2026-11-02)

Base SHA `0d74af30`. 28 parallel workers on the four authorized routes, each
artifact-first and read-only on canonical. Per-ID JSON verdicts live in
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/wave44/`.
This pass **finalizes** the following IDs and supersedes their earlier entries.

> **This wave-44 table is HISTORICAL. Do not read a verdict from it.**
> Later waves (45–54) superseded many rows. **The authoritative verdicts are the
> summary counts at the end of this document and the `REAL_GAP` / `BLOCKED` /
> `LANDED` sections below.** Specifically, wave-44 rows that still read
> `REAL_GAP` are superseded as follows:
> - to `BLOCKED`: `PAR-04`, `PAR-09`, `PAR-10`, `PAR-19`, `SIDE-04`,
>   `MSG-001..003`, `MSG-005`, `MSG-008..010`, `DIALOG-12..20`,
>   `FFI-01/03/04/06/07/08/11`, `LAY-10/11/13`, `TAB-04/06/10`, `CP-07`,
>   `CP-09`, `MD-007`, `THEME-08/09/13/14`, `TOOL-006/011`, `DIFF-001/003`,
>   `STATE-002`, `FORM-001`;
> - to `ALREADY_CORRECT`: `PAR-11`, `LAY-12`, `LAY-17`, `MSG-004`, `FFI-05`,
>   `CP-08`;
> - to `LANDED`: `MSG-007` (`4a6b487b`), `CP-11` (`f8e276cf`), `DIALOG-04`
>   (`d137313d`), `DIALOG-20` (toast half, `d891f9c3`), `TAB-23` (`ae56897f`),
>   `SIDE-08` (`76fcb75a`), `THEME-12` (`bc24dd30`), `PAR-16`/`PAR-22`
>   (`f240367b`/`99e0e6ae`).

**Reclassified / newly finalized:**

| Item | Wave-44 verdict | Evidence / seam |
| --- | --- | --- |
| PAR-13 | `ALREADY_CORRECT` | `native_message_rows.rs:2115` `click_acts`; live `tui_entry.rs:10102,10162`; test `native_text_selection.rs:532` |
| PAR-14 | `ALREADY_CORRECT` | `native_tui_parity.rs:186` COLUMNS=40; `upstream_view_parity.rs:76-78` 60/88/116; `:145` >120 boundary |
| PAR-15 | `ALREADY_CORRECT` | `native_theme_runtime.rs:392-434` hex/ref/{dark,light}/transparent resolve; live `tui_entry.rs:6736-6753,11425` |
| PAR-16 | `LANDED` (`f240367b`) | Fixture classes (`baseline`/`optional`/`experimental`/`extension`): every fixture resolves to exactly one class, unknown classes are rejected, `--class` filters, the class travels into the request/golden, and a golden/fixture class mismatch is a hard `fixture_class` diff (`ui_parity.py`); 5 focused selftest checks |
| PAR-17 | `ALREADY_CORRECT` | `baseline.lock.json:12-24` v2.2.0 `is_authoritative:false`; pins `native_identity.rs:36-38` |
| PAR-18 | `ALREADY_CORRECT` | `safe_renderer.rs:1148` Drop, `:609` restore; live `tui_entry.rs:12756-12759`; tests `native_terminal_lifecycle.rs:192-217`, `highlight.rs:1133` |
| PAR-19 | `REAL_GAP` | Model `native_diff_view.rs:1043,1058` + test `:1963`; no live consumer (`native_view_chat.rs:2707` hardcodes `paint_rows_with_gutter`) |
| PAR-20 | `ALREADY_CORRECT` | `selftest.py:161-207` mutations one_cell_color/cursor/width/focus/action_extra all assert exit 1 |
| PAR-21 | `ALREADY_CORRECT` | `support/upstream_view_parity.rs:4` frozen `upstream_layout_parity.rs` untouched; sibling file |
| PAR-22 | `LANDED` (`f240367b`) | `--evidence-bundle` writes a SHA-bound bundle (`rust_sha`, spec/golden/fixture hashes, class counts, exact reproduction commands, per-fixture outcomes, installed-capture hashes) so a parity claim binds to the exact Rust revision under test; 5 focused selftest checks |
| PAR-23 | `ALREADY_CORRECT` | Every checklist item `status:unverified_rust`; kit `rust_checks_passed:null` |
| PAR-24 | `ALREADY_CORRECT` | Bridge sources present/inspectable (`native_identity.rs:36-38`, scene/highlight/terminal_input) |
| THEME-02 | `ALREADY_CORRECT` | `prompt_pane.rs:119/:1288`; `theme_runtime.rs:938`; live `tui_entry.rs:8567` |
| THEME-03 | `ALREADY_CORRECT` | `theme_runtime.rs:800,916,762`; tests `:3179,:3227`; live `tui_entry.rs:8567` |
| THEME-04 | `ALREADY_CORRECT` | `theme_runtime.rs:762`; tests `:3537`+f3_*; live `tui_entry.rs:8567` |
| THEME-05 | `REAL_GAP` | `theme_runtime.rs:1189,1198` + test `:3574`; no live per-cell ActionStates map (`PaintCtx` `tui_entry.rs:8494` threads no state map) |
| THEME-06 | `ALREADY_CORRECT` | `theme_runtime.rs:1712,1244`; test `:3148`; live `tui_entry.rs:4491` dialog_panel in modal paint |
| THEME-07 | `ALREADY_CORRECT` | `theme_runtime.rs:120,142`; tests `:2654-2691`; live `tui_entry.rs:4118` paint_color |
| THEME-08 | `REAL_GAP` | `theme_runtime.rs:965` + tests `:3607-3757`; markdown half live `tui_entry.rs:4747`; `syntax_rules()` has zero callers, no highlight producer |
| THEME-09 | `REAL_GAP` | `tui_entry.rs:2849,2866`; test `theme_runtime.rs:3757`; unified live `tui_entry.rs:4626`; split unreachable (`native_view_chat.rs:2179` hardcodes Unified) |
| THEME-10 | `ALREADY_CORRECT` | `theme_runtime.rs:1592,1665,1754,1768`; tests `:4167-4241`; live `tui_entry.rs:6744,11425` |
| TAB-21 | `BLOCKED` | `ViewAcknowledgement` (`native_navigation.rs:1133`, test `:2067`) zero live callers; `tui_entry.rs:8689-8694` defers driver; no idle/viewed watermark into CLI (`LiveSnapshot` `tui_entry.rs:573`; `SessionSummary` `contracts/src/lib.rs:202-213`) |
| TAB-23 | `LANDED` | `ae56897f` — live `session.tab.*` dispatch now resolves through the config `keybinds` chord table (`native_navigation::tab_keybinds`); `UiPreferences.keybinds` (raw JSON) + focused tests. |
| SIDE-03 | `BLOCKED` | Renderer+test correct (`native_sidebar.rs:230-287`); live `set_usage` (`native_view_status.rs:1146`) drops cached/reasoning; `set_usage_classes` (`:1168`) + `set_cost` (`:1160`) zero non-test callers; no last-eligible selector; no revert boundary; no cost producer |
| SIDE-04 | `REAL_GAP` | Five-class arithmetic `native_sidebar.rs:205-287` (test `:1608`) dead on live path; no last-eligible-assistant selector; limit joined only from selected UI model (`tui_entry.rs:8112`); wire `input_tokens` is inclusive of cached (`server/src/auto_compaction.rs:102`) so disjoint classes need a `nonCachedInput` producer |
| SIDE-08 | `REAL_GAP` | No `DialogKind::Mcp` (`native_view_dialogs.rs:129`); row click toggles via PUT (`tui_entry.rs:10050`), never opens dialog. Upstream `mcp.tsx:56-60` |
| SIDE-14 | `BLOCKED` | `footer_menu` (`native_sidebar.rs:1020`) + duplicate `location_menu` (`native_status_rail.rs:1114`) test-only; no footer hit rect (`native_view.rs`), no hover routing (`tui_entry.rs:9407` kind==7), no click branch (`tui_entry.rs:10016`), no CopyPath/OpenFolder/Workspaces executors |
| MSG-005 | `BLOCKED` | Chip helpers `native_message_rows.rs:1904-2046` (tests `:2449-2514`) zero consumers; `MessageRecord` (`contracts/src/lib.rs:250-257`) has no files/skills; producer chain absent `sessions/src/lib.rs:623`, `storage/src/writer_v2.rs:129`, `server/src/lib.rs:1256`, `native_chat_projection.rs:641`, `native_view_chat.rs:1069/:2635` |
| MSG-008 | `REAL_GAP` | Widths/agent/model/duration/interrupted live (`native_footer.rs:377`, `native_assistant_facts.rs:391`); TPS dead end-to-end: producer `server/src/lib.rs:2894-2901` emits no streamed stamp; `MessageExecution` (`contracts/src/lib.rs:284-293`) has no timing field; `RowTiming.streamed_ms` (`native_message_rows.rs:825`) never set; `tps_enabled` test-only (`native_assistant_facts.rs:325`); `data.session.tps` absent |
| MSG-009 | `REAL_GAP` | `TranscriptSurfaces`/`RevertCard`/`TranscriptNotice`/`ChildCompletionNotice` (`native_view_chat.rs:4482/4103/4273/4409`) zero live consumers; `SessionRevert` (`contracts/src/lib.rs:191`) has no count; server stage `server/src/lib.rs:1199-1204` hardcodes `files:None`; no notice projection; no child-completion metadata |
| DIFF-001 | `BLOCKED` | Split arithmetic `native_diff_view.rs:600` (tests `:1826-1837`) unwired; no `config.diffs.view` parse; live render hardcodes unified (`native_view_chat.rs:2707`); flat `Vec<(String,ChatRowClass)>` row model cannot carry per-side classes; `DiffSurface::Viewer` (`:591`) zero callers |
| DIFF-003 | `BLOCKED` | Virtualization model `native_diff_view.rs:1014-1076` (tests `:1962-2209`) zero callers; `AddedPatchChunk` (`:926-930`) carries no highlight field |
| STATE-002 | `BLOCKED` | Producer seam `server/src/lib.rs:1668 compact_session` returns `{compacted,strategy}` and never persists/pushes a compaction part; `MessageRole` (`contracts/src/lib.rs:222`) has no Compaction variant; rich `CompactionState` (`native_timeline.rs:382`) zero consumers; live render placeholder `native_view_chat.rs:2689` |
| TOOL-011 | `BLOCKED` | `execute_dialog` (`native_tool_rows.rs:1159`, tests `:1951`) zero consumers; live render placeholder `native_view_dialogs.rs:1429`; only producer is test `:5847`; no `ExecuteDialog` payload on `OpenDialog`/`DialogView`; no syntax producer (`opentui-bridge/src/highlight.rs:27-29`) |
| FORM-001 | `BLOCKED` | Live dialog `native_view_dialogs.rs:1090` emits 6 labels vs upstream 3 (`permission.tsx:232-240`); save gate drops only "Always allow"; always-body swap missing (static `:1420`); nested rejection stage missing. Sub-behavior 1 (3-option realign + save gate) is independently bounded via `permission_alias:488`; sub-behaviors 3+4 need a stage+textarea in `DialogView` |
| DIALOG-20 | `LANDED` (toast half) | `d891f9c3` — the live loop now owns a `Toast` and paints its `ToastFrame` as the topmost overlay (`PaintCtx.toast`), advancing the logical clock on the animation tick and triggering on session-id copy (`app.tsx:605`). Crash/error-details half remains `BLOCKED` (no `panic::set_hook`, no `DialogKind::Crash`). |

**Additional wave-44 verdicts (second batch):**

| Item | Wave-44 verdict | Evidence / seam |
| --- | --- | --- |
| PAR-01 | `ALREADY_CORRECT` | Pins stamped into every golden (`ui_parity.py:464`), compared every `--check` (`:247-276`), negative-controlled (`selftest.py:237`) |
| PAR-02 | `BLOCKED` | Capability table empty (`audit-implementation.md:79-96`); no producer |
| PAR-03 | `BLOCKED` | Rust adapter exists (`oc2_pty.py`); upstream TS/OpenTUI adapter absent (`README.md:148`) |
| PAR-04 | `REAL_GAP` | text/pos/style/clipping/cursor covered+tested; hit targets never modeled (no `hit_targets` compare key) |
| PAR-05 | `ALREADY_CORRECT` | Wide-cell continuation + combining fold (`ui_parity.py:65-73,139-182`), tested (`selftest.py:244`) |
| PAR-06 | `BLOCKED` | Cell/text-only harness; no font/pixel capture path (`oc2_pty.py:362-369`) |
| PAR-07 | `BLOCKED` | Frame side complete (`native_capture.rs`); event-trace sink (`native_debug.rs:81`) misses queue/steer, permissions, forms, input-submission; seam `native_composer.rs` submit path |
| PAR-08 | `ALREADY_CORRECT` | `native_clock.rs` FrameClock; live `tui_entry.rs:6666` |
| PAR-09 | `REAL_GAP` | async syntax await + stale/blank rejection in `highlight.rs`; `pub mod highlight` has zero cli call sites |
| PAR-10 | `REAL_GAP` | `native_transcript.rs` accums; live path consumes only `strip_ansi` |
| PAR-11 | `REAL_GAP` | `native_message_rows.rs` anchors/identity/group-expansion; no `tui_entry` consumer |
| PAR-12 | `ALREADY_CORRECT` | modal keymap scope/priority/suppression live (`native_view_dialogs.rs:652-828`); dock/side-panel scope unasserted |
| TAB-01 | `ALREADY_CORRECT` | `native_tabs.rs:1440`; test `:3576`; live `tui_entry.rs:8245` |
| TAB-02 | `ALREADY_CORRECT` | `native_tabs.rs:34`; test `:3607`; live `tui_entry.rs:5032` |
| TAB-03 | `ALREADY_CORRECT` | `native_tabs.rs:1592`; test `:3640`; live `tui_entry.rs:4998` |
| TAB-04 | `REAL_GAP` | numbers-mode `TabIndicators` (`native_tabs.rs:1773`) has no live config source; `config.tabs.indicators` not loaded; render `tui_entry.rs:8245` hardcodes status |
| TAB-05 | `ALREADY_CORRECT` | `native_tabs.rs:754`; test `:3783`; live `tui_entry.rs:8215` |
| TAB-06 | `REAL_GAP` | `enqueue_user_prompt`/`pending_marks_busy` (`native_tabs.rs:1877/1893`) zero callers; `queued_drafts` push (`tui_entry.rs:11512`) carries no per-tab flash/busy feed |
| TAB-07 | `ALREADY_CORRECT` | `native_tabs.rs:1940`; test `:3852`; live `tui_entry.rs:4964` |
| TAB-08 | `ALREADY_CORRECT` | `native_tabs.rs:2027`; test `:4032`; live `tui_entry.rs:5009` |
| TAB-09 | `ALREADY_CORRECT` | `native_tabs.rs:2146`; test `:4052`; live `tui_entry.rs:7930` |
| TAB-10 | `REAL_GAP` | geometry live (`render:471` → `tui_entry.rs:8281`); `session_tab_detail` (`native_tab_rail.rs:103`) no live consumer; `rail_inputs_for_navigation` (`tui_entry.rs:5277`) feeds only `project_label` |
| TAB-11 | `ALREADY_CORRECT` | `native_tab_rail.rs:778`; test `:2363`; live `tui_entry.rs:8281` |
| TAB-12 | `ALREADY_CORRECT` | `native_tab_rail.rs:1138`; test `:2228`; live `tui_entry.rs:5072` |
| MD-001..006 | `ALREADY_CORRECT` | Live path `native_view_chat.rs:2320 push_block_markdown` → `native_markdown_flow.rs:770` → paint `tui_entry.rs:4753`; per-ID tests in `native_markdown_flow.rs` |
| MD-007 | `REAL_GAP` | `set_fence_handlers`/`MarkdownPluginRegistry` zero non-test callers; seam `native_view_chat.rs:2334` (flow built, no handlers fed) |
| MD-008 | `BLOCKED` | mermaid renderer built `without_painter` (`native_view_chat.rs:2276`); needs live `DiagramPainter` |
| MD-009 | `BLOCKED` | math renderer built `without_engine` (`native_view_chat.rs:2298`); needs live `MathLayoutEngine` |
| CP-07 | `BLOCKED` | The `AgentFact` model (`native_prompt_pane.rs:580-631`) is test-only: no live caller of `selectable_agents`/`agent_color`, `set_composer_agent` (`native_view.rs:1232`) has zero non-test callers so `composer_agent` is always `None` live, `agent.cycle` is inert (`tui_entry.rs:10807-10811`), `DialogKind` has no Agent variant, and the `mention_agents` contract (`configured_commands.rs:80-99`) emits only name/description/read_only — no `hidden`/`subagent`/`color`. Needs a cross-crate contract field + live selection state |
| CP-08 | `REAL_GAP` | `next_prompt_display` (`native_prompt_pane.rs:675`) not routed via `composer_metadata` (`native_view.rs:1612`) |
| CP-09 | `REAL_GAP` | `prompt_highlight`/`border_highlight`/`MetadataReveal` (`native_prompt_pane.rs:137,155,702`) not consumed by Paint path |
| CP-10 | `ALREADY_CORRECT` | `rail_spans`/`slot_text` (`native_status_rail.rs:874,1048`); live `native_view.rs:3867-3904` → paint `:3820-3822` |
| CP-11 | `REAL_GAP` | `RailFacts::status_slot` (`native_status_rail.rs:739`); live `footer_line` (`native_view.rs:3879`) never sets `running`/`move_progress`/`pending_new`/`location` |
| LAY-16 | `ALREADY_CORRECT` | `native_layout.rs:405-420`; live `native_shell_frame.rs:279` |
| LAY-17 | `REAL_GAP` | Anchor model `native_view_layout.rs:1014-1320` (tests `:1916-2196`) zero non-test callers; live transcript scroll/tab-restore never calls it |
| LAY-18 | `ALREADY_CORRECT` | `native_home.rs:200`; live `native_view.rs:1451` |
| LAY-19 | `ALREADY_CORRECT` | `native_home.rs:277`; live `native_view.rs:2042` |
| LAY-20 | `ALREADY_CORRECT` | `native_home.rs:88-146`; live `native_view.rs:4175/4200` |
| MSG-006 | `ALREADY_CORRECT` | `click_acts` (`native_message_rows.rs:2109-2117`, test `:2519`); live `tui_entry.rs:10102-10106` gates message-menu + child-nav |
| MSG-007 | `REAL_GAP` | `MessageActionMenu` (`native_view_chat.rs:3568-3760`) live dispatch `tui_entry.rs:3745-3829`; ForkSession → notice only (`:3820-3822`); `menu.resync` (`native_view_chat.rs:3642`) zero live callers |
| SIDE-11 | `ALREADY_CORRECT` | `native_sidebar.rs:426`; test `:1739`; live `native_view_status.rs:871` → `native_view.rs:3360` |
| SIDE-12 | `ALREADY_CORRECT` | `native_sidebar.rs:1063`; test `:1936`; live `native_view_status.rs:354` → `native_view.rs:271/:308` |
| SIDE-13 | `ALREADY_CORRECT` | `native_sidebar.rs:64,777,949`; test `:2081`; live `native_view.rs:3904` |
| SIDE-15 | `ALREADY_CORRECT` | `native_sidebar.rs:406-409`; test `:2170`; live `native_view.rs:3360` |

**Additional wave-44 verdicts (third batch):**

| Item | Wave-44 verdict | Evidence / seam |
| --- | --- | --- |
| FFI-07 | `REAL_GAP` | `highlight.rs:683` impl, `:1133` test; no cli consumer (cli renders through its own `native_*` models) |
| FFI-08 | `REAL_GAP` | `diff.rs:1140` impl, `:1434` test; live path is separate `native_diff_view.rs` |
| FFI-09 | `ALREADY_CORRECT` | `scroll.rs:328/:411`; consumer `native_view.rs:5580` (`scrollbar_thumb`) |
| FFI-10 | `ALREADY_CORRECT` | `terminal_input.rs:327/:1113`; consumer `safe_renderer.rs:69` |
| FFI-11 | `REAL_GAP` | `scene.rs:442` impl, `:1149` test; no cli consumer |
| MD-006 | `ALREADY_CORRECT` | `native_view_chat.rs:2432`, test `:5826`, consumer `:2574`; redacted-only renders nothing (`:2586`) |
| MD-007 | `REAL_GAP` | `MarkdownPluginRegistry`/`set_fence_handlers` (`native_markdown_flow.rs:468/681`, test `:2570`) zero non-test callers; live flow `native_view_chat.rs:2334` never binds registry |
| MD-008 | `REAL_GAP` | mermaid fan-out/fallback (`native_mermaid.rs:270/313/642`, tests `:1110-1393`); live `without_painter` (`native_view_chat.rs:2276`) forces `Fallback` (`:2280`); needs live `DiagramPainter` injection |
| MD-009 | `REAL_GAP` | math lifecycle (`native_math.rs:676`, `last_good` `:657`, test `:943`); live `without_engine` (`native_view_chat.rs:2298`) forces `Fallback` (`:2302`); needs live `MathLayoutEngine` injection. **Supersedes** the earlier `ALREADY_CORRECT` entry |
| MSG-010 | `REAL_GAP` | `turn_duration`/`turn_tokens_per_second`/`legacy_turns` (`native_message_rows.rs`) no non-test caller; footer `native_footer.rs:263` leaves throughput an explicit gap; projection `native_chat_projection.rs:713` uses `native_assistant_facts::turn_duration_ms` instead |

**Additional wave-44 verdicts (fourth batch):**

| Item | Wave-44 verdict | Evidence / seam |
| --- | --- | --- |
| LAY-01..09 | `ALREADY_CORRECT` | Live path `native_view_layout::{Layout,compose_frame}` + `NativeView::presentation/sidebar_visible`, driven by `tui_entry.rs:8334 view.render_typed`; per-ID consumers traced (LAY-01 `tui_entry.rs:8334`; LAY-02 `native_view.rs:4089/2128`; LAY-03/05 `tui_entry.rs:8148/7389`; LAY-04 `rail_resize.preferred()`; LAY-06/07 `native_view.rs:1855-1856`; LAY-08 `tui_entry.rs:8973/8988`; LAY-09 `tui_entry.rs:7624`) |
| LAY-10 | `REAL_GAP` | 107/108 fullscreen threshold + tabs-hide rule model-complete (`native_pane_resize.rs:551/558/586`, `native_host.rs:684/691`) but `NativeHost::new` is `#[cfg(test)]`-only (`native_host.rs:755,772`); `NativeView` carries no plugin-panel/terminal pane state; seam `tui_entry.rs:8334` |
| LAY-11 | `REAL_GAP` | Precedence single-valued island-only; seam `native_view_layout.rs:808 right_pane` |
| LAY-12 | `REAL_GAP` | Enter/permission guard in unstepped `native_host.rs:324-433`; only cosmetic `set_rail_interactive` live |
| LAY-13 | `REAL_GAP` | handle/drag/double-click in `native_pane_resize.rs:276,308`, test-only |
| LAY-14 | `ALREADY_CORRECT` | Independent scroll live-wired; consumed `tui_entry.rs:10010,7160,10196` |
| LAY-15 | `REAL_GAP` | Reserved action row const self-asserted only (`native_view_layout.rs:1898`), not painted live |
| SIDE-01 | `ALREADY_CORRECT` | `native_sidebar.rs:48-88,1346-1385`; live `native_view.rs:3356-3574` |
| SIDE-02 | `ALREADY_CORRECT` | `native_sidebar.rs:1153-1306`; live `native_view_status.rs:829-887` |
| SIDE-06 | `ALREADY_CORRECT` | `native_sidebar.rs:419-492`; live `native_view.rs:3512` |
| SIDE-07 | `ALREADY_CORRECT` | `native_sidebar.rs:364-590`; live `native_view.rs:3471-3494` |
| SIDE-09 | `ALREADY_CORRECT` | `native_view_status.rs:739-765`; `native_ui_preferences.rs:70-75`; live `tui_entry.rs:6772,10021-10036` |
| SIDE-10 | `BLOCKED` | Rendering correct (`native_sidebar.rs:640-732,1408-1441`); derivation seam missing: `todo_http.rs:31-38` returns stored `session_todos` verbatim; `stage_session_revert` (`server/src/lib.rs:1193-1209`) stores boundary but never recomputes; `tui_entry.rs:7508` refetches stored list |
| THEME-11 | `ALREADY_CORRECT` | Live loader `tui_entry.rs:6083` handles missing/invalid/unknown with fallback; consumer boot `:6744` + picker `:11425`; tests `:3020-3067` |
| THEME-12 | `REAL_GAP` | `ThemeController` (`native_theme_runtime.rs:1860`, tests `:3803/:3830/:3860`) but picker confirm `tui_entry.rs:11422-11430` calls `load_native_theme`+`persist_theme_selection` directly; no live preview-on-move/cancel-revert |
| THEME-13 | `REAL_GAP` | `SystemThemeState` (`native_theme_runtime.rs:2135`, tests `:3897/:3918/:3941`) but boot `tui_entry.rs:6738-6753` picks mode from prefs only; no palette probe/reply |
| THEME-14 | `REAL_GAP` | `SyntaxStyleArena` (`native_theme_runtime.rs:2374`, test `:3986`) zero non-test callers; `native_paint` `tui_entry.rs:4469` installs no syntax style |

**Additional wave-44 verdicts (fifth batch):**

| Item | Wave-44 verdict | Evidence / seam |
| --- | --- | --- |
| FFI-01 | `REAL_GAP` | cli never calls `probe_installed` (identity probe runs only in its own test `native_identity.rs:493`) |
| FFI-02 | `REAL_GAP` | `Scene`/`GenHandle` unused by cli; scene consumes handle/text internally |
| FFI-03 | `REAL_GAP` | `BridgeHandle` unused by cli |
| FFI-04 | `REAL_GAP` | cli renders via `safe_renderer::Renderer`, not `frame::FrameController` |
| FFI-05 | `REAL_GAP` | `text::wrap_cells` unused by cli; `text.rs:95-96` self-declares native equivalence NOT demonstrated |
| FFI-06 | `REAL_GAP` | cli markdown uses `native_markdown_flow`, not `markdown::MarkdownDoc` |
| THEME-15 | `ALREADY_CORRECT` | `native_theme_runtime.rs:2627 cursor_wire`; live `tui_entry.rs:8447 set_cursor_style`; tests `:4012/:4052/:4085` |
| THEME-16 | `ALREADY_CORRECT` | `native_text_selection.rs:372 ctrl_c_selection_key`; live `tui_entry.rs:10595/:10646`; paint `:4784/:5150`; tests `:402/:427` |
| THEME-17 | `ALREADY_CORRECT` | `native_view_layout.rs:515 char_width` + ZWJ/VS16/Tamil; `native_composer.rs:5-12`; live cursor `tui_entry.rs:8434`; tests `native_composer.rs:2373/2440/2450/2473` |
| THEME-18 | `ALREADY_CORRECT` | one segmentation (`visit_cells`/`visit_unpadded_cells`) shared by paint/clip/cursor/hit-test (`native_dock.rs:304`); tests `native_view.rs:7825`, `native_dock.rs:1885` |
| DIALOG-11 | `ALREADY_CORRECT` | palette module + live `view.command_palette_activate` dispatch (`tui_entry.rs:10727-10746`) |
| DIALOG-12 | `REAL_GAP` | `ModelPickerState` base wired (`native_setup.rs:144`→`tui_entry.rs:7355/9992`); `ModelVariantPicker`/`next_dialog_after_model_select`/favorites snapshot no live caller |
| DIALOG-13 | `REAL_GAP` | `SessionPicker` referenced only in `native_palette.rs` tests; seam `tui_entry.rs:10917` |
| DIALOG-14 | `REAL_GAP` | `DrillDown`/`LateLoadGuard` test-only; seam `tui_entry.rs:10917` |
| DIALOG-15 | `REAL_GAP` | palette `McpStatus`/`IntegrationPhase` test-only; seam `tui_entry.rs:10917` |

**Additional wave-44 verdicts (sixth batch):**

| Item | Wave-44 verdict | Evidence / seam |
| --- | --- | --- |
| CP-01 | `REAL_GAP` | `PromptSlot`/`prompt_slot` (`native_prompt_pane.rs:202,221`, test `:1665`) uncalled; live `native_view.rs:3056` ad-hoc slot |
| CP-02 | `REAL_GAP` | live `lip_row(width,[0x14,0x14,0x14,0xff])` hardcodes opaque bg; transparent variant (`native_prompt_pane.rs:1276`) unreachable |
| CP-03 | `ALREADY_CORRECT` | `prompt_surface` `native_prompt_pane.rs:119`; live `tui_entry.rs:8567` |
| CP-04 | `ALREADY_CORRECT` | rules `native_prompt_layout.rs:148-190`; live `native_view_chat.rs:928`→`native_view.rs:3137` |
| CP-05 | `REAL_GAP` | `metadata_runs`/`metadata_style` (`native_prompt_pane.rs:340,477`) no live producer; seam `native_view.rs:1612,3149` |
| CP-06 | `REAL_GAP` | `metadata_layout` (`native_prompt_pane.rs:378`) ports `metadata.tsx:117-156` but live `composer_metadata` re-implements it; seam `native_view.rs:1612` |
| TOOL-001 | `ALREADY_CORRECT` | `canonical_tool_name` (`native_tool_rows.rs:78`) + 14-kind enum (`:91-130`); live `native_view_chat.rs:2599` |
| TOOL-002 | `ALREADY_CORRECT` | spinner swap (`native_tool_rows.rs:1296-1306`), permission/error order (`:1283`); live `tui_entry.rs:7947`→`native_view.rs:1001`→`native_view_chat.rs:3086` |
| TOOL-003 | `ALREADY_CORRECT` | 2-cell icon column `header_prefix` (`:1345`), Background badge (`:1316`, test `:1748`); live `native_view_chat.rs:2633` |
| TOOL-004 | `ALREADY_CORRECT` | `primitive_input_summary:238`, `strip:1448`; tests `:1624/:1934`; live `native_view_chat.rs:2620/2633` |
| TOOL-005 | `ALREADY_CORRECT` | `SHELL_BUDGET_LINES:40`, `collapse_tail:1024`; tests `:1549-1602`; live `native_view_chat.rs:2607` |
| TOOL-006 | `REAL_GAP` | `BackgroundShell`/`ShellLoad`/`ShellChunk` (`native_shell.rs`) only in-file; client shells use one-shot `/shell/stream` (`server/src/lib.rs:294`); seam `native_shell.rs:1812` |
| MEDIA-001 | `REAL_GAP` | model+tests (`native_image_preview.rs:458,1047`, test `:1231`); page render live (`native_view_pages.rs:1056`→`native_view.rs:3031`) but no live caller of `PageState::open_image_preview` (`native_view_pages.rs:788`); preview always `None` |
| DIALOG-06 | `REAL_GAP` | `SelectDialog::filtered()` (`native_palette.rs:1409`, test `:3408`); no live consumer; seam `native_view_dialogs.rs:1423-1428` placeholder render |
| DIALOG-07 | `REAL_GAP` | `row_footer()` (`native_palette.rs:1136`, test `:3832`); same seam |
| DIALOG-08 | `REAL_GAP` | `page_down/page_up` (`native_palette.rs:1598`, test `:3442`); same seam |
| DIALOG-09 | `REAL_GAP` | `SelectExperiment` (`native_palette.rs:1151`, test `:3727`); same seam |
| DIALOG-10 | `LANDED` | `a52c222a` — live session rename drives the `DialogPrompt` (`DialogView::open_prompt` + `prompt_*` forwarders over `native_palette::select::PromptState`); ad-hoc paint path removed. |
| MSG-001 | `REAL_GAP` | Row model `reduce_session_rows` (`native_message_rows.rs:242,265,150,2088,960`, tests `:2148-2232`) never called by the live painter (`native_view_chat.rs:2505/3340` flatten `ChatMessage.parts`; grouping only by `starts_new_group` `:1294`) |

**Additional wave-44 verdicts (seventh batch):**

| Item | Wave-44 verdict | Evidence / seam |
| --- | --- | --- |
| CP-40 | `ALREADY_CORRECT` | `native_completion.rs` Kind/destructive/directory/hover + `panel_state`; 6 tests `:965-1169`; live `tui_entry.rs:6996-9101` |
| CP-41 | `ALREADY_CORRECT` | `native_text_selection.rs` cell-based spans; tests `:611`, `native_completion.rs:1232/1246`; live `tui_entry.rs:8489-10647` |
| CP-42 | `REAL_GAP` | Retry-notice transcript half live (`tui_entry.rs:2913`); `PromptSurface` streaming/retry chrome (`native_composer.rs:1481-1568`, test `:2078`) no live consumer; seam `tui_entry.rs:6498-6510` |
| MSG-002 | `REAL_GAP` | `native_message_rows.rs:165,177,191,203,150` (tests `:2244-2266`); `DisclosureState` zero non-def callers; live render always-expanded (`native_view_chat.rs:2569`) |
| MSG-003 | `REAL_GAP` | `partition_pending`/`group_refs` (`native_message_rows.rs:710,665,678`, tests `:2283/:2315`) uncalled; live pending = flat call-id set (`native_view_chat.rs:205`, wired `tui_entry.rs:7815,7861`) |
| MSG-004 | `REAL_GAP` | Semantic anchor model (`native_message_rows.rs:1400-1559`, `native_view_layout.rs:1042,1219,1291`; tests `:2346-2422`, `native_view_layout.rs:1989-2196`) zero runtime callers; live tab switch saves bare `view.chat_scroll_offset()` (`tui_entry.rs:12907`) via `native_navigation.rs:14,44,58` |

**Additional wave-44 verdicts (eighth batch):**

| Item | Wave-44 verdict | Evidence / seam |
| --- | --- | --- |
| DIALOG-01 | `ALREADY_CORRECT` | widths/cap/placement live in `native_modal_layer.rs:109,704,721`; consumer `native_view.rs:3696` (`native_palette.rs:647/656` is a test-only dup) |
| DIALOG-02 | `REAL_GAP` | shell gap/row-pad/footer-band helpers (`native_palette.rs:667-733`, test `:3808`) zero production callers. The shared-shell half is live-equivalent (`native_modal_layer.rs:704/721/735` duplicates `dialog_width`/`dialog_top`/`footer_is_narrow` with the same formulas), but the requirement's "list row padding / footer bands / both experiment states" is the `dialog-select.tsx`+`dialog.tsx` `newDesigns` behavior: upstream gates the title weight/color, description, footer gap and every select-row pad on `newDesigns` (`dialog.tsx:109-160`, `dialog-select.tsx:875-941`), while the live `DialogView::layout` (`native_view_dialogs.rs:1627`) never reads `new_designs` and uses its own fixed `" > "`/`"   "` row marker. The `SelectDialog` port that does implement it (`native_palette.rs:1251`) has zero live callers |
| DIALOG-03 | `ALREADY_CORRECT` | live stack `native_modal_layer.rs:437/382/387/532`; remount/refocus tests `:1150/:1292`, `native_view.rs:7728`; consumed `tui_entry.rs:7865/10676` |
| DIALOG-04 | `REAL_GAP` | ordered routes `escape_route:808`/`ctrl_c_route:821` (test `:7316`) no production caller; live `tui_entry.rs:10671-10700` does own selection/clear then `dialogs.cancel()` |
| DIALOG-16 | `REAL_GAP` | Base `SettingsDialog` shell is live (`tui_entry.rs:6340/7963/10735`), but the requirement's sub-behaviors are test-only: `NumericLimit`/`SCROLL_SPEED`/`LEADER_TIMEOUT`/`ALERT_VOLUME` (`native_settings.rs:509-559`) and `ExperimentsDialog` (`:561`) have zero non-test callers; `ThemePreview` (`native_palette.rs:2946`) no live consumer. Seam `tui_entry.rs:8832-8875`. (Resolved disagreement: the retry verdict is upheld over the earlier `ALREADY_CORRECT`.) |
| DIALOG-17 | `REAL_GAP` | `native_palette.rs:2999,3037,3141,3287` (tests `:4393,4423,4464`); no live consumer |
| DIALOG-18 | `REAL_GAP` | `native_view_dialogs.rs:3432,3609,3661,3767` (tests `:5351,5365,5430,5464,5493`); `diff_dialog` module test-only |
| DIALOG-19 | `REAL_GAP` | `native_overlay_stack.rs:38,360,243` + `native_modal_layer.rs:775` (tests `:1001..1181`); `ChromeModel`/Startup-Loading test-only |

## Final consolidated verdicts — all 223 items

Wave-44 is authoritative: where it disagrees with an earlier wave's per-ID entry,
the wave-44 verdict supersedes it.

| Verdict | Count |
| --- | --- |
| `ALREADY_CORRECT` | 144 |
| `REAL_GAP` | 2 |
| `BLOCKED` | 61 |
| `LANDED` (wave-45..53) | 16 |
| **Total** | **223** |

### Landed fixes (wave-45..51) — moved out of `REAL_GAP`/`BLOCKED`

| Item | Commit | What changed |
| --- | --- | --- |
| LAY-15 | `774ccbcd` | Live reserved action row is now derived from `SESSION_RESERVED_ACTION_ROWS` (`native_layout.rs:380`) instead of a hardcoded `u16::from(composer_room > 0)`; the live composed frame asserts `regions.jump.h` against the const. Behavior-preserving. Focused tests + full CLI suite (2164/0) green. |
| CP-01 | `a98c9afb` | Live session composer slot now resolves through `native_prompt_pane::prompt_slot` (`NativeView::session_prompt_slot`) instead of ad-hoc `permission_rows.is_some()`; an open auxiliary composer/dock or child-without-forms wins the slot, permission outranks form (upstream Switch `routes/session/index.tsx:1591-1660`). Focused tests + full CLI suite (2166/0) green. |
| CP-11 | `f8e276cf` | The shared footer line now feeds the provider-turn fact + a live monotonic clock into `RailFacts` and paints `RailSlot::Status` as the left term when non-empty, so a running session replaces the idle directory with the scanner + interrupt label (upstream `prompt.footer.status` Switch `index.tsx:1901-1967`). Adds `NativeView::set_rail_status_clock`, fed from the live loop via `busy_now` + `busy_since`. Focused tests + full CLI suite (2168/0) green. |
| CP-02 | `f8e276cf` | The live composer lip row now takes its background from the resolved `promptBg` (`prompt_surface`) instead of a hardcoded opaque value, so an alpha-0 transparent surface substitutes spaces (`index.tsx:1879,1890-1895`). Adds `PromptPaneInput.background` + `ChatView::set_prompt_background`, threaded from the live frame (`render_styled_with_spans_mode` → `rendered_prompt_pane`). Focused tests + full CLI suite (2168/0) green. |
| CP-05 / CP-06 | `3ef78186` | The live in-box metadata row now resolves through `native_prompt_pane::{metadata_layout, metadata_text}` instead of a parallel re-derivation, so the candidate order, the <44-column agent/provider drop, the provider-shortening fallback and the nine-cell model floor all come from the single authoritative model (`metadata.tsx:117-167`). Behavior-preserving; focused metadata tests + full CLI suite (2168/0) green. |
| MSG-007 | `4a6b487b` | The live message-click path opened the action menu with `message=None`, so Revert/Copy/Fork resolved against an empty snapshot. Now passes the live projection's `ChatMessage` at open and adds `SessionMessageMenu::resync`, called from the live input loop before dispatch so an open menu tolerates a projection resync without retargeting (`dialog-message.tsx:23`). Focused test + full CLI suite (2169/0) green. |
| SIDE-08 | `76fcb75a` | The sidebar MCP row click now opens the `DialogMcp` server list (`dialog-mcp.tsx`) instead of PUT-toggling in place: adds `DialogKind::Mcp`, `DialogView::open_mcp`/`open_kind`/`focused_choice`, `OpenDialog.mcp_status`, the Mcp render arm, `NativeView::mcp_entries`/`mcp_dialog_open`/`mcp_dialog_toggle`/`mcp_server_failed` and `dispatch_mcp_toggle`; Enter toggles the focused server via the same `PUT /api/sessions/{id}/mcp/{name}`, Esc closes locally. Focused tests + full CLI suite (2172/0) green. |
| MSG-007 / CP-11 review fix | `df4f4e29` | MSG-007: `SessionMessageMenu::resync` now takes the captured session id and guards `actions.session_id() == Some(session_id)`, so a live-session switch with a colliding message id cannot retarget Copy/Revert/Fork (`dialog-message.tsx:23`). CP-11: the running scanner + interrupt label moved from the shared shell footer (`footer_line`) into the composer footer slot (`composer_footer_row`, the `prompt.footer` slot) via `NativeView::composer_status_text`; the shell footer keeps the idle directory. Focused tests + full CLI suite (2172/0) green. |
| THEME-12 | `bc24dd30` | The live theme picker (`tui_entry.rs`) persisted only on Enter, so arrowing through the list showed no preview and Esc left the last previewed theme in place. Adds `ThemePicker` (a live wrapper over the tested `ThemeController`) plus `theme_picker_move`/`theme_picker_cancel`: page_next/page_prev on the Theme page preview+persist the focused row, Esc restores+persists the pre-open selection, Enter clears the picker (`dialog-theme-list.tsx:32-37` -> `context/theme.tsx:341-349`). Focused test `theme12_live_picker_previews_on_move_and_reverts_on_cancel` + full CLI suite (2173/0) green. |
| DIALOG-04 | `d137313d` | The ordered modal routes (`native_modal_layer.rs:489-600`) had no live consumer: the live loop quit the app on Ctrl-C even with a dialog open, and a backdrop click did nothing. Ctrl-C (0x03) now routes through the same close/reject path as Escape while the dialog stack owns input (`dialog_consumes_byte`); Ctrl-D (not bound in the modal layer) keeps its byte-exit meaning. A left-button release outside the panel rect closes the top dialog (`ui/dialog.tsx:54-60`); a pending decision is never abandoned. Focused test `dialog04_ctrl_c_closes_dialog_but_ctrl_d_keeps_byte_exit` + full CLI suite (2174/0) green. |
| PAR-04 | `669036cd` | Harness-side (Python): the Rust capture model already serialized `Frame.hit_targets` (`native_capture.rs:142`, canonical `hit={x},{y},{action}`) but the renderer transport dropped the key and the comparator had no member. Adds the key to every renderer output, a canonical `(y,x,action)`-sorted comparator member gated on the fixture's `compare` list, validation, README, selftest coverage and the matching empty-list goldens. The PTY channel exposes no pointer events, so the live `oc2-pty` adapter emits an empty list; the member is exercised by the deterministic static renderer. `tools/ui_parity/selftest.py` 22 checks PASS. |
| TAB-23 | `ae56897f` | The live `session.tab.*` dispatch was a hardcoded `InputEvent → TabCommand` match and the config `keybinds` root key (`config/index.tsx:78` `Schema.optional(TuiKeybind.KeybindOverrides)`) was never read. Adds `UiPreferences.keybinds` (raw JSON, like `cursor`), a `native_navigation::tab_keybinds` resolver (defaults + per-command replace, `false`/`"none"` disable, `KeyStroke` objects, both command-name spellings, unknown keys ignored, `super`/`hyper` dropped), and drives the live tab dispatch through the resolved chord table. No config yields exactly `DEFAULT_TAB_KEYBINDS`. Focused tests + full CLI suite (2176/0) green. |
| DIALOG-10 | `a52c222a` | The live session rename painted an ad-hoc rectangle with its own `String` buffer and never used the tested `DialogPrompt`. Adds `DialogView::open_prompt` (kind `Prompt`, seeded from `native_palette::select::PromptState`) plus `prompt_value`/`push`/`backspace`/`submit`/`escape`/`set_busy`/`set_message` forwarders, renders the value (or the busy line) and the upstream footer `Enter: save · Esc: cancel`, and routes the live rename through it (`MenuAction::Rename` opens the prompt; the key block edits/submits; the settlement closes it or sets busy/inline-error). Escape/submit are suppressed while busy (`dialog-prompt.tsx:43-46,93-100,168-170`). The ad-hoc `PaintCtx.rename_dialog` paint path is removed. Focused test + full CLI suite (2177/0) green. |
| DIALOG-20 (toast half) | `d891f9c3` | The tested `Toast`/`ToastFrame` model had no live consumer. Adds `PaintCtx.toast`, paints the frame as the topmost overlay in `native_paint` (surface fill + each styled `CellRun`), owns a live `Toast` in the loop whose logical clock advances on the animation tick so a visible toast auto-dismisses after 5000 ms, and triggers it from the session-id copy path (`Copied to clipboard`, `app.tsx:605`). The crash/error-details half remains `BLOCKED`. Full CLI suite (2177/0) green. |
| PAR-16 / PAR-22 | `f240367b` | Harness-side (Python). PAR-16: fixtures carry exactly one class (`baseline`/`optional`/`experimental`/`extension`); `fixture_class` defaults to `baseline`, `validate_fixture_class` rejects an unknown value, `--class` filters the selection, the class travels into `build_request`/`golden_from_screen`, and `compare_screens` raises a hard `fixture_class` diff on a golden/fixture class mismatch. PAR-22: `--evidence-bundle` + `build_evidence_bundle` write a SHA-bound bundle (`rust_sha`, spec/golden/fixture sha256s, class counts, exact reproduction commands, per-fixture outcomes, installed-capture hashes). `tools/ui_parity/selftest.py` 32 checks PASS (+10). |

### `REAL_GAP` — faithful model + focused test exist, no live consumer (2)

`MEDIA-001, DIALOG-02`.

Per-ID seams are recorded in the wave-44 batch tables above and in
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/wave44/<ID>.json`.
Selected high-value seams:

| Item | Seam |
| --- | --- |
| MEDIA-001 | `image_attachments`/`inline_tool_images`/`gallery_model` (`native_image_preview.rs:1047-1160`) have zero non-test callers; `PageState::open_image_preview` (`native_view_pages.rs:788`) never invoked live, so the preview always renders "no image attached". Wave-48 re-confirmed: no clipboard-image read path, no live `PromptPaneInput` attachment field, and `LocalFiles` (`native_image_preview.rs:67`) has only a test implementation. |
| DIALOG-02 | `native_palette.rs:667-733` dialog shell/list-row metrics are not read by the live `DialogView::layout` (`native_view_dialogs.rs:1627`), which ignores `new_designs`; the `SelectDialog` port (`native_palette.rs:1251`) that consumes them has zero live callers. |
| THEME-08 | `syntax_rules()` (`native_theme_runtime.rs:965`) has zero non-test callers; no tokenizer/highlighter producer and no renderer syntax-style sink (`safe_renderer.rs:906` takes explicit fg/bg/attrs only). |
| THEME-13 | `SystemThemeState`/`palette_reply` (`native_theme_runtime.rs:2183/2264`) have zero non-test callers; no OSC palette-query emitter and no payload-carrying input event to decode a reply. |
| THEME-09 | **BLOCKED (verified wave-53).** The live chat `Patch`/diff-fence painter hardcodes `DiffViewMode::Unified` (`native_view_chat.rs:2202/2730`) and `resolve_view_mode`/`config.diffs.view` (`native_diff_view.rs:586`) have no live caller. Threading the preference is not sufficient: a split row is `SplitRow::Pair { left, right }` (`native_diff_view.rs:709-715`) with two independent backgrounds, but the live row model is `(String, ChatRowClass)` with exactly one class per row (`native_view_chat.rs:902`) and the painter maps a class to a single background (`tui_entry.rs:2880-2898 chat_row_background`). The split view is therefore not renderable without extending the row model — the same blocker as DIFF-001. |

### `BLOCKED` — needs a producer/transport/contract field that does not exist (61)

`PAR-02, PAR-03, PAR-06, PAR-07, PAR-09, PAR-10, PAR-19,
TAB-04, TAB-06, TAB-10, TAB-21, SIDE-03, SIDE-04, SIDE-05, SIDE-10, SIDE-14,
MSG-001, MSG-002, MSG-003, MSG-005, MSG-008, MSG-009, MSG-010,
TOOL-006, TOOL-011, DIFF-001, DIFF-003, FFI-01, FFI-02, FFI-03, FFI-04, FFI-06,
FFI-07, FFI-08, FFI-11, FORM-001, STATE-002, THEME-08, THEME-09, THEME-13,
THEME-14, CP-07, CP-09,
DIALOG-06, DIALOG-07, DIALOG-08, DIALOG-09,
DIALOG-12, DIALOG-13, DIALOG-14, DIALOG-15, DIALOG-16, DIALOG-17, DIALOG-18,
DIALOG-19, LAY-10, LAY-11, LAY-13, MD-007, MD-008, MD-009`.

| Item | Exact missing seam |
| --- | --- |
| TAB-04 | `TabIndicators` (`native_tabs.rs:1771`) is consumed only by the non-live `indicator_text`; the live horizontal path uses `indicator_label` (`:1562`) with hardcoded status glyphs. No config loader produces `config.tabs.indicators` anywhere in `crates/` (zero producers); `from_navigation` (`:754`) never reads it. |
| TAB-06 | `enqueue_user_prompt` (`native_tabs.rs:1877`), `pending_marks_busy` (`:1893`) and `member_busy` (`:1900`) have zero live callers; live busy is only `work_state == Running` in `from_navigation` (`:754`). No producer/transport emits pending `PendingKind` items or cross-tab enqueue events; `LiveSnapshot` (`tui_entry.rs:1224`) carries no per-tab pending queue. |
| TAB-10 | `rail_inputs_for_navigation` (`tui_entry.rs:5277`) sets `detail` from `project_label` only; `session_tab_detail` (`native_tab_rail.rs:103`) needs current branch, default branch and worktree flag. `session_project_labels` is built from `session["directory"]` alone (`tui_entry.rs:1220-1222`); `Tab` (`native_navigation.rs:202-214`) has no branch/worktree field and no `current_branch`/`default_branch`/`is_worktree` producer exists in `crates/cli`. |
| MD-007 / MD-008 / MD-009 | The live fence painter `paint_fence_block` (`native_view_chat.rs:2285`) routes mermaid/math through `MermaidRenderer::without_painter` (`:2299`) / `MathRenderer::without_engine` (`:2321`), which fail closed because the only trait impls in the workspace are the no-op sentinels `NoDiagramPainter` (`native_mermaid.rs:451`) and `NoMathEngine` (`native_math.rs:420`). Upstream configures real painters unconditionally via builtin plugins (`builtins.ts:14,15,29,30` → `merman/src/plugin.ts:9`, `latex/src/plugin.ts:7-12`). `MarkdownPluginRegistry`/`set_fence_handlers` have zero live callers, and the live painter `push_block_markdown` (`:2343`) is a stateless per-frame repaint with no block-identity/epoch state. Needs a diagram-painter/math-engine/plugin-registration producer. |
| CP-09 | The composer border highlight (`prompt_highlight` `native_prompt_pane.rs:137`, `border_highlight` `:155`, `MetadataReveal`/`fade_alpha` `:693-737`) needs a live agent-color source; the live rail paints static `Slot::Border` (`tui_entry.rs:2822`). The agent catalogue parser extracts only name/description/read_only/aliases (`tui_entry.rs:5434-5473`, no `color`), and `set_composer_agent` carries name+variant only (`native_view.rs:1191/161`), so `agent_color` (`native_prompt_pane.rs:619`) has no producer. Needs a `color` field on the agent-catalogue contract plus a view setter. |
| DIALOG-06..10 | Select/Prompt/Confirm controllers (`native_palette.rs:1251-1295,1922-1933`) are fully ported + tested, but the live dialog body is a generic placeholder (`native_view_dialogs.rs:1393-1511`) and the live event handler opens only `Permission` (`tui_entry.rs:7868-7874`); there is no live `Select`/`Prompt` producer or route into `DialogView::open_request`. Needs a live route/host for those dialog kinds. |
| DIALOG-12..20 | The live dialog surface is `native_setup::Dialog` (`native_setup.rs:184`), and only the model-picker base rows are wired (`tui_entry.rs:8056`). Every assigned palette/settings/overlay/toast model has zero non-test callers and no producer for its driving facts: favorites/variants (`native_setup.rs:117`, `native_palette.rs:2344`), session-picker debounce/scope/armed-delete (`native_palette.rs:2505`), drill-down (`:1785`), MCP-status palette (`:2797`), numeric limits/experiments (`native_settings.rs:509/561`), stash/workspaces (`native_palette.rs:2999`), diff dialog (`native_view_dialogs.rs:3767`), overlay `ChromeModel::new` (`native_modal_layer.rs:775`), and the toast sink (`native_toast.rs:418`; `native_shell.rs:1755 set_toast` never called). |
| LAY-10 / LAY-11 / LAY-13 | `panel_can_split`/`panel_presentation`/`panel_toggle_fullscreen` (`native_pane_resize.rs:551-586`), `right_pane` (`native_view_layout.rs:808`) and `pane_resize_handle`/`PaneResize` (`native_pane_resize.rs:276/308`) are test-only; the live render composes only the sidebar (`native_view.rs:3602`). The seam is the missing live plugin-panel/terminal producer `panel_open`/`terminal_selected` (`native_shell_frame.rs:122-124`). |
| PAR-09 | Live code/patch render is plain `sanitize_code` text (`native_view_chat.rs:1273`); no syntax-highlight `HighlightedText` producer (tree-sitter/syntect) exists, and `HighlightService::complete` (`opentui-bridge/src/highlight.rs:782`) has no live call site. |
| PAR-10 | The live SSE consumer appends raw bytes to a draft `String` (`tui_entry.rs:917,920`) with no per-delta `stream_id`/`seq`, so `StreamAccumulator` (`native_transcript.rs:73`) / `TokenDelta` have no ordered-delta producer. |
| PAR-19 | `relative_scroll_top`/`visible_chunk_range` (`native_diff_view.rs:1043/1058`) need a diff-region `scrollTop` + `root.y`/`content.y` + viewport height producer; the live `paint_inline_patch` (`native_diff_inline.rs:83`, caller `native_view_chat.rs:2200`) has no scroll contract and flattens to ChatRow units with no chunk-to-row mapping. |
| MSG-001 / MSG-002 / MSG-003 | The live styled painter (`native_view_chat.rs:3248`) iterates `ChatMessage`/`ChatPart` flat; there is no `SessionRow`→`StyledChatRow` adapter, so `reduce_session_rows` (`native_message_rows.rs:960`), `DisclosureState` (`:165`) and `partition_pending` (`:710`) have zero live callers. Live pending is keyed by bare call id (`native_view_chat.rs:219`), not message-scoped `PartRef`. |
| FFI-01 | No artifact-pin verification step: `native_identity::identify` (`opentui-bridge/src/native_identity.rs:272`) has zero CLI callers, the live TUI never loads the pinned dylib, and vendored artifacts embed no commit string (`:13`) — a build-tooling seam, not live wiring. |
| FFI-03 / FFI-04 / FFI-06 / FFI-07 / FFI-08 / FFI-11 | The bridge `scene`/`frame`/`markdown`/`highlight`/`diff` models have no CLI consumer: the live tree is not generational (`native_view.rs:5652`), `tui_paint.rs` paints directly with no `FrameController` ticket seam (`opentui-bridge/src/frame.rs:210`), live rows are re-derived per frame (`native_view_chat.rs:1044`), no grammar/query transport exists for highlight (`highlight.rs:1`), the live diff painter lacks the bridge diff parse/selection model (`native_diff_view.rs:119`), and live focus is `DialogView`-local (`tui_entry.rs:10666`) not `Scene`-keyed (`scene.rs:402`). Adopting any replaces the live render substrate. |
| DIFF-001 / DIFF-003 | Split-mode arithmetic is correct (`native_diff_view.rs:600`) but has no live consumer: `resolve_view_mode` is only reached from unconsumed `build_*` fns and the live render hardcodes unified (`native_view_chat.rs:2707`); the added-file virtualization model (`native_diff_view.rs:1014-1076`) has zero callers and `AddedPatchChunk` (`:926-930`) carries no highlight field. Needs config/width threading + a `SplitRow→ChatRowClass` adapter + a diff-viewer component. |
| THEME-14 | No syntax-style install/destroy API on the checked OpenTUI bridge ABI: `opentui-bridge/src/text.rs:7` records `syntaxStyleRegister` "intentionally absent"; `highlight.rs:18-29` binds no text-buffer style call; `native_identity.rs:85` `REQUIRED_SYMBOLS` has no syntax-style symbol; `safe_renderer.rs:906` `draw_text_styled` has no style-table parameter; `tui_entry.rs:4469` `native_paint` installs none. `SyntaxStyleArena` (`native_theme_runtime.rs:2374`) has no live sink/destroy target, so wiring it to the `Copy` `HighlightTheme` tables would be a no-op facade, not consumption of upstream `createSyntaxStyleMemo` (`packages/tui/src/context/theme.tsx:402-421`). Needs an operator-authorized ABI extension. |
| FFI-02 | No native scene entry point: the bridge `Scene` renders a node tree to `CellGrid` (`scene.rs:845`), while the live CLI paints styled strings directly to `NativeRenderer` (`tui_entry.rs:4469/8334`); the two render substrates are disjoint. Every capability FFI-02 names already has a live non-Scene equivalent (measurement `native_box_style.rs:695`, clip `tui_entry.rs:4511`, focus `native_host.rs:81`), and its fixture `bridge-scene_move` (`fixtures.json:8607`) targets a `run_native_fixture` case that does not exist live. Wiring `Scene` would duplicate the whole paint path or be test-only dead code. |
| TOOL-006 | The tested `BackgroundShell` load cycle (`native_shell.rs:1809-2119`) has no live consumer, and the server exposes only `POST /api/sessions/{id}/shell/stream` (`server/src/lib.rs:294`) — there is no `GET /api/shell` or `/api/shell/{id}/output` route, so a client poll of `shell.output` (upstream `packages/client/src/promise/generated/client.ts:2045`) would be non-functional. Live shell output flows through `native_turn::TurnWorker::start_shell` (`native_turn.rs:108-122`) instead. Needs the server route + live background-shell discovery. |

| Item | Exact missing seam |
| --- | --- |
| SIDE-03 / SIDE-04 | Live `set_usage` (`native_view_status.rs:1146`) sums `input+output` only; `set_usage_classes` (`:1168`) and `set_cost` (`:1160`) have zero non-test callers. No `lastAssistantWithUsage` selector, no revert boundary reaching the view, no cost producer, and the wire `input_tokens` is inclusive of cached (`server/src/auto_compaction.rs:102`), so disjoint classes need a `nonCachedInput` producer. |
| SIDE-05 | `set_limit` (`native_view_status.rs:1175`) test-only; no usage-limits pipeline reaches the CLI. |
| SIDE-10 | `todo_http.rs:31-38` returns the stored `session_todos` table verbatim; `stage_session_revert` (`server/src/lib.rs:1193-1209`) stores the boundary but never recomputes the latest-completed-`todowrite` list sliced at the revert boundary (upstream `util/todo.ts:32-43`). |
| SIDE-14 | `footer_menu` (`native_sidebar.rs:1020`) test-only; no footer hit rect / hover routing / click branch / CopyPath-OpenFolder-Workspaces executors. Direct re-inspection: `Copy path`/`Open folder` are bounded, but upstream's `Workspaces` action opens `DialogWorkspaces` (`component/prompt/move.tsx` `move.open()`), which has no live route, so the item is not fully bounded. |
| STATE-002 | Live transcript renders compaction as a placeholder (`native_view_chat.rs:2689`); projection (`native_chat_projection.rs:392`) drops status/summary/provider-native/tokens; rich `CompactionState` (`native_timeline.rs:382`) has zero consumers; `compact_session` (`server/src/lib.rs:1668`) never persists/pushes a compaction part. |
| MSG-005 | Chips need a server/storage `files`/`skills` field that does not exist (`MessageRecord` `contracts/src/lib.rs:250-257`). |
| MSG-008 | Footer TPS needs `time.streamed`; `MessageExecution` (`contracts/src/lib.rs:284-293`) has no timing field and the producer frame (`server/src/lib.rs:2895-2901`) emits none; `RowTiming.streamed_ms` (`native_message_rows.rs:825`) never populated live; `config.data.session.tps` absent. |
| MSG-009 | `TranscriptSurfaces`/`RevertCard`/`TranscriptNotice`/`ChildCompletionNotice` (`native_view_chat.rs:4482/4103/4273/4409`) have no live consumer; `SessionRevert` (`contracts/src/lib.rs:191`) has no count; server stage route hardcodes `files:None`; no notice projection; no child-completion metadata. |
| MSG-010 | `turn_tokens_per_second` (`native_message_rows.rs:1242`) reads `step.timing.streamed_ms` (`:1259`), zero live callers, field has no producer. |
| TOOL-011 | `execute_dialog` (`native_tool_rows.rs:1159`) zero consumers; live render placeholder (`native_view_dialogs.rs:1429`); no `ExecuteDialog` payload on `OpenDialog`/`DialogView`; no syntax producer (`opentui-bridge/src/highlight.rs:27-29`). |
| TAB-21 | `ViewAcknowledgement` (`native_navigation.rs:1133`) zero live callers; no client POST `/api/sessions/{id}/view` in `crates/cli`; no idle/viewed timestamp reaches the CLI (`LiveSnapshot` `tui_entry.rs:573`; `SessionSummary` `contracts/src/lib.rs:202-213`). |
| FORM-001 | The upstream-correct `PermissionPrompt` (`native_permissions.rs:557`) is dead code; the live path (`native_view_dialogs.rs:1090`) is a flat choice list. Sub-behavior 1 (3-option realign) is **not** a parity bug: the canonical server supports `scope ∈ {once,session,always}` (`run_owner.rs:232`) and the 6-choice scoped set is a deliberate extension protected by semantic tests. Sub-behaviors 3 (always-body swap, static `:1420`) and 4 (nested rejection stage) need a stage+textarea in `DialogView`. |
| PAR-02 / PAR-03 / PAR-06 / PAR-07 | Harness-side: capability table empty (`audit-implementation.md:79-96`); no upstream TS/OpenTUI adapter (`README.md:148`); no font/pixel capture path; event-trace sink misses queue/steer/permissions/forms. |

### `ALREADY_CORRECT` — implementation + focused test + traced live consumer (144)

Everything not listed in the two sections above. This includes the full
`CP-12..CP-39` range, `TAB-01..03/05/07..09/11..20/22/24`, `SIDE-01/02/06/07/09/11/12/13/15`,
`THEME-01..04/06/07/10/11/15..18`, `DIALOG-01/03/05/11`, `FFI-09/10`,
`MD-001..006`, `MSG-006`, `TOOL-001..005/007..010/012`, `DIFF-002/004/005`,
`FORM-002/005..008`, `STATE-001/003/004`, `LAY-01..09/14/16/18/19/20`,
`CP-03/04/10/40/41`, `PAR-01/05/08/12/13/14/15/17/18/20/21/23/24`.

Wave-45 reclassification: `CP-42` moved here. Its two required behaviors are
already live via independent mechanisms — the network retry message is a
persistent transcript row (`native_chat_projection.rs:383` `ChatPart::Retry` →
`native_view_chat.rs:2694-2702` `ChatRowClass::RetryNotice`, styled at
`tui_entry.rs:2913/2962/3127`), and prompt busy/idle is the provider-turn fact
`busy_now = binding_active && !worker_finished` (`tui_entry.rs:8332/8359`),
decoupled from transcript contents. The tested `PromptSurface`/`TurnPhase` model
(`native_composer.rs:1483-1568`) has zero non-test callers, so it is a redundant
model, not a missing live consumer.

Wave-46 reclassification: `CP-08` moved here. The live model-load path already
retains prior composer labels by construction — `model_load.poll()`
(`tui_entry.rs:7343`) gates every setter, so nothing is written while a fetch is
in flight; on `Ok` it replaces via `set_model` + `set_composer_model`
(`:7364-7365`); on `Err` it records `setup_error` only (`:7374`), leaving the
prior label intact until the initial-load branch (`:6797-6804`) clears it via
`set_model_unknown`. The retention requirement is satisfied by a different
mechanism, so `PromptDisplay`/`next_prompt_display` is a redundant model.

Wave-47 reclassification: `FFI-05`, `LAY-12`, `LAY-17`, `MSG-004` and `PAR-11`
moved here; each requirement is met live by a different mechanism, so the ported
model is a redundant (not missing) consumer:
- `FFI-05` (Unicode/cell-exact widths): the live cell-exact width impls with
  exact-equality tests (`native_tab_rail.rs:1098,1254`,
  `native_completion.rs:535,1257`) satisfy the acceptance clause "demonstrated
  equivalent Unicode/cell results".
- `LAY-12` (focus guards): the permission prompt gates all keys via
  `Dialogs::intercepts_input` (`native_view_dialogs.rs:643`) at
  `tui_entry.rs:8964/9021/9141`; pane focus is the live `rail_interactive`
  (`native_view.rs:1250`) set at `tui_entry.rs:9169`.
- `LAY-17` (reflow/scroll anchors): the live `RenderedChatAnchor`
  (`native_view_chat.rs:1005`) via `chat_rendered_anchor`/
  `restore_chat_rendered_anchor` (`native_view.rs:2892/2906`) at
  `tui_entry.rs:10218/10236` and reflow `:7121/:7156`; per-tab independence via
  `navigation.save_active_state` (`native_navigation.rs:653`).
- `MSG-004` (saved scroll anchors): live anchor preservation via
  `resize_chat_preserving_anchor` (`tui_entry.rs:1321`), backfill
  `:7121/7156`, resize `:7398/8183`.
- `PAR-11` (message-anchored scroll): live via `rendered_anchor`/
  `scroll_for_anchor` (`native_view_chat.rs:3435,3460`) exposed by `NativeView`
  (`native_view.rs:2892,2906`) and consumed in the live loop
  (`tui_entry.rs:1340,1345`).

## Coverage and method

Wave-44 covered every item still `UNVERIFIED` after wave-43 (165 IDs) with 28
parallel workers on the four authorized routes; workers that hit step caps were
re-dispatched in ≤6-ID slices. The remaining 58 IDs were already verdicted in
earlier waves. Per-ID JSON verdicts are authoritative and live in
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/wave44/`.
A placeholder `UNVERIFIED`/`"note":"stub"` JSON is **not** a verdict. Wave-46
re-verification JSONs live in
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/wave46/`.
Wave-47 re-verified every remaining `REAL_GAP` item (45 IDs) with the decision
rule "does the LIVE UI produce the same observable behavior as upstream?":
5 moved to `ALREADY_CORRECT` (a live equivalent mechanism exists), 1 landed
(`MSG-007`), and 29 moved to `BLOCKED` (the required producer/transport/contract
field is absent).

## Honest completion statement

Of the 223 checklist obligations, **144 are `ALREADY_CORRECT`** (a faithful
implementation, a focused test, and a traced live consumer), **2 are
`REAL_GAP`** (model + test exist but the live TUI does not consume them),
**61 are `BLOCKED`** (a producer/transport/contract field does not exist), and
**16 are `LANDED`** (`LAY-15`, `CP-01`, `CP-11`, `CP-02` in wave-45;
`CP-05`, `CP-06` in wave-46; `MSG-007` in wave-47; `SIDE-08`, `THEME-12`,
`DIALOG-04` in wave-48; `PAR-04` in wave-49; `TAB-23`, `DIALOG-10`,
`DIALOG-20` (toast half) in wave-51; `PAR-16`, `PAR-22` in wave-53).

The remaining work is therefore **not** more UI drawing:

- `REAL_GAP` items need the live TUI wired to models that already exist, each
  with a focused test. Several are cosmetic refactors where the live path has an
  equivalent inline implementation, so wiring changes structure, not pixels.
- `BLOCKED` items need cross-crate contract/server changes (serialized by the
  integrator): usage classes + cost + revert boundary (`SIDE-03/04/05/10`),
  compaction transport (`STATE-002`), message chips (`MSG-005`), streamed
  timing (`MSG-008/010`), revert/notice surfaces (`MSG-009`), view
  acknowledgement (`TAB-21`), execute-dialog payload
  (`TOOL-011`), split-diff config (`DIFF-001/003`), the permission body-swap
  stage (`FORM-001`), the syntax-style bridge ABI (`THEME-14`), the native scene
  entry point (`FFI-02`), the `shell.output` server route (`TOOL-006`), the
  agent-color producer (`CP-09`), a live Select/Prompt dialog route
  (`DIALOG-06..10`), the palette/settings/overlay/toast producers
  (`DIALOG-12..20`), a live plugin-panel/terminal producer (`LAY-10/11/13`), a
  diagram-painter/math-engine/plugin-registration producer (`MD-007..009`), a
  `SessionRow`→painter adapter (`MSG-001/002/003`), a syntax-highlight producer
  (`PAR-09`), an ordered-delta stream producer (`PAR-10`), a diff-region scroll
  producer (`PAR-19`), a harness hit-target transport (`PAR-04`), the bridge
  render-substrate models (`FFI-01/03/04/06/07/08/11`), and the tab
  indicators/pending/branch producers (`TAB-04/06/10`).
  `FORM-001` sub-behavior 1 is a deliberate extension, not a gap.

### Wave-48 re-confirmation (2026-11-03)

Independent workers re-examined the remaining `REAL_GAP`/`BLOCKED` seams against
upstream pin `450e7b92412c780a7b5a75226bb8d49aaca24e22` and confirmed the
following as genuinely missing producers (not bounded wiring):

- `THEME-08` — no tokenizer/highlighter producer and no renderer syntax-style
  sink; `syntax_rules()` stays test-only.
- `THEME-13` — no OSC palette-query emitter and no reply-carrying input event;
  `SystemThemeState`/`palette_reply` stay test-only.
- `CP-07` / `CP-09` — no live selected-agent id and no `color` field on the
  agent catalogue contract; `AgentFact`/`set_composer_agent` stay test-only.
- `TAB-10` — no `current_branch`/`default_branch`/`is_worktree` producer;
  `session_tab_detail` stays test-only.
- `TAB-21` — the `idle` half of the unread pair is absent from storage,
  contract, transport and snapshot; wiring the `/view` POST alone cannot
  produce an observable unread transition.
- `TOOL-011` — no `ExecuteDialog` payload on the dialog contract, no live
  per-row tool-part owner, and no highlight producer.
- `STATE-002` — no compaction message producer: `compact_session`
  (`server/src/lib.rs:1668`) returns an ack only, `MessageRole`
  (`contracts/src/lib.rs:222`) has no compaction variant, and the live render is
  a placeholder (`native_view_chat.rs:2712`).
- `MSG-008` / `MSG-010` — no per-step streamed timestamp: no contract field, no
  storage column, no server capture and no `session.tps` config key; the
  duration half is already live-correct.
- `LAY-10` / `LAY-11` / `LAY-13` — no live plugin-panel/terminal producer feeding
  `panel_open`/`terminal_selected` (`native_shell_frame.rs:122-124`).

`THEME-05` was reclassified `ALREADY_CORRECT`: upstream product code never calls
the `state()` precedence combinator either (its only callers are upstream's own
library test `packages/tui/test/theme/v2/component.test.ts`); the live TUI
already reproduces the same observable behavior through discrete-token
resolution plus hand-written precedence.

`DIALOG-04` was the one bounded item and is now landed (`d137313d`). `THEME-09`
is re-scoped: the diff colors are live, but the upstream width>120 auto-split
(`routes/session/index.tsx:3457-3459`) is unreachable because the live chat
painter hardcodes `DiffViewMode::Unified` and `config.diffs.view` is never
parsed.

### Wave-49 implementation-owner results (2026-11-03)

Wave-49 reframed the remaining work as implementation ownership (build the
smallest real producer chain, else BLOCKED with the exact seam). Outcomes:

- `PAR-04` — **LANDED** (`669036cd`, see the fixes table). The only bounded item
  the fleet returned.
- `SIDE-05` — re-confirmed BLOCKED: the whole usage-limits producer chain has no
  Rust port. No `/api/experimental/usage/limits` route, no core service mirroring
  `packages/core/src/usage-limits.ts`, no `UsageLimits.Info` contract, no
  `modelUsage`/`servedByConsole` reduction. The data sources from the external
  OpenCode Console API with a signed-in workspace credential, so wiring only the
  CLI `set_limit` half would be a no-op.
- `MSG-005` — re-confirmed BLOCKED: no `files`/`skills` field anywhere in the
  producer→transport→contract→storage→serializer→CLI chain (upstream keeps them
  as top-level user-message fields, `prompt.ts:77-85`).
- `DIFF-001` / `DIFF-003` — re-confirmed BLOCKED: split paints one terminal row
  as two 50% panes with up to four style regions, which the flat
  `(String, ChatRowClass)` row model cannot represent (one fg + one uniform row
  bg; `ChatRunSpan` maps flags to fg/attrs only). Bounded upgrade path recorded:
  add bg/fg fields to `ChatRunSpan` + an x-ranged `role_bg` override in the paint
  loop (`tui_entry.rs:4623-4666`), then route the live `Patch`/fence painters
  through `paint_inline_patch` with `config.diffs.view` + measured width.
- `SIDE-14` — reconnaissance only (worker hit its step limit): the footer menu
  model is complete, but the live path needs a footer hit rect, hover routing, a
  click branch and the three action executors. Re-inspected directly: the
  `Copy path` and `Open folder` actions are bounded, but upstream's `Workspaces`
  action opens `DialogWorkspaces` (a move dialog, `component/prompt/move.tsx`
  `move.open()`), which has no live route — so the item is not fully bounded.
- `TAB-04`, `TAB-23`, `TOOL-011`, `THEME-09`, `CP-09`, `DIALOG-06..10`,
  `DIALOG-12..20` — workers returned no deliverable (provider/step-limit
  failures); no new evidence, prior BLOCKED seams stand.

### Wave-51 grouped-area results (2026-11-03)

Base `1ea3d25a`. 16 grouped workers (one per area, 3-6 related items each) on the
four authorized routes. `TAB-23` — **LANDED** (`ae56897f`): see the fixes table.
Every other item re-confirmed as before:

- `TAB-10` — no server VCS producer / contract branch field / git probe
  (cross-crate chain). `TAB-21` — the `idle` half of the unread pair is absent
  from storage, contract and server (no `set_session_idle`, no idle column, no
  `ServerEvent` idle variant); `ViewAcknowledgement` has zero live callers.
- `PAR-09/10/19` — no syntax-engine dependency, no wire `seq`/`messageID`, no
  mounted diff-scroll substrate. `PAR-A` (`PAR-02/03/06/16/22`) — harness-side
  gaps (no upstream capture profile, no class dimension, no evidence bundle);
  bounded harness patches identified but not landed.
- `THEME-13` — no OSC palette-query emitter bound in the FFI and both input
  decoders discard OSC payloads by design. `THEME-08` — no tokenizer + sink.
- `CP-09` — no live agent-color producer (catalogue omits `color`,
  `set_composer_agent` has zero live callers, `agent.cycle` inert).
- `STATE-002` — no `Compaction` message role in contract/storage, no server
  persistence. `MSG-001/002/003` — no `SessionRow`→painter adapter.
  `MSG-005/008/009/010` — no `files`/`skills` field, no per-step streamed
  timestamp. `TOOL-006` — no `shell.output` server route; executor refuses
  `background`. `LAY-10/11/13` — no plugin-panel/terminal producer.
- `DIALOG-12..20` — toast (`DIALOG-20`) **LANDED** (`d891f9c3`, toast half);
  the crash/error-details half and the rest need live driving-fact producers.
  `DIALOG-06..10` — `DIALOG-10` (Prompt / session
  rename) **LANDED** (`a52c222a`); `DIALOG-06..09` (Select) are blocked by the
  pinned SIDE-08 `DialogMcp` consumer.

This document is a status map, not acceptance. No item is marked
runtime-proven; captured styled-cell/cursor goldens were not produced.
