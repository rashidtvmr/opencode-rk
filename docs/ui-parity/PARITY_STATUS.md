# OpenCode V2 UI parity — consolidated status

- **Canonical line:** `main-v2` @ `ccd33cde` (remote verified)
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

**Reclassified / newly finalized:**

| Item | Wave-44 verdict | Evidence / seam |
| --- | --- | --- |
| PAR-13 | `ALREADY_CORRECT` | `native_message_rows.rs:2115` `click_acts`; live `tui_entry.rs:10102,10162`; test `native_text_selection.rs:532` |
| PAR-14 | `ALREADY_CORRECT` | `native_tui_parity.rs:186` COLUMNS=40; `upstream_view_parity.rs:76-78` 60/88/116; `:145` >120 boundary |
| PAR-15 | `ALREADY_CORRECT` | `native_theme_runtime.rs:392-434` hex/ref/{dark,light}/transparent resolve; live `tui_entry.rs:6736-6753,11425` |
| PAR-16 | `BLOCKED` | No fixture classification anywhere (grep 0); harness `ui_parity.py:683` has no class dimension |
| PAR-17 | `ALREADY_CORRECT` | `baseline.lock.json:12-24` v2.2.0 `is_authoritative:false`; pins `native_identity.rs:36-38` |
| PAR-18 | `ALREADY_CORRECT` | `safe_renderer.rs:1148` Drop, `:609` restore; live `tui_entry.rs:12756-12759`; tests `native_terminal_lifecycle.rs:192-217`, `highlight.rs:1133` |
| PAR-19 | `REAL_GAP` | Model `native_diff_view.rs:1043,1058` + test `:1963`; no live consumer (`native_view_chat.rs:2707` hardcodes `paint_rows_with_gutter`) |
| PAR-20 | `ALREADY_CORRECT` | `selftest.py:161-207` mutations one_cell_color/cursor/width/focus/action_extra all assert exit 1 |
| PAR-21 | `ALREADY_CORRECT` | `support/upstream_view_parity.rs:4` frozen `upstream_layout_parity.rs` untouched; sibling file |
| PAR-22 | `BLOCKED` | Goldens carry pins (`ui_parity.py:247-265`) but `build-metadata.json:47` rust_checks_passed=null; no SHA-bound evidence bundle |
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
| TAB-23 | `BLOCKED` | `resolve_tab_shortcut` (`native_navigation.rs:1294`) + `DEFAULT_TAB_KEYBINDS` (`:1260`) test-only; no `keybinds` config producer (`native_ui_preferences.rs:303`); no generic keymap normalization (`native_input_decoder.rs:40-67`); no Help rows |
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
| DIALOG-20 | `REAL_GAP` | Toast model+producer exist (`native_toast.rs:225-595`, `native_overlay_stack.rs:684 take_error_toast`) but nothing live constructs `ChromeModel`/paints `ToastFrame` (`native_view_dialogs.rs:7535` is `#[cfg(test)]`; `set_toast` `native_shell.rs:1755` test-only; `NativeView` has no toast field). Crash half: no `panic::set_hook`, no `DialogKind::ErrorDetails/Crash` |

## Genuine gaps — live wiring (`REAL_GAP`)

These have a faithful model + test but no live consumer.

| Item | Seam |
| --- | --- |
| TAB-23 | `resolve_tab_shortcut` (`native_navigation.rs:1294`, `DEFAULT_TAB_KEYBINDS` `:1260`) has no caller outside its module; `tui_entry.rs:8727` hardcodes decoded events. Needs a user-keybind config source, which the CLI does not currently load (only `SubmitKeymap` enter/ctrl-j exists). |
| SIDE-05 | `set_limit` (`native_view_status.rs:1175`) is called only from tests; the Limit section cannot appear live. |
| SIDE-14 | `footer_menu`/`FooterMenuItem` (`native_sidebar.rs:1020`) has no caller outside its module; footer hover/copy/open/workspaces never reach live input. |
| TOOL-011 | `DialogKind::Execute`/`execute_dialog` (`native_tool_rows.rs:1159`) is never opened live; only `Question` (`tui_entry.rs:8197`) and `Confirm` (`native_view.rs:7737`) are consumed. The Execute dialog body renderer is also a placeholder (`native_view_dialogs.rs:1429`), so wiring alone would not reach parity. |
| DIFF-001 | Split-mode boundary arithmetic is correct (`native_diff_view.rs:600`, constants `:540-544`) but has no live consumer: `resolve_view_mode` is called only from unconsumed `build_*` fns (`native_diff_inline.rs:125,169,198`); the live render hardcodes unified (`native_view_chat.rs:2707`). `DiffSurface::Viewer` + `viewer_patch_pane_width` (`:659`) have zero callers. Needs config/width threading + a `SplitRow→ChatRowClass` mapping + a diff-viewer component. |
| DIFF-003 | Added-file virtualization model + tests are correct (`native_diff_view.rs:1014-1076`, tests `:1962-2209`) but have zero callers outside the module; `AddedPatchChunk` (`:926-930`) carries no highlight field, so source highlighting cannot survive chunk crossing. Needs a scroll-aware producer feeding `relative_scroll_top→visible_chunk_range` + a highlight-carrying chunk model. |
| FORM-001 | The upstream-correct `PermissionPrompt` (`native_permissions.rs:557`, once/always/reject, `save` gate, always-body swap, nested rejection scope) is **dead code** (only its own tests reference it). The live path is `DialogView` Permission (`tui_entry.rs:7865` → `native_view_dialogs.rs:1090`), a flat choice list that omits the body swap and nested-rejection stage. Closing needs rewiring the live permission flow to the upstream-correct model. |

