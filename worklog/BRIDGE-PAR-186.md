# BRIDGE-PAR-186 scratchpad

Claim: BRIDGE-PAR-186, session ses_par186, owned file crates/opentui-bridge/src/stream_drain.rs.
Source evidence:
- crates/opentui-bridge/src/stream_transport_full.rs:11 StreamTransportFull, :42 recv()->Option<Frame>, :7 seq/body, :49 pending
- crates/opentui-bridge/src/sdk_stream.rs:17 SdkStream, :27 push(SdkEvent)->bool (false closed/full), :10 SdkEvent::Text(String), :43 drain_text, :39 close
Target boundary: drain_frames(tx,max)->Vec<String> recv up to max cap 64 + feed_sdk(sdk,bodies) push Text each stop on false. std-only, forbid(unsafe_code), <110 lines, >=4 tests. No lib.rs/Cargo.toml edits, no cargo.
Tests: empty_drain, respects_max, caps_64, feed_roundtrip, feed_stops_closed (5 in-file).
Decisions: max.min(64) bound; for/break loops; clone per push (SdkStream takes owned).
Unknowns: none. Verify: rustfmt --check only per scope.
