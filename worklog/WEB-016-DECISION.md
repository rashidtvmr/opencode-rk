# WEB-016 wiring decision

## Claim
Wire `voice_capture.rs` as read-only state machine. Keep daemon capability `voice.available=false`.

## Source evidence
- `crates/server/src/voice_capture.rs:1-366` — pure state machine. No `Command::new`, `std::process`, `cpal`, `rodio`, `webrtc`, `getUserMedia`, `MediaRecorder`. Only `String`/`Vec` heap use. No I/O, clock, net, threads.
- `crates/server/src/lib.rs:21` — new: `pub mod voice_capture;` (additive, one line). No route/handler/touch to turns, capabilities, or web assets.
- `crates/server/src/lib.rs:164-167` — capability unchanged: `available:false`, reason "no native transcription or realtime audio adapter is installed".
- `tasks/WEB-016.md:31-36` — boundary already declares unavailable; composer keeps text chat, never requests mic while unavailable. Story NOT ACCEPTED.
- `docs/SECURITY.md:1,9-22,62-72` — mic needs permission broker + explicit human grant. Server `PermissionBroker` (`crates/security/src/lib.rs:151-251`) has `OperationIntent::File/Process/Sql/Tool` only — no mic/audio intent. No grant issuer exists. `*` cannot bypass human-only grant.
- `crates/server/Cargo.toml:9-23` — no audio deps (cpal/rodio/webrtc absent). Adding one violates lazy/minimal rule.

## Decision: wire read-only, NOT live
- `start()` (`voice_capture.rs:230-243`), `push_utterance` (`284-312`), `unmute` (`255-269`) gate on `MicGrant` + `adapter` bool. With `adapter=false` (only value server can honestly supply — no native adapter installed) every capture path returns `VoiceError::NoAdapter` with zero transcript side effects. Denied grant returns `PermissionDenied`, empty transcript (frozen T02 asserts this).
- No mic activation possible: module holds no device handle, no process spawn, no audio I/O; `live_tracks()` is a counter derived from enum state, max 1, no FDs.
- Capability-denied semantics already satisfied by existing `available:false` manifest + `VoiceError::NoAdapter`; no new error type needed.
- Full wiring (routes, `getUserMedia` bridge, turn integration) rejected: would invent broker authority and request mic without human grant. Violates SECURITY.md §1/§5 and AGENTS.md human-only grant rule.

## Diff
- `crates/server/src/lib.rs`: +1 line `pub mod voice_capture;` (alphabetical slot). Nothing else touched.

## Evidence
- No mic/process spawn: `grep -E 'Command::new|std::process::Command|tokio::process|cpal|rodio|webrtc|getUserMedia|MediaRecorder|alsa|pulse' crates/server/src/voice_capture.rs` → no matches (exit 1).
- `cargo check -p opencode-rk-server` → 0 errors, 10 pre-existing warnings.
- `cargo test -p opencode-rk-server --test voice_capture` → 5 passed (T01..T05 frozen suite), 0.00s. Run with `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2`, timeout 120, via `rtk`.

## Remaining unknowns / blockers
- Real native audio/realtime adapter dependency (per task card) still absent — T01 happy path exercises state machine only, not hardware.
- Mic `OperationIntent` + human-grant flow undefined in security crate; needs integration proposal, not this lane.
- Story stays NOT ACCEPTED per task card; verifier decides.
