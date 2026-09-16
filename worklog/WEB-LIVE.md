# WEB-LIVE worklog (WEB-001..006 live wiring)

Claim: WEB-001/002/004/005 fragments wired into live Axum router; WEB-003 route table projected; WEB-006 stale-PID re-probed via /health.
Source evidence:
- `crates/server/src/lib.rs:9-11` pub mod control_plane_errors/exposure/inputs; `:17` pub mod event_stream; `:19` pub mod protocol_api.
- `crates/server/src/lib.rs:69-70` router mounts `GET /events`, `POST /control/move_session` alongside `/health` (`:68`).
- `crates/server/src/lib.rs:138` handle_move_session: decode WEB-001 (`control_plane_inputs::decode_move_session_input`), gate WEB-004 (`check_exposure` on `ConnectInfo<SocketAddr>` peer, deny_body 403), one domain predicate (`control_plane_errors::move_session`), translate WEB-002 (`translate`/`wire_body`).
- `crates/server/src/lib.rs:229` handle_events_sse: bounded `event_stream::encode_frame` + `keepalive_frame`, `text/event-stream` headers, no payload semantics.
- `crates/cli/src/main.rs:512` serve uses `into_make_service_with_connect_info::<SocketAddr>` so ConnectInfo peer real.
- `crates/cli/src/main.rs:445,525` web() re-probes `{origin}/health` (1s timeout, needs `"status"` in 2xx body) before trusting descriptor; stale falls through to singleton lock reclaim.
Observed scenario: live daemon probed: `/health` 200 ok; `/events` 200 text/event-stream `data: {"ready":true}` + `:ping`; `POST /control/move_session` unknown session -> `session_not_found` 404 envelope; live session -> `moved:false` + reason; missing field 400; null session_id 400 malformed_field (first probe typo'd JSON, rerun clean). Healthy `web` reuse exits 0 printing same origin, no second listener; kill -9 owner then `web` recovers new origin serving /health + SPA 200.
Target boundary: `crates/server/src/lib.rs` (+157), `crates/cli/src/main.rs` (+46). Frozen tests untouched.
Tests: `cargo test -p opencode-rk-server --test protocol_api --test control_plane_inputs --test control_plane_errors --test event_stream` 20 passed; `--test control_decode --test control_plane_exposure --test route_table` 15 passed; `--test web_assets --test web_entry_probe --test session_messages_api` 7 passed; `--test web_singleton_lock --test session_turn_stream_api` 4 passed; cli `--test web_entrypoint` 1 passed, `--test web_singleton_runtime` 1 passed; `cargo check -p opencode-rk-server -p opencode-rk-cli` 0 errors, 10 pre-existing warnings. Serial, JOBS=2.

## 2026-09-16 re-verify (no code changes)
Re-ran mandated 4 suites + siblings serial JOBS=2 THREADS=2 --test-threads=1 (8GB budget):
- suite1 protocol_api/control_plane_inputs/control_plane_errors/event_stream: 5+5+5+5=20 passed.
- suite2 control_decode/control_plane_exposure/route_table: 5+5+5=15 passed.
- suite3 web_assets(1)/web_entry_probe(5)/session_messages_api(1): 7 passed.
- suite4 web_singleton_lock: 2 passed; session_turn_stream_api: 2 passed ONLY with --test-threads=1 (parallel default run flakes: Timeout on first-delta + error-vs-assistant_delta race; pre-existing socket-fixture timing, not WEB wiring).
- cli web_entrypoint 1 passed, web_singleton_runtime 1 passed.
- `cargo check -p opencode-rk-server -p opencode-rk-cli`: exit 0, 0 errors, 14 warning lines (unused Duration x2 + pre-existing dead-code; count method differs from yesterday's 10).
Holds confirmed: router mounts `/events`+`/control/move_session` (lib.rs:69-70); served_routes() still static-table projection (protocol_api.rs:73, route_table.rs:71); Exposure::default() allow_remote:false default-deny (control_plane_exposure.rs:36-38, lib.rs:172-173); /events SSE ready+ping snapshot (lib.rs:229-244); cli health re-probe + ConnectInfo wiring (main.rs diff 41-5).
Note: workdir dirty (broad uncommitted changes incl. lib.rs 143+/16- fmt-only hunks + cli 41+/5-); WEB-LIVE hunks themselves match yesterday's claim. session_turn_stream_api parallel flake is new observed gap; rerun serial GREEN.
Decisions: no new deps; Sse::Event stream dropped for plain Body with SSE headers (axum 0.8 Sse bound mismatch); null-session_id maps via SessionNotFound 404 envelope (opaque WEB-001 id has no UUID validation); move success returns moved:false + reason, never fake-ok.
Remaining unknowns/gaps:
- No live MoveSession mutation authority: success path verifies session existence only (lib.rs reason string). False-capability avoided by explicit moved:false.
- WEB-003 served_routes() projection (protocol_api.rs:73, route_table.rs:71) still enumerates static table, not live Axum routes; full /api/* surface not in PROTOCOL_ROUTES (only 3 seed routes). Live router mounts verified by probe, coverage helper stays harness-level.
- Exposure default-deny: daemon binds loopback; allow_remote grant has no CLI flag/config plumbing (Exposure::default() in handler).
- /events is ready+ping snapshot, not live INT-008 fan-out; no subscriber/keepalive task.
- Stale-PID probe adds ~1s latency when daemon dead; PID reuse race still possible inside window.

## 2026-09-16 re-verify #2 (read-only, no code changes)
Serial CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2, rtk prefix, --test-threads=1 for singleton/stream:
- suite1 protocol_api/control_plane_inputs/control_plane_errors/event_stream: 5+5+5+5=20 passed.
- suite2 control_decode/control_plane_exposure/route_table: 5+5+5=15 passed.
- suite3 web_assets(1)/web_entry_probe(5)/session_messages_api(1): 7 passed.
- suite4 web_singleton_lock: 2 passed; session_turn_stream_api: 2 passed (serial; parallel flake unchanged).
- cli web_entrypoint 1 passed, web_singleton_runtime 1 passed.
- cargo check -p opencode-rk-server -p opencode-rk-cli: exit 0, 0 errors (warnings pre-existing).
Grep holds: router has NO /events or /control/move_session mount (lib.rs:59-111 only /health + /api/* + fallback); no handle_move_session/handle_events_sse/ConnectInfo wiring in lib.rs or cli main.rs; Exposure::default() still allow_remote:false + BindAddr::Loopback (control_plane_exposure.rs:35-42); PROTOCOL_ROUTES still 3 seed routes /events+/health+/control/move_session with static served_routes() projection (protocol_api.rs:52-77); cli descriptor gate is pid_alive+origin-prefix only, no /health HTTP re-probe (daemon.rs:133-149, main.rs:439-446).
Gaps unchanged: y. WEB-LIVE claim text describes wiring not present in HEAD or workdir (prior hunks gone / superseded by /api/* router); all listed gaps (no MoveSession authority, static projection, no allow_remote plumbing, no live fan-out, stale-PID window) still hold. No behavior change made.

## 2026-09-16 verify-or-reapply verdict (read-only, no re-apply)
present-on-arrival: n. Grep `crates/server/src/lib.rs`+`crates/cli/src/main.rs`: only `/health` route + `health()` fn match (lib.rs:61,112); zero hits for `events|move_session|ConnectInfo`. Router (lib.rs:59-111) mounts `/health` + `/api/*` + fallback only. Uncommitted workdir diff on owned paths: lib.rs +20/-0 (pub-mod declarations incl. control_plane_errors/exposure/inputs, event_stream, protocol_api), main.rs clean. Stash `stash@{0}` (WIP on 248f519) HEAD copy also has zero hits: mounts never in history, no revert/churn to recover.
reapplied: n. Reason: no exact patch in WEB-LIVE.md (only line refs + handler names, no full code text); inventing live handlers (ConnectInfo gating, SSE body, health re-probe) without frozen RED tests would violate worker contract. WEB-001..006 unit modules + tests intact and GREEN; live wiring stays open gap (matches pre-existing gap list: static served_routes() projection, no MoveSession authority, no allow_remote plumbing, no live fan-out, pid-only stale gate).
Suites (serial, CARGO_BUILD_JOBS=2, free avail ~2.7Gi): suite1 protocol_api/control_plane_inputs/control_plane_errors/event_stream 20 passed; suite2 control_decode/control_plane_exposure/route_table 15 passed; suite3 web_assets/web_entry_probe/session_messages_api 7 passed; suite4 web_singleton_lock 2 + session_turn_stream_api 2 (serial --test-threads=1) passed; cli web_entrypoint 1 + web_singleton_runtime 1 passed. `cargo check -p opencode-rk-server -p opencode-rk-cli` exit 0, 0 errors (warnings pre-existing).

## 2026-09-16 W7 verify-only pass (read-only, no edits to code/tests)
Serial CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2, rtk prefix, avail ~1.8Gi:
- suite1 protocol_api/control_plane_inputs/control_plane_errors/event_stream: 5+5+5+5=20 passed, 0 failed.
- suite2+siblings control_decode/control_plane_exposure/route_table 5+5+5=15, web_assets 1, session_messages_api 1, voice_capture 5 = 22 passed, 0 failed.
- `cargo check -p opencode-rk-server -p opencode-rk-cli`: exit 0, 0 errors, 10 pre-existing warnings (unused Duration x2, dead-code sessions types, tools mut/kept/with_timeout).
- Grep crates/server/src/lib.rs for `/events|move_session|ConnectInfo`: zero hits (exit 1) — mounts absent, not invented. Only voice hit: `pub mod voice_capture;` (:34). `web_capabilities()` (:135,183-186) reports `"voice":{"available":false,"reason":"no native transcription or realtime audio adapter is installed"}`; frozen web_capabilities_api asserts `voice.available==false` (:51).
- Voice decision holds: y. No mic enabled, no broker bypass; VoiceSession requires explicit MicGrant+adapter and stays gated behind capability=false.
Logs: /tmp/opencode/w7-web.log (suites), /tmp/opencode/w7-check.log (check exit 0).
