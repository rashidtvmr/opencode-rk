# APP-004-retry (turn_service lane)

HEAD 5af7884.

## Claim
`crates/server/src/turn_service.rs` implements coding-turn state machine: TurnId,
TurnPhase Streaming/AwaitingApproval/Executing/Settled/Cancelled/Uncertain,
classify_retry safe-vs-ambiguous, CancelToken, DenyReceipt. No other files touched.

## Source evidence
- `crates/agents/src/turn_state.rs:62-128` — TurnSubmissionState Idle/Submitted/Running,
  no approval/interrupt/retry/recovery. New file extends lifecycle server-side.
- `crates/server/src/lib.rs:688-711` — create_turn appends messages, no approval,
  interrupt, retry classification. Turn owns that missing state.
- APP-004 card (`python3 tools/completion_plan.py --card APP-004`): journey
  submit/stream/permission-edit/continue; T03 interrupt stable cancelled, T04 no
  replay of ambiguous effects, T05 restart truthful uncertain, T02 deny unchanged.
- docs/TDD.md: compiling RED first, frozen tests; docs/SECURITY.md: deny asserts
  absence of side effects, scoped cancellation.

## Observed scenario
First `rustc --test` run: 6 passed, 1 failed — off-by-one in test event-log fill
(start records 1 event; loop over MAX_TURN_EVENTS overflowed one early).
Implementation untouched; fixed test loop bound to MAX_TURN_EVENTS-1.

## Target boundary
Owned file only: crates/server/src/turn_service.rs (created, 25533 bytes).
std only: std::fmt, std::sync atomic/Arc. No serde/tokio. #![forbid(unsafe_code)].
Bounds: MAX_TURN_EVENTS=64, MAX_TEXT_BYTES=8KiB. No threads/I/O.

## Tests (frozen in-file #[cfg(test)], 7 tests)
happy_path / deny_receipt / interrupt_cancel / safe-vs-ambiguous retry /
restart_uncertain / invalid+bounds / classifier exhaustive.
Verify: `rustc --edition 2021 --test crates/server/src/turn_service.rs -o /tmp/opencode/ts && /tmp/opencode/ts`
Result: 7 passed, 0 failed.

## Decisions
- Deny settles with receipt, possible_effect=false (provably no execution).
- Ambiguous retry moves to Uncertain AND returns AmbiguousRetryDenied error.
- Retry during AwaitingApproval rejected (would bypass human grant).
- Terminal phases reject retry/settle/interrupt/mark_uncertain.

## Remaining unknowns
- Wiring into lib.rs router/service + agents turn_state: integrator-owned, not this lane.
- Gate `python3 tools/lane_gate.py` owned by orchestrator.
