# LANE-TURN-SETTLE — scratchpad

Claim: LANE-TURN-SETTLE, session ses_f423cceb6ffeLkm86YxBA2Z5YW.
Owned file: crates/server/src/turn_service.rs only. lib.rs untouched. No test edits to frozen tests.

## Source evidence
- Repo rev 6b19524 (checked `git rev-parse HEAD`).
- `crates/server/src/turn_service.rs:361-371` `tool_finished`: Executing -> Streaming, clears possible_effect.
- `crates/server/src/turn_service.rs:376-388` `settle`: Streaming -> Settled; Executing only if !possible_effect.
- `crates/server/src/lib.rs:51` `pub mod turn_service;` — module wired, but grep shows zero callers of `Turn::start` / `tool_finished` / `settle` outside turn_service.rs tests. `create_turn` (lib.rs:744) and `create_turn_stream` (lib.rs:877) persist messages without driving a `Turn`.
- `runtime_wiring.rs:47` imports only `turn_service::CancelToken`.

## Observed scenario
- After `approve`, possible_effect=true. `settle` from Executing then fails InvalidTransition unless `tool_finished` ran first. A caller that forgets the two-step leaves the turn in Executing forever: non-terminal, never Settled = infinite pending.
- Bounds forbid touching lib.rs, so the fix lands inside the owned file: an atomic `finish_and_settle` (tool_finished then settle) giving the turn path one call that reaches Settled. Guards preserved: wrong-phase and ambiguous-effect behavior unchanged (tool_finished still requires Executing; settle still requires clean).

## Target boundary
- Add `Turn::finish_and_settle` + 2 new tests (happy path, wrong-phase rejection). Zero changes to existing 7 tests.

## Tests
- RED: new tests fail (no method) — error[E0599] on both tests pre-impl.
- GREEN: `rustc --edition 2021 --test crates/server/src/turn_service.rs -o /tmp/opencode/turn_service_test && /tmp/opencode/turn_service_test` → 9 passed, 0 failed.
- NOTE: `cargo test -p opencode-rk-server --lib turn_service` blocked by pre-existing `crates/tools/src/shell_tool.rs` breakage (PermissionBroker/Decision/ShellError::Denied unresolved — another lane's WIP in working tree, untouched by this lane). Standalone harness compiles the owned file only and runs all 9 tests green.
- sha256 b1996c1b956389c1216a04da85cad2acc7db3067084638a9179e2a87ed818d14; diff is pure addition (grep `^-` empty).

## Decisions
- Two event-log slots consumed (tool_finished + settle records); EventLogFull propagates with turn left clean in Streaming. Documented on method.
- No idempotency: second call fails InvalidTransition from Settled, same as settle.

## Remaining unknowns
- None for this lane. lib.rs wiring of Turn into create_turn_stream stays integrator-owned (out of bounds).
