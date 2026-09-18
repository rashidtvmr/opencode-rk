# CONFIRM-P13 — verify-only rerun (rev b60ceda)

Mode: VERIFY-ONLY. No product/test/config edits. Owned file: this report only.
Budget: serial runs, `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`, `timeout 120`,
`free -h` before each batch (~1.0 Gi avail shown; 8 GiB budget held, one suite
at a time). Frozen tests, `lib.rs`, `ralph.json`, mic untouched.

## 1. TOOL/SYNC/UI — 8 suites, 40/40 GREEN, log /tmp/opencode/p13-tool.log

| suite (crate :: test target) | tests | pass | fail |
|---|---|---|---|
| tools :: mcp_catalog_search (TOOL-016) | 5 | 5 | 0 |
| tools :: mcp_bulk_actions (TOOL-017) | 5 | 5 | 0 |
| tools :: mcp_lifecycle (TOOL-018) | 5 | 5 | 0 |
| tools :: mcp_payload_filter (TOOL-019) | 5 | 5 | 0 |
| sessions :: mcp_status_panel (TOOL-020) | 5 | 5 | 0 |
| server :: sync_log (SYNC-001) | 5 | 5 | 0 |
| sessions :: part_events (SYNC-002) | 5 | 5 | 0 |
| sessions :: tui_info_panel (UI-019) | 5 | 5 | 0 |
| TOTAL | 40 | 40 | 0 |

Stub scan over 8 owned src files
(`mcp_catalog_search/bulk/lifecycle/payload_filter`, `mcp_status_panel`,
`sync_log`, `part_events`, `tui_info_panel`): no `todo!()`/`unimplemented!()`
(grep exit 1 = clean).

## 2. WEB-001..017 sweep — log /tmp/opencode/p13-web.log

Server-side contract suites (each `--test-threads=1`, serial):

| WEB id | suite | tests | result |
|---|---|---|---|
| WEB-001 | server control_plane_inputs | 5 | GREEN |
| WEB-002 | server control_plane_errors | 5 | GREEN |
| WEB-003 | server protocol_api | 5 | GREEN |
| WEB-004 | server control_plane_exposure | 5 | GREEN |
| WEB-005 | server event_stream | 5 | GREEN |
| WEB-006 | server web_singleton_lock (T02+T04, 2 tests in file) | 2 | GREEN |
| WEB-006 | server web_assets (T03 embedded origin, 1 test) | 1 | GREEN |
| WEB-006 | server web_entry_probe (T03 probe, 5 tests) | 5 | GREEN |
| WEB-007 | server transcript_lane | 5 | GREEN |
| WEB-008 | sessions web_008_lane | 5 | GREEN |
| WEB-009 | server turn_parts | 5 | GREEN |
| WEB-010 | server chat_composer | 5 | GREEN |
| WEB-011 | server web_attachments | 5 | GREEN |
| WEB-012 | server web_tool_chooser | 5 | GREEN |
| WEB-013 | sessions web_013 | 5 | GREEN |
| WEB-014 | sessions chat_nav_lane | 5 | GREEN |
| WEB-015 | sessions project_context | 5 | GREEN |
| WEB-016 | server voice_capture (state-machine model) | 5 | GREEN |
| WEB-017 | server web_artifact | 5 | GREEN |

Totals: 19 targets, 88 passed, 0 failed.
Voice: `crates/server/src/voice_capture.rs` (366 lines) is a bounded
state-machine model (permission/adapter gating, transcript caps,
reconnect dedup); no native audio capture path in server src
(no `MediaRecorder`/`getUserMedia`/`cpal`/`rodio` hits in server src).
Capability manifest in `crates/server/src/lib.rs:176,180,184` reports
voice/search/research `available:false` with daemon reasons; composer
never requests mic while unavailable. No mic/audio path added.
Acceptance caveats from cards hold: WEB-009 producer, WEB-010 HTTP
chip contracts, WEB-011 unified store+adapter, WEB-012 execution-owned
approvals, WEB-013 executor, WEB-014 pin/share/search/temp-chat
persistence, WEB-015 membership/memory, WEB-016 real adapter, browser
`vitest`/`tsc` unexecuted. Verifier decides acceptance.

## 3. ACP/WSX/SDK — 7 suites, 48 tests GREEN (this session, serial)

| suite | tests | result |
|---|---|---|
| server acp_bridge (ACP-001) | 5 | GREEN |
| server acp_files (ACP-002) | 5 | GREEN |
| server remote_sync (WSX-002) | 5 | GREEN |
| server workspace_proxy (WSX-001) | 5 | GREEN |
| server sdk_client (SDK-001) | 12 | GREEN |
| server sdk_spawns (SDK-002) | 11 | GREEN |
| server sync_log | 5 | GREEN (counted in §1) |
| ACP/WSX/SDK subtotal (excl. sync_log dup) | 43 | GREEN |

## 4. CLI — 5 suites, 23 tests GREEN (this session, serial)

| suite | tests | result |
|---|---|---|
| cli doctor_diagnostics | 5 | GREEN |
| cli run_headless (HEAD-001) | 8 | GREEN |
| cli session_export (HEAD-002) | 8 | GREEN |
| cli web_entrypoint (WEB-006-T03) | 1 | GREEN |
| cli web_singleton_runtime (WEB-006-T01+T05) | 1 | GREEN |
| TOTAL | 23 | GREEN |

Providers spot (same session): auth_profile 5, codex_oauth 5,
prov_017..024 5 each (40), codex_oauth_bounds 6, codex_oauth_bounds2 6 —
all GREEN, 0 failed.

## 5. Guards

- Rev `b60ceda1eaec7f3a7df0c0eb17328a6eef6368dc`. Worktree has
  pre-existing unrelated modifications (FEATURES.md, ralph.json,
  REL worklogs, other CONFIRM lanes); this lane wrote only
  `worklog/CONFIRM-P13.md` plus `/tmp/opencode/p13-*.log` files.
- No edits to frozen tests, `lib.rs`, `ralph.json`, mic by this lane.
- No stubs found in scanned owned paths. No `passes:true` emitted;
  evidence is per-suite `test result: ok` lines in the logs.

## 6. Counts for orchestrator

- TOOL/SYNC/UI: 8 suites, 40/40 (`/tmp/opencode/p13-tool.log`).
- WEB-001..017 sweep: 19 targets, 88 passed, 0 failed
  (`/tmp/opencode/p13-web.log`; WEB-006 spans 3 files: 2+1+5).
- ACP/WSX/SDK: 6 suites + sync_log dup = 48 tests GREEN.
- CLI: 5 suites, 23 tests GREEN.
