# WEB-VERIFY4 — WEB-001..017 verify-only rerun (no product/test edit)

Claim: all WEB suites GREEN at worktree over base rev `248f519`. No stubs. Voice `available:false`, no mic/audio path. Chip/tool write-paths stay disabled. `lib.rs` READ-ONLY (untouched by this lane). `ralph.json` untouched.

## Source evidence
- Base rev `248f519` (`git rev-parse HEAD` → `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`).
- Cards `tasks/WEB-001.md`..`tasks/WEB-017.md`; prior worklogs `worklog/WEB-001.md`..`worklog/WEB-017.md`, `WEB-007-012-ACCEPT.md`, `WEB-014-017-SUITES.md`.
- `crates/server/src/lib.rs:34` `pub mod voice_capture`, `:183-186` voice `available:false` ("no native transcription or realtime audio adapter").
- `ralph.json`: WEB-001..006 `in-progress`, WEB-007..017 `not-started` (read-only check, no edit).
- Worktree dirty (431 files, other lanes) — NOT this lane. `git diff HEAD -- ralph.json` empty. `lib.rs` worktree diff (+20 `pub mod` lines) predates lane; lane made zero edits.

## Observed scenario
- Serial runs, `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`, `timeout 120`, `free -h` before each (≈2.9–3.0 Gi avail, 8 GB budget held, one suite at a time).
- `cargo check -p opencode-rk-server` → 0 errors. No E0432 observed → per order, nothing to fix, `lib.rs` NOT touched.
- Stub scan `grep -rn "todo!()\|unimplemented!()" crates/server/src/` → no matches.
- Audio scan `grep -rn "MediaRecorder\|getUserMedia\|cpal\|rodio\|webrtc" crates/server/src/` → no matches. `voice_capture.rs` has no `Command::new|std::process|tokio::process|cpal|rodio|webrtc|getUserMedia|MediaRecorder|alsa|pulse`.
- Write-path scan `crates/server/src/lib.rs`: no `TurnParts`/`push`, no `turn_contract`, no `Selection::select`, no `lower_composer_doc`. Live paths present and out of scope: `create_draft_attachment` (:465), `append_assistant_with_reasoning` (:894).

## Tests (107 passed, 0 failed, frozen files untouched)
| suite | result |
|---|---|
| control_plane_inputs + control_decode | 5+5 pass |
| control_plane_errors + error_translate | 5+6 pass |
| protocol_api + route_table | 5+5 pass |
| control_plane_exposure + event_stream | 5+5 pass |
| web_assets + web_singleton_lock + web_entry_probe | 1+5+2 pass |
| transcript_lane + turn_parts | 5+5 pass |
| chat_composer + web_attachments | 5+5 pass |
| web_tool_chooser + voice_capture | 5+5 pass |
| web_artifact + web_artifact_api + session_history_api | 5+2+2 pass |
| web_capabilities_api + web_workspace_api | 1+2 pass |
| chat_nav_lane + web_008_lane (sessions) | 5+5 pass |
| session_messages_api + session_turn_api | 1+1 pass |
| session_turn_stream_api (`--test-threads=1`) | 2 pass |
| cli web_entrypoint + web_singleton_runtime | 1+1 pass |
| TOTAL | 107 passed, 0 failed |

Full per-suite output tail logged in `/tmp/opencode/yK-web.log`.

## Target boundary
- Owned: this worklog + log file only. No product/test/config edit. No frozen/ralph.json touch.
- NOT owned: `crates/server/src/lib.rs` wiring, capability block, voice adapter, research executor, workspace membership/memory authority, browser `vitest`/`tsc` (unexecuted).

## Decisions
- No code change (verify-only). GREEN already held; no stub found; E0432 absent so no report beyond this note.
- `ponytail:` mic `OperationIntent` + human-grant flow, research executor, unified blob store, chip write-path HTTP contracts deferred (need integration proposal + human authority).

## Remaining unknowns (verifier decides acceptance)
- Browser DOM/focus/reflow (`web/` vitest/tsc unexecuted). WEB-009 producer + citation contract. WEB-010 chip/queue HTTP contracts. WEB-011 unified store + provider adapter. WEB-012 execution-owned approvals. WEB-013 executor/replay. WEB-014 pin/share/search/temp-chat. WEB-015 membership/memory. WEB-016 real adapter. Integrator wiring + full regression on integrated rev.