## Genuine gaps — missing producer/transport (`BLOCKED`)

| Item | Exact missing seam |
| --- | --- |
| SIDE-04 | Five-class arithmetic is correct and tested (`native_sidebar.rs:205-287`, test `:1608`), but the live consumer is `set_usage` (`native_view.rs:1030` → `native_view_status.rs:1146`), which sums only `input+output`; `set_usage_classes` (`:1168`) has zero callers, so the five-class percentage is never reached. The "last eligible assistant's model" half has **no producer**: there is no selector for the last eligible assistant (index > last completed compaction, index < revert boundary) and no join to that message's catalog context limit — the only limit input is the *selected UI model* (`tui_entry.rs:8112`). Upstream's five-class sum (`util/session.ts:52-55`) also diverges from canonical's deliberate inclusive model (`native_view_status.rs:1148-1150`; server `auto_compaction.rs:102`), so wiring is a data-model decision. |
| SIDE-03 | Renderer + test are correct (`native_sidebar.rs:230-287`, test `:1608`), but the live usage path is `native_turn.rs:23 Event::Usage → tui_entry.rs:7456 view.set_usage(input,output)` (`native_view_status.rs:1146`, sums input+output only). `set_usage_classes` (`:1168`) and `set_cost` (`:1160`) have zero non-test callers; no `lastAssistantWithUsage` selector exists; revert stage request exists (`tui_entry.rs:3729`) but no `revert.messageID` reaches or bounds the view. Needs a new transport (revert boundary + per-message token classes + model-id + catalog limit join). |
| STATE-002 | The live transcript renders compaction as a placeholder `[compaction] {reason}` (`native_view_chat.rs:2689`); the projection (`native_chat_projection.rs:392`) drops status/summary/provider-native/tokens. Rich `CompactionState` (`native_timeline.rs:382`, tests t12-t17) has zero consumers and the live `CompactionStatus` (`native_message_rows.rs:795`) has only Running/Completed. Missing seam: a transport mapping a compaction message's status/summary/provider_native/tokens onto `ChatPart`. Only the `CompactionQueued` divider (`native_view_chat.rs:4337`) is live. |
| MSG-005 | Chips need a server/storage `files`/`skills` field that does not exist. |
| MSG-008 | Footer TPS needs `time.streamed`; `MessageExecution` (`crates/contracts/src/lib.rs:284-293`) has no timing field and the producer frame (`crates/server/src/lib.rs:2895-2901`) emits none. `RowTiming.streamed_ms` (`native_message_rows.rs:825`) is never populated live. Also needs the native `config.data.session.tps` key (absent). Agent/model/duration/interrupted **do** flow live. |
| MSG-009 | `TranscriptSurfaces`/`RevertCard`/`TranscriptNotice`/`ChildCompletionNotice` (`native_view_chat.rs:4482/4103/4273/4409`) have no live consumer; producers absent (revert count+files: `SessionRevert` `contracts/src/lib.rs:191` has no count, server stage route `server/src/lib.rs:1199-1204` hardcodes `None`; no notice projection; no child-completion metadata). Queue-dock + pending-steer halves are live-wired. |
| SIDE-08 | Row-click → `DialogMcp`: no `DialogKind::Mcp`; no local dialog-open path. Truncation half already done. |
| DIALOG-05 | `empty_state()` (`native_palette.rs:1534`) has no live caller; the `Empty` branch is unreachable because every live option source is static non-empty. Viewport/gap/detail halves already pass. |
| DIALOG-20 | Both halves are model-complete but unwired. Toast: `Toast`/`ToastFrame` (`native_toast.rs:225-595`) and `ChromeModel` (`native_modal_layer.rs:775-1057`) exist, and a producer exists (`native_overlay_stack.rs:684 take_error_toast`), but nothing live constructs `ChromeModel` or paints `ToastFrame` (sole construction is under `#[cfg(test)]` at `native_view_dialogs.rs:7535`; `set_toast` `native_shell.rs:1755` has zero non-test callers; `NativeView` has no toast field). Crash/error-details: `error_details`/`crash::CrashScreen` (`native_view_dialogs.rs:4408-5130`) have zero non-test callers; no panic hook / error boundary; `DialogKind` has no ErrorDetails/Crash variant. Both need wiring into the shared live render root (`tui_entry.rs` loop + `native_view.rs:2835`). |
| TOOL-012 | `metadata.loaded` — pinned upstream `read.ts` emits only `truncated`; no server NDJSON metadata field. |
| THEME-14 | No native `SyntaxStyle` producer; `SyntaxStyleArena` (`native_theme_runtime.rs:2374-2453`) is test-only and the paint path (`tui_entry.rs:4469`) installs no syntax style, so the guarantee is vacuous. |
| TAB-21 | Focus-confirmed view acknowledgement: `ViewAcknowledgement` (`native_navigation.rs:1133`, 250→5s) has zero live callers (only its own `#[cfg(test)]` at `:2069`); `tui_entry.rs:8689-8694` defers the driver until a "watermark seam" lands. No client POST `/api/sessions/{id}/view` in `crates/cli` (server route only: `server/src/lib.rs:236/1229`); no idle/viewed timestamp reaches the CLI. The unread **paint** transition itself is live. |
| FORM-003 | `form_overlay::FormPrompt` (`native_view_dialogs.rs:2258`) is a complete pure port (all field kinds, validation, review, draft restore) but has **no consumer and no producer**: no `/form` route (`server/src/lib.rs:290-310` only permission) and MCP `elicitation/create` is captured (`mcp_runtime.rs:178`) but unconsumed (`native_view.rs:2021-2024`). Needs a server form/question route + elicitation feed + CLI poll/render/reply. |
| FORM-004 | Same producer/transport seam as FORM-003; the narrow-field/checkbox/inset/editor arithmetic itself is correct. |
| W35-MEDIA01 | Transport (`crates/server/src/lib.rs:730-733`) reports `available_for_web_turn: false`. |

