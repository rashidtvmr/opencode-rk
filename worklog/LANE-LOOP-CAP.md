# LANE-LOOP-CAP scratchpad

Claim: LANE-LOOP-CAP, session ses_f423cceb3ffe8DP2G3ZFzAOT5p.
Owned file: crates/server/src/agent_loop.rs only. lib.rs forbidden.

## Source evidence
- crates/server/src/agent_loop.rs:152-157 `end_round` returns `RoundEnd::ToolRound(calls)` unbounded; doc claims truncation but code does not truncate.
- crates/server/src/agent_loop.rs:159-174 `truncate_calls` enforces MAX_CALLS_PER_ROUND=16 with overflow CallOutputs.
- crates/server/src/lib.rs:1031-1075 pushes every FunctionCall into pending_calls unbounded; :1149-1159 truncates only at Executing stage via truncate_calls.
- Rev 6b19524.

## Observed scenario
end_round never called from lib.rs; per-call accumulation unbounded until Completed event. Fix must live in agent_loop.rs helpers so future wiring bounds pending dispatch batch at classification time.

## Target boundary
- end_round truncates ToolRound batch to MAX_CALLS_PER_ROUND in place.
- Doc points at truncate_calls for overflow notices.
- No signature change (frozen T02 calls end_round). No lib.rs touch. No test edits.

## Tests
- Frozen: cargo test -p opencode-rk-server --lib agent_loop (T01-T06).
- T02 (1 call) unaffected; no frozen test passes >16 calls to end_round.

## Decisions
- `calls.truncate(MAX_CALLS_PER_ROUND)` minimal; reuses existing const. Overflow surfacing stays in truncate_calls (Executing stage already uses it).

## Remaining unknowns
- None for this lane. lib.rs per-event push cap is follow-up lane work (needs lib.rs ownership).
