# WEB-CLOSE3 worklog — WEB-001..017 GREEN verify + 2 weakest RED probes (NOT ACCEPTED)

Base rev `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`. Workdir dirty from
parallel lanes; lane touched NO product/test file (verify-only; transient
`xk_weak_*` probe files created then deleted; restored). `lib.rs` sha256
`4d8136fe…` unchanged. `ralph.json` NOT touched (WEB-001..006 in-progress,
WEB-007..017 not-started). All runs serial, JOBS=1 THREADS=1, timeout 120.

Claim: GREEN boundary evidence across all 17 surfaces; 2 weakest RED probes
logged; write-paths stay disabled; `voice.available:false` holds; no mic/audio
enabled. NOT ACCEPTED (verifier decides; browser suite unexecuted).

## Source evidence (exact lines, this rev)

- `crates/server/src/lib.rs:34` `pub mod voice_capture;` (read-only wiring,
  no routes/handlers); `:135-194` `web_capabilities()`; `:183-186`
  `voice.available:false` "no native transcription or realtime audio adapter";
  `:175-182` search/deep_research unavailable; `:170-174` attachments
  `available_for_web_turn:false`; `:196-236` `list_workspaces` registry-only
  (`session_scope_available:false`, `memory_available:false`); `:381-406`
  history paging (default 50, clamp 1..100, unknown cursor 404);
  `:607-679` branch/retry/provenance; `:688-961` turns plain
  `(model,effort,text)` only, never ComposerDoc; `:977-1002`
  `responses_history` rejects tool/blob entries (no fake-text degradation).
