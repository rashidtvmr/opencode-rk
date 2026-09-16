# WEB/WSX/HEAD verify sweep (2026-09-16)

Scope: non-accepted WEB/WSX/HEAD only. AGENT-031 accepted — skipped.
Base: `248f519` + dirty tree from parallel lanes (not mine).
Bounds: JOBS=2 THREADS=2, timeout 120, serial runs. Zero product/test edits this pass.

## GREEN reruns (serial, exit 0)

| task | target | result | log |
|---|---|---|---|
| WEB-001 | server `--test control_plane_inputs` | 5/5 | inline |
| WEB-002 | server `--test control_plane_errors` | 5/5 | inline |
| WEB-003 | server `--test protocol_api` | 5/5 | inline |
| WEB-004 | server `--test control_plane_exposure` | 5/5 | inline |
| WEB-005 | server `--test event_stream` | 5/5 | inline |
| WEB-006 | server `--test web_singleton_lock` + `--test session_branch_api` | 2/2 + 4/4 | /tmp/opencode/wsing.log |
| WEB-007 | server `--test transcript_lane` | 5/5 | combined run |
| WEB-008 | sessions `--test web_008_lane` | 5/5 | /tmp/opencode/w8008-full.log |
| WEB-009 | server `--test turn_parts` | 5/5 | combined run |
| WEB-010 | server `--test chat_composer` | 5/5 | combined run |
| WEB-011 | server `--test web_attachments` | 5/5 | combined run |
| WEB-012 | server `--test web_tool_chooser` | 5/5 | combined run |
| WEB-013 | server `--test web_capabilities_api` | 1/1 (boundary only, no T01-T05 suite) | /tmp/opencode/wcap2.log |
| WEB-014 | server `--test session_history_api` | 2/2 (boundary only) | /tmp/opencode/whist2.log |
| WEB-015 | server `--test web_workspace_api` | 2/2 (boundary only) | /tmp/opencode/wws2.log |
| WEB-016 | server `--test voice_capture` | 5/5, capability `available:false` holds | combined run |
| WEB-017 | server `--test web_artifact_api` | 2/2 (boundary only) | /tmp/opencode/wart2.log |
| WSX-001 | server `--test workspace_proxy` | 5/5 | inline |
| WSX-002 | server `--test remote_sync` | 5/5 | inline |
| HEAD-001 | cli `--test run_headless` | 8/8 | /tmp/opencode/wcli-both.log |
| HEAD-002 | cli `--test session_export` | 8/8 | /tmp/opencode/wcli-both.log |

## Holds verified

- `#[path]` includes in all owned test files; no `lib.rs`/`ralph.json`/frozen edits this pass.
- Stub scan (`todo!`/`unimplemented!`/placeholder/stub) over all 16 owned impl files: clean.
- WEB-016: `voice.available=false`; `voice_capture` gates capture on grant+adapter, no audio deps, no mic/process spawn strings.
- Write-paths stay disabled: turn route takes plain `(model, effort, text)`; `TurnParts`/`Selection::select`/`turn_contract`/chip-send unreachable from wire.

## Gaps (verifier-owned)

1. WEB-003/WEB-004: unit-contract GREEN but NOT wired into live axum `router()` (projection table vs `/api/*` superset; exposure gate has no middleware).
2. WEB-005: Hub bounds GREEN; live `GET /events` transport wiring with transport owner.
3. WEB-009: no native `tool_calls` producer, no durable citation contract; T01/T03/T05 end-to-end blocked.
4. WEB-010/011/012: chip/tool/attachment-send write-paths disabled by design; HTTP contracts for slash/mentions/queue-steer absent.
5. WEB-013/014/015/017: GREEN-only boundary evidence (1/2/2/2 tests); no frozen T01-T05 suites; browser vitest/tsc unexecuted.
6. HEAD-002: test hash on disk `e213d1bd…` matches neither frozen RED `593d615b…` nor post-fmt `439273ab…`; predates pass, untouched — needs re-freeze note.
7. Transient: one combined multi-target run failed with `E0583 mod runner` + `mcp_transport` unclosed delimiter while parallel lanes wrote; serial reruns all GREEN. Tree still dirty from other lanes (sessions `lib.rs` +3 mods, providers fmt).
8. Browser DOM/a11y (reflow, focus, keyboard popover) runner-unverifiable here; acceptance blocked per cards.
