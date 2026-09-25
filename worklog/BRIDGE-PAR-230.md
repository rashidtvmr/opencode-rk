# BRIDGE-PAR-230 scratchpad

Claim: BRIDGE-PAR-230 via cc.claim, session ses_par230. OK.
Source: crates/opentui-bridge/src/event_ctx.rs:19-21 EventCtx subs Vec cap 64; message_router.rs:31 MessageRoute pattern. Read only, not edited.
Boundary: ONE new file event_bus_full.rs. No lib.rs/Cargo.toml edits. No cargo/commit per scope.
Tests: 6 unit tests inline (sub_emit_hit, emit_miss_zero_but_logs, dup_empty_false, sub_cap_32, entry_truncates_256, log_cap_64_and_tail).
Decisions: exact-match subs (filter count, mirrors EventCtx::emit); emit always logs even on 0 matches; log eviction remove(0) oldest-first; entry format topic:body truncated 256 chars; log_tail oldest-first slice.
Evidence: rustfmt --check PASS, wc -l 116 (<130).
Unknowns: none. Module wiring into lib.rs left to orchestrator (out of scope).
