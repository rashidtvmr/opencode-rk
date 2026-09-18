# LANE-LOOP — Task scratchpad

## Claim
- Task: LANE-LOOP
- Session: ses_worker_loop
- Owned file: crates/server/src/loop_driver.rs (NEW)
- Goal: Roadmap 3.1 LOOP — task-level agentic driver state machine

## Source evidence
- lib.rs:6-13 — agent_loop exports MAX_TURN_STEPS, LoopController, TurnStop
- context_report.rs — std-only standalone module pattern with in-file tests
- admission_bounds.rs — forbid(unsafe_code), bounded collections pattern

## Target boundary
- Pure-state, std-only, forbid(unsafe_code)
- No IO, no threads, no provider calls
- Deterministic: same event sequence → same state
- MAX_LOOP_STEPS=256, per-goal attempt cap 8
- Checkpoint/resume via deterministic serialization

## Tests (frozen)
- T01: start→tick Continue with first goal InProgress ✓
- T02: step Done advances to next goal ✓
- T03: all goals Achieved → Stop{Achieved} ✓
- T04: blocked goal triggers Replan once, then Failed after cap ✓
- T05: steer appends amendment, preserves determinism ✓
- T06: checkpoint→resume roundtrip restores exact state ✓
- T07: resume rejects foreign version ✓
- T08: max_steps cap stops with Exhausted ✓
- T09: per-goal attempt cap 8 → Failed goal skipped ✓
- T10: empty plan → immediate Stop ✓

## Verification
- Command: `rustc --edition 2021 --test src/loop_driver.rs`
- Result: 10/10 passed, 0 failed, x2 runs

## Decisions
- Description stored as Vec<u8> with bounds check (<=256 bytes)
- Goal id is u32 for simplicity
- CheckpointBlob is Vec<u8> (caller interprets)
- Amends stored as VecDeque<String> (UTF-8 enforced)
- State machine drives goals sequentially (first InProgress goal)
- Steer truncates notes exceeding MAX_DESCRIPTION_BYTES

## Remaining unknowns
- Integrator must add `pub mod loop_driver` to server/lib.rs
