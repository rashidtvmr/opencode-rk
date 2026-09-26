# BRIDGE-GAP-67 scratchpad

Claim: BRIDGE-GAP-67, session ses_gap67.
Source: TS `packages/opencode/src/cli/cmd/run/stream.transport.ts` (global event sub, single-turn Wait, buffered replay); Rust naming from `crates/opentui-bridge/src/run_stream.rs` (StreamBuf/push/commit, fail-closed oversize).
Target: `crates/opentui-bridge/src/run_stream_transport.rs` only. No lib.rs/Cargo.toml/run_stream.rs edits. No cargo. No commit/push.
Tests: send seq, overlong errs, ack removes, ack unknown false, cap 128 (+ wrap).
Decisions: std-only, forbid(unsafe_code), Frame{seq,body}, Transport{next_seq,pending}, send(&str)->Result<u32,String> wrapping_add, ack->bool via position/remove, pending_len.
Unknowns: none.
