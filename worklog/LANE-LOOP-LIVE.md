# LANE-LOOP-LIVE — LoopDriver live-turn integration tests

## Claim
- Task: LANE-LOOP-LIVE
- Session: ses_worker_loop_live (pre-claimed)
- Owned file: crates/server/tests/loop_driver_live.rs (NEW)
- Status: completed

## Wiring gap (exact evidence)

The LoopDriver state machine (`loop_driver.rs`) is wired into `lib.rs:28`
as `pub mod loop_driver` but is **never instantiated** in the live turn path.

| Component | File:line | Mechanism |
|-----------|-----------|-----------|
| LoopDriver | loop_driver.rs:375 | Pure state machine, 256-step budget |
| Live turn handler | lib.rs:871 (`create_turn_stream`) | Uses `agent_loop::LoopController` |
| LoopController init | lib.rs:968 | `LoopController::with_cap(max_steps)` |
| agent_loop cap | agent_loop.rs:23 | `MAX_TURN_STEPS = 64` |
| loop_driver cap | loop_driver.rs:26 | `MAX_LOOP_STEPS = 256` |

These are two independent mechanisms. The turn stream never creates a
`LoopDriver`, never feeds `LoopEvent`s, and never emits LoopDriver
trajectory events (`plan_started`, `step_outcome`, `goal_completed`,
`steer_applied`, `checkpoint`).

**Cannot be closed from a test-only file**: wiring LoopDriver into the
turn path requires editing `lib.rs` (the turn stream handler), which is
outside this lane's authority.

## RED / GREEN

- RED sha256: `7ca6a1c1fb1f2fb6bbe75fb0eb4e117fb78da6869ce45b22f0f451915dc599e6`
  (initial file; compile errors from private fields `DriverState::new()` and
  `LoopDriver.state.steps_taken`)
- GREEN sha256: `a60cf68c0ddc535cc8687e18423eeb327e117ad03964189cbe105645a5a837ac`
  (after fixing to use public API only)
- Zero test edits post-freeze

## Test commands + results

```
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 300 rtk cargo test -p opencode-rk-server --test loop_driver_live
# result: 16 passed (1 suite), 0 failed, 0.03s
```

Full suite regression check:
```
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 600 rtk cargo test -p opencode-rk-server
# 2 pre-existing failures in session_turn_stream_api (not caused by this lane)
# All other tests pass, including loop_driver_live 16/16
```

## Scenarios covered

| Scenario | Test | What it proves |
|----------|------|----------------|
| A (drive) | `scenario_a_drive_multi_goal_trajectory` | Plan → step Done → goal completion → Stop{Achieved} |
| A (budget) | `scenario_a_drive_budget_exhausted` | Blocked goal → Replan once → Failed → Achieved |
| A (cap) | `scenario_a_drive_step_budget_exhaustion` | steps_taken=255 → Tick → Stop{Exhausted} |
| B (resume) | `scenario_b_checkpoint_resume_roundtrip` | checkpoint→resume restores exact state |
| B (foreign) | `scenario_b_checkpoint_resume_rejects_foreign_version` | version mismatch → Stop{Exhausted} |
| B (skip) | `scenario_b_resume_skips_completed_goals` | Achieved goals stay achieved after resume |
| C (steer) | `scenario_c_steer_amendment_reflected` | Steer appends amendment |
| C (truncate) | `scenario_c_steer_truncates_long_amendment` | >256 byte amendment truncated |
| C (determinism) | `scenario_c_steer_preserves_determinism` | Identical events → identical state |
| D (gap) | `scenario_d_live_turn_no_loop_driver_events` | Turn stream has no LoopDriver events |
| D (proof) | `scenario_d_turn_uses_loop_controller_not_loop_driver` | Different caps prove separate mechanisms |
| Contract | 5 additional tests | Empty plan, checkpoint determinism, per-goal cap, NeedsReplan |

## Decisions

1. **Honest GREEN**: Tests assert the current verifiable contract rather than
   fabricating integration that doesn't exist. Scenario D explicitly proves
   the wiring gap.
2. **No private field access**: Fixed RED compile errors by constructing
   checkpoint blobs manually instead of using private `DriverState::new()`
   or private field assignment.
3. **Pre-existing failures**: 2 `session_turn_stream_api` failures are
   pre-existing (verified via stash test), not caused by this lane.

## Unknowns

- Whether the orchestrator will wire LoopDriver into `create_turn_stream`
  in a follow-up lane. The wiring gap is documented with exact file:line
  evidence for that purpose.
