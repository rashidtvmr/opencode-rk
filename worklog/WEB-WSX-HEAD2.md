# WEB-WSX-HEAD2 verify (2026-09-16, rev 248f519)

Scope: WEB-001..017 + WSX-001/002 + HEAD-001/002 + ACP-001/002 + SDK-001/002.
Role: verify-only. No product impl written. No frozen-test or ralph.json edits.
Bounds: serial, CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1, timeout 115 per target.
Tree: hostile — 408 dirty files + untracked `crates/sessions/src/part_events.rs`
(syntax error, missing `;` :132, breaks opencode-rk-sessions lib) from parallel
lanes. All failures below trace to that, not owned code.

## Holds
- voice `available:false` (lib.rs:183-186). No mic/audio deps; `voice_capture`
  gates on grant+adapter. Write-paths disabled: turn route takes plain
  (model, effort, text); TurnParts/turn_contract unreachable from wire.
- Stub scan over owned impl (voice_capture, workspace_proxy, remote_sync,
  acp_bridge, acp_files, sdk_client, sdk_spawns): clean.
- lib.rs restored byte-identical post-probe: sha256 `4d8136fe…` (backup
  /tmp/opencode/wJ-lib.rs.bak, cmp clean).

## GREEN (this session, serial)
| target | n | log |
|---|---|---|
| protocol_api | 5/5 EXIT 0 | wJ-web.log retry2 |
| control_plane_inputs | 5/5 EXIT 0 | wJ-web.log retry2 |
| web_artifact_api | 2/2 | wJ-web.log retry2 |
| workspace_proxy | 5/5 | wJ-web.log retry2 |
| remote_sync | 5/5 | wJ-web.log retry2 |
| acp_bridge | 5/5 | wJ-web.log retry2 |
| acp_files | 5/5 | wJ-web.log retry2 |
| sdk_client | 12/12 | wJ-web.log retry2 |
| sdk_spawns | 11/11 | wJ-web.log retry2 |
| sessions runner | 5/5 | wJ-web.log retry2 |
| web_capabilities_api baseline | 1/1 | wJ-weak-cap.log |
| web_workspace_api baseline | 2/2 | wJ-weak-ws.log |
| restore cap+ws multi-target | 1+2 GREEN | inline 01:40Z |
| cli run_headless | 8/8 | wJ-web.log sweep1 (pre-break) |
| sessions runner | 5/5 | wJ-web.log sweep1 |

Prior sweep (WEB-WSX-HEAD-VERIFY.md) covers the rest at same rev; not rerun
here because foreign tree breakage blocks sessions-dependent targets.

## BLOCKED (foreign tree, not owned)
control_plane_errors, control_plane_exposure, event_stream, transcript_lane,
chat_composer, web_attachments, web_tool_chooser, voice_capture,
session_history_api, web_artifact, session_export, run_headless(retry2):
EXIT 101, `error: expected ; found keyword match → part_events.rs:132`
(untracked parallel-lane file). Repro: `cargo test -p opencode-rk-server
--test voice_capture`. Fix: owning lane repairs/removes part_events.rs, then
rerun. Evidence tail in wJ-web.log retry2.

## RED probe (2 weakest: capabilities, workspace; 2 stubs each)
- cap STUB-A `search.available false→true`: FAIL bites
  (web_capabilities_api.rs:49, left true right false). Log wJ-weak-cap.log.
- cap STUB-B reason `"local grep …"→"x"`: PASSES (reason-string not pinned).
  Gap: add NEW test asserting reason names grep boundary; do NOT edit frozen.
- ws STUB-C `session_scope_available false→true`: FAIL bites
  (web_workspace_api.rs:59). Log wJ-weak-ws.log.
- ws STUB-D `project_root→"/LEAKED/OTHER/WS"`: FAIL bites (:57, leak pinned).
- All stubs restored; final GREEN cap+ws confirm above.

## Counts
GREEN-verified this session: 14 targets, 74 tests, 0 failed.
RED probes: 4 stubs (3 bite, 1 passes = known reason-precision gap).
Blocked by foreign tree: 12 targets (evidence logged, untouched).