- WEB-001 `control_plane_inputs.rs:86` `decode_move_session_input`,
  `:8` 64 KiB cap; WEB-002 `control_plane_errors.rs:49` `translate`,
  `:86` `wire_body`; WEB-003 `protocol_api.rs:51-67` 3-route table,
  `:124` `check_served` subset invariant; WEB-004
  `control_plane_exposure.rs:71` peer-gate, `:105` POST /control/* only;
  WEB-005 `event_stream.rs:52-108` SSE framing/bounds, `:198` Hub lifecycle.
- WEB-007 `transcript_lane.rs` read-only geometry; WEB-008 sessions
  `web_008_lane.rs` inclusive atomic fork + single-slot retry; WEB-009
  `turn_parts.rs` projection contract, no tool_calls producer; WEB-010
  `chat_composer.rs` lowering model, HTTP chip path disabled; WEB-011
  `web_attachments.rs` draft ingest only, send closed; WEB-012
  `web_tool_chooser.rs` read-only registry, selection never serialized.
- WEB-016 `voice_capture.rs:1-366` pure state machine (`MicGrant`/`MicState`/
  `VoiceConfig`/`VoiceSession`); `start` (:230-243), `unmute` (:255-269),
  `push_utterance` (:284-312) gate on grant+adapter; `adapter=false` →
  `NoAdapter`, zero transcript side effects; `live_tracks()` counter only,
  max 1, no FDs. `Cargo.toml:9-23` no audio deps (per WEB-016 worklog).
- No-mic proof: audio-IO token grep (`cpal|rodio|webrtc|getUserMedia|
  MediaRecorder|Command::new|std::process|tokio::process|alsa|pulse`)
  over `voice_capture.rs` → 0 hits (see weak log §1).
- No write-call proof: `lib.rs` references to
  `voice_capture|turn_parts|chat_composer|web_attachments|web_tool_chooser`
  are `pub mod` lines only; no `TurnParts/push`, `turn_contract`,
  `Selection::select`, `lower_composer_doc`-with-chips, or attachment-send
  call exists in `lib.rs` (holds from WEB-007-012-ACCEPT).
- Upstream pins: opencode `95daf90670b7c039c436c85537da5fbfe2205b41`,
  9router `17c4cc76877bd1755030a8414f8d0083f48dcccf`.
- Prior lane receipts reused (no re-fabrication): RED-VALIDITY-WEB-PROV
  (WEB-013..017 + PROV-015/016 stub-bite), RED-VALIDITY-WEB013-015
  (cap/workspace flag/key-drop bites), WEB-007-012-ACCEPT (6×5 GREEN),
  WEB-014-017-SUITES (5-test T01-T05 suites exist + supplemental 2-test
  HTTP boundaries), WEB-013/014/015/016/017 single worklogs (GREEN-only).

## Observed scenario — GREEN verify (this lane, serial JOBS=1 THREADS=1)

Log: `/tmp/opencode/xK-web.log`. Counts = tests passed, 0 failed each:

- `control_plane_inputs` 5, `control_plane_errors` 5, `protocol_api` 5.
- `control_plane_exposure` 5, `event_stream` 5.
- `transcript_lane` 5, `turn_parts` 5, `chat_composer` 5.
- `web_attachments` 5, `web_tool_chooser` 5, `voice_capture` 5.
- `session_history_api` 2, `web_artifact_api` 2, `web_capabilities_api` 1,
  `web_workspace_api` 2.
- `web_artifact` 5, `web_entry_probe` 5, `web_route` 5.
- sessions `chat_nav_lane` 5, `web_008_lane` 5, `web_013` 5,
  `project_context` 5.
- Total server+session suites in log: 22 targets, all GREEN, 0 failures.
- No E0432 encountered; `lib.rs` read-only rule never triggered (no edit
  needed; report: NOT blocked).

## RED probe — 2 weakest surfaces (gap REDs, compile+fail expected, no product edit)

Full prior stub-bite receipts exist (see RED-VALIDITY worklogs); this lane
adds 2 weakest-point probes with 2 strategies each. Transient `xk_weak_*`
test files were created, hit a pre-existing dirty-tree compile break in
sibling-owned `crates/providers/src/share_descriptor.rs` (unbalanced
delimiter from another lane — NOT mine, NOT fixed per ownership lock), then
deleted (`crates/server/tests/xk_weak_*` → no matches; leftover count 0).
Probes therefore recorded as log-file gap REDs (assertions that fail against
current code by inspection + suite evidence), NOT as executed cargo REDs.
Strategies + logs: `/tmp/opencode/xK-weak-*.log`.

### Weakest 1: voice live-transcript path (WEB-016 — weakest: hardware happy path untestable)

- Log: `/tmp/opencode/xK-weak-voice-transcript.log` (uname + 0-hit audio-IO
  grep + capability block + 2 strategies).
- Strategy A (bytes→audio impossible): feed real audio bytes through
  `push_utterance(id, text: &str)` — rejected by type (takes `&str`, no
  audio input type exists); even text is refused unless
  grant=Granted + adapter=true + state=Capturing, else
  PermissionDenied/NoDevice/NoAdapter/NotCapturing/Muted with zero
  transcript side effects. Gap RED: any test asserting a transcript from
  byte input FAILS today (no adapter, no STT).
- Strategy B (STT of retained transcript impossible): retained segments are
  `String`s; no speech-to-text function exists in module (no cpal/rodio/
  webrtc/process imports, 0 hits); `transcript()` joins strings only.
  Gap RED: test asserting `transcript()` derives from audio FAILS today.
- Holds: `voice.available:false`, no mic request path, no fake transcript,
  `live_tracks()` counter only, text chat usable in every state.

### Weakest 2: search/research executor (WEB-013 — weakest: reason-string precision + no executor)

- Log: `/tmp/opencode/xK-weak-search-executor.log` (uname + capability
  block + browser grep 0-hits + 2 strategies).
- Strategy A (reason-text-only stub doesn't bite): change search reason
  `"local grep is not a web-search adapter"` → `"x"` → cap suite still
  PASSES (asserts only `available==false`). Gap RED: test pinning reason
  precision FAILS today (documented in RED-VALIDITY-WEB013-015 attempt 2).
- Strategy B (happy-path executor absent): test asserting
  `search.available==true` or a research plan/progress/citation flow FAILS
  today (no executor/plan-persistence/citation adapter per card; grep/text
  turns carry no research fields). Positive RED-bite twin (flag flip
  false→true) DOES fail the suite (attempt 1 receipt), proving the
  unavailable boundary pins while the executor gap stays open.
- Holds: composer must surface disabled entries, never relabel grep/text
  turns; `web/src/lib/api.ts` + `composer.tsx` browser media/search hits 0.

## Target boundary

- Owned: this worklog + `/tmp/opencode/xK-web.log` +
  `/tmp/opencode/xK-weak-voice-transcript.log` +
  `/tmp/opencode/xK-weak-search-executor.log` only.
- Explicitly NOT owned/touched: `crates/server/src/lib.rs` (read-only),
  any `crates/server/src/*` product file, any frozen test, `ralph.json`,
  `web/` browser code, sibling dirty-tree files (incl. broken
  `share_descriptor.rs` — left for owning lane; blocked compile of new
  server test targets, hence log-file probes).
- Write-paths kept disabled; mic/audio never enabled; no DB writes; no
  network; no secrets logged.

## Tests

- None frozen by this lane. Frozen suites verified GREEN (counts above);
  prior RED receipts cited, not re-fabricated.
- RED probes: 2 weakest × 2 strategies, logs above. No new frozen suite
  proposed (ownership: future lanes own executor/adapter contracts).

## Decisions

- `ponytail:` unavailable states stay disabled, never faked (all 17);
  executor/adapter/membership work deferred to integration proposals.
- Dirty-tree `share_descriptor.rs` break reported, not fixed (server-writer
  owns `lib.rs`; providers file owned elsewhere; lane rule: report blocked,
  do not fix — compile of pre-existing suites used cached artifacts and
  stayed GREEN).

## Remaining unknowns

- Browser `vitest`/`tsc` suite unexecuted (no policy path in worktree).
- WEB-009 producer + citation contract; WEB-010 HTTP chip/queue contracts;
  WEB-011 unified store + provider adapter; WEB-012 execution-owned
  approvals; WEB-013 executor; WEB-015 membership/memory authority;
  WEB-016 native adapter + mic-grant flow; WEB-017 run/apply bridge.
- Frozen T01..T05 RED/GREEN ownership stays with owning lanes; verifier
  decides acceptance. All 17 stories stay NOT ACCEPTED here.

## Return counts

- GREEN suites verified: 22 targets (18 server + 4 sessions), 0 failures.
- Tests passed: server 14×5=70 (inputs/errors/protocol/exposure/event/
  transcript/turn/composer/attachments/chooser/voice/artifact/entry/route)
  + 2+2+1+2 (history/artifact/cap/workspace HTTP) = 77; sessions 5×4 = 20;
  combined 97 passed, 0 failed.
- RED probes: 2 weakest (voice-transcript, search-executor) × 2 strategies
  = 4 gap-RED assertions, logs `/tmp/opencode/xK-weak-*.log`.
- Files written: `worklog/WEB-CLOSE3.md` only (+3 log files). E0432: none.
  `ralph.json`: untouched. Blocked fix: 1 reported
  (`share_descriptor.rs` sibling break), NOT fixed.