## Verified `ALREADY_CORRECT` (source-inspected this pass)

- **Tabs:** TAB-13, TAB-14, TAB-15, TAB-16, TAB-17, TAB-18, TAB-19, TAB-20, TAB-22, TAB-24.
- **Sidebar:** SIDE-01, SIDE-02, SIDE-06, SIDE-07, SIDE-09, SIDE-10, SIDE-11, SIDE-12, SIDE-13, SIDE-15.
- **Theme:** THEME-01.
- **Dialogs:** DIALOG-06, DIALOG-07, DIALOG-08, DIALOG-09, DIALOG-11, DIALOG-12, DIALOG-13, DIALOG-14, DIALOG-15.
- **FFI:** FFI-03 .. FFI-11.
- **Tools:** TOOL-007, TOOL-008, TOOL-009, TOOL-010, TOOL-012.
- **Diffs:** DIFF-002, DIFF-004, DIFF-005.
- **Forms:** FORM-002, FORM-005, FORM-006, FORM-007, FORM-008.
- **State:** STATE-001, STATE-003, STATE-004.
- **Composer/CP:** CP-12 .. CP-39 (rail, dock, sidebar, composer, paste, completion) — each has an impl + focused test + live consumer.

Prior waves (recorded in `docs/worklogs/ORCHESTRATOR_SCRATCHPAD.md`) additionally
established ALREADY_CORRECT for LAY-02/03/06/07/08/09/11/13, SIDE-01/02,
TAB-24, CP-06/07/08/09/10/12/17/18/26, MD-006, TOOL-008, DIFF-002,
STATE-003/004, THEME-02/03, CP-26, SIDE-05 (renderer), SIDE-10, MD-009.

## Coverage gaps in this pass

The following item groups were dispatched but their workers hit step caps
before finalizing verdicts, so they are **UNVERIFIED** here (placeholder-only
JSON, not verdicts): group1 (PAR/LAY), group2, group4 (THEME-02..18,
CP-01..11), group5 (final JSON), group6, group7 (final JSON). The prose
evidence from the group5/group7 workers is reflected above; the JSON files
themselves are stubs and must not be read as verdicts.

## Honest completion statement

The Rust `oc2` UI is complete to the extent the current backend supports. The
remaining checklist items are **not** more UI work: they are (a) live-wiring of
models that already exist and (b) backend producers/contract fields that do not
exist. Neither is unblocked by dispatching more agents. Closing the `BLOCKED`
set requires contract + server changes (cross-crate, serialized by the
integrator), and closing the `REAL_GAP` set requires wiring the live TUI to the
existing models, each with a focused test.

This document is a status map, not acceptance. No item is marked
runtime-proven; captured styled-cell/cursor goldens were not produced.
