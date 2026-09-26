# WEB-009 runtime events RED

Status: blocked (compiling RED; runtime event publication absent).
Task/session: WEB-009 / ses_f3756bcfcffekwqqbw0PsgFBHh.
Base: 81fba08, branch lane/WEB-006-integration.

## Evidence
- `crates/server/src/runtime_wiring.rs:317-369`, `RuntimeWiring::for_daemon` and `events()` compose/expose daemon EventBus.
- `crates/server/src/event_bus.rs:11-18`, `ServerEvent` durable event variants; `MessageAppended` has `seq`, currently test uses required `0`.
- `crates/server/src/lib.rs:129-130`, `AppState` remains two-field sessions/catalog; `runtime_wiring.rs` is injected through `Extension`.
- `crates/server/tests/runtime_wiring_policy_http.rs:49-68`, disposable storage, explicit runtime, deny-default policy, router extension fixture.
- `crates/server/tests/runtime_wiring_policy_http.rs:169-190`, bounded scripted two-round provider fixture.

## Contract
Create session before subscribing. HTTP turn must execute fixed harmless `echo runtime-events-fixture`, persist User/Tool/Assistant rows, return bounded NDJSON/tool transcript, and complete two provider rounds. After durable writes, runtime subscription must receive exactly, in order: `MessageAppended(session, seq=0)` user; `ToolExecuted(name=bash, duration_ms=0)`; `MessageAppended(session, seq=0)` tool; `MessageAppended(session, seq=0)` assistant. No fifth event. No event before corresponding persisted row. No-runtime legacy path unchanged. Bounds: provider request 256 KiB, response body 64 KiB, event receive 100 ms each, fifth-event probe 25 ms, one child command only, loopback only, fake key.

## RED
Command:
`env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test runtime_wiring_events_http -- --test-threads=1`

Result: compiled; 1 test failed as expected. Failure is semantic event absence, not fixture/policy compile failure:
`crates/server/tests/runtime_wiring_events_http.rs:350:14: runtime event timeout: Elapsed(())`
Body/tool/provider/message assertions ran successfully before first event receive. Existing HTTP response/tool transcript behavior succeeded; first runtime event receive timed out.

Test SHA-256 (after RED, committed bytes): `a4496430f11fd1cb02e8bc728bc8d1f0abcecd81c233ca895a505c2c00bf5223`.

## Implementation proposal
Carry optional cloned runtime `EventBus` in `TurnStreamState`; publish user only after append succeeds; publish `ToolExecuted` after each dispatch outcome (including deny); publish tool `MessageAppended` only after append succeeds; publish assistant `MessageAppended` only after append succeeds. Legacy no-runtime path publishes nothing. Do not use process-wide `CancelState`; cancellation remains separate.
