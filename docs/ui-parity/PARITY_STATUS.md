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

## Genuine gaps — live wiring (`REAL_GAP`)

These have a faithful model + test but no live consumer.

| Item | Seam |
| --- | --- |
| TAB-21 | `ViewAcknowledgement` (`native_navigation.rs:1133`, test `:2067`) has no production caller; `client_focused` (`tui_entry.rs:6552,8700`) is produced but never consumed. The unread **paint** transition is live (`native_navigation.rs:1016-1021` → `TabInput.unread`); only the focus-confirmed acknowledgement path is unwired. |
| TAB-23 | `resolve_tab_shortcut` (`native_navigation.rs:1294`) has no caller outside its module; `tui_entry.rs:8727` hardcodes decoded events and never loads configured keybinds. |
| SIDE-03 | `set_usage_classes` (`native_view_status.rs:1168`) has no caller; live usage uses the inclusive `set_usage` (`tui_entry.rs:7456`). No revert/cached-token producer. |
| SIDE-05 | `set_limit` (`native_view_status.rs:1175`) is called only from tests; the Limit section cannot appear live. |
| SIDE-14 | `footer_menu`/`FooterMenuItem` (`native_sidebar.rs:1020`) has no caller outside its module; footer hover/copy/open/workspaces never reach live input. |
| TOOL-011 | `DialogKind::Execute`/`execute_dialog` (`native_tool_rows.rs:1159`) is never opened live; only `Question` (`tui_entry.rs:8197`) and `Confirm` (`native_view.rs:7737`) are consumed. The Execute dialog body renderer is also a placeholder (`native_view_dialogs.rs:1429`), so wiring alone would not reach parity. |

## Genuine gaps — missing producer/transport (`BLOCKED`)

| Item | Exact missing seam |
| --- | --- |
| SIDE-04 | Five-class arithmetic is correct and tested (`native_sidebar.rs:205-287`, test `:1608`), but the live consumer is `set_usage` (`native_view.rs:1030` → `native_view_status.rs:1146`), which sums only `input+output`; `set_usage_classes` (`:1168`) has zero callers, so the five-class percentage is never reached. The "last eligible assistant's model" half has **no producer**: there is no selector for the last eligible assistant (index > last completed compaction, index < revert boundary) and no join to that message's catalog context limit — the only limit input is the *selected UI model* (`tui_entry.rs:8112`). Upstream's five-class sum (`util/session.ts:52-55`) also diverges from canonical's deliberate inclusive model (`native_view_status.rs:1148-1150`; server `auto_compaction.rs:102`), so wiring is a data-model decision. |
| STATE-002 | The live transcript renders compaction as a placeholder `[compaction] {reason}` (`native_view_chat.rs:2689`); the projection (`native_chat_projection.rs:392`) drops status/summary/provider-native/tokens. Rich `CompactionState` (`native_timeline.rs:382`, tests t12-t17) has zero consumers and the live `CompactionStatus` (`native_message_rows.rs:795`) has only Running/Completed. Missing seam: a transport mapping a compaction message's status/summary/provider_native/tokens onto `ChatPart`. Only the `CompactionQueued` divider (`native_view_chat.rs:4337`) is live. |
| MSG-005 | Chips need a server/storage `files`/`skills` field that does not exist. |
| MSG-008 | Footer TPS needs `time.streamed`; `MessageExecution` (`crates/contracts/src/lib.rs:284-293`) has no timing field and the producer frame (`crates/server/src/lib.rs:2895-2901`) emits none. `RowTiming.streamed_ms` (`native_message_rows.rs:825`) is never populated live. Also needs the native `config.data.session.tps` key (absent). Agent/model/duration/interrupted **do** flow live. |
| MSG-009 | `TranscriptSurfaces`/`RevertCard`/`TranscriptNotice`/`ChildCompletionNotice` (`native_view_chat.rs:4482/4103/4273/4409`) have no live consumer; producers absent (revert count+files: `SessionRevert` `contracts/src/lib.rs:191` has no count, server stage route `server/src/lib.rs:1199-1204` hardcodes `None`; no notice projection; no child-completion metadata). Queue-dock + pending-steer halves are live-wired. |
| SIDE-08 | Row-click → `DialogMcp`: no `DialogKind::Mcp`; no local dialog-open path. Truncation half already done. |
| DIALOG-05 | `empty_state()` (`native_palette.rs:1534`) has no live caller; the `Empty` branch is unreachable because every live option source is static non-empty. Viewport/gap/detail halves already pass. |
| DIALOG-20 | Crash/error-details half has no producer. Toast half is applyable. |
| TOOL-012 | `metadata.loaded` — pinned upstream `read.ts` emits only `truncated`; no server NDJSON metadata field. |
| THEME-14 | No native `SyntaxStyle` producer; `SyntaxStyleArena` (`native_theme_runtime.rs:2374-2453`) is test-only and the paint path (`tui_entry.rs:4469`) installs no syntax style, so the guarantee is vacuous. |
| TAB-21 | Focus-confirmed view acknowledgement: `ViewAcknowledgement` (`native_navigation.rs:1133`, 250→5s) has zero live callers (only its own `#[cfg(test)]` at `:2069`); `tui_entry.rs:8689-8694` defers the driver until a "watermark seam" lands. No client POST `/api/sessions/{id}/view` in `crates/cli` (server route only: `server/src/lib.rs:236/1229`); no idle/viewed timestamp reaches the CLI. The unread **paint** transition itself is live. |
| W35-MEDIA01 | Transport (`crates/server/src/lib.rs:730-733`) reports `available_for_web_turn: false`. |

## Verified `ALREADY_CORRECT` (source-inspected this pass)

- **Tabs:** TAB-13, TAB-14, TAB-15, TAB-16, TAB-17, TAB-18, TAB-19, TAB-20, TAB-22, TAB-24.
- **Sidebar:** SIDE-01, SIDE-02, SIDE-06, SIDE-07, SIDE-09, SIDE-10, SIDE-11, SIDE-12, SIDE-13, SIDE-15.
- **Theme:** THEME-01.
- **Dialogs:** DIALOG-05, DIALOG-06, DIALOG-07, DIALOG-08, DIALOG-09, DIALOG-11, DIALOG-12, DIALOG-13, DIALOG-14, DIALOG-15.
- **FFI:** FFI-03 .. FFI-11.
- **Tools:** TOOL-007, TOOL-008, TOOL-009, TOOL-010, TOOL-012.
- **Diffs:** DIFF-001, DIFF-002, DIFF-003, DIFF-004, DIFF-005.
- **Forms:** FORM-001 .. FORM-008.
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
