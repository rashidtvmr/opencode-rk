# APP-003 runtime composition worklog

Task: APP-003 single-engine composition (`crates/server/src/app_runtime.rs`).
HEAD: `5af7884cf7637c0760d985da7a03f2f99ccd0c78`.

## Claim
New file owns composition only: `EngineHandles` (provider/sessions/tools/
policy/store/events), `SingleOwner` + process `claim_engine`, `assert_single_owner`,
`assert_same_turn` dual-client same-turn-ID check, `const` bounded queues.
Real Rust, `forbid(unsafe_code)`, no blocking calls, no stubs.

## Source evidence (exact commit/path/line)
- `crates/server/src/lib.rs:73` — `TURN_PERMITS: Semaphore::const_new(2)`.
  Cap adopted as `MAX_CONCURRENT_TURNS = 2`.
- `crates/server/src/lib.rs:75-78` — `AppState { sessions, catalog }` loose pair;
  this file bundles the owned superset instead.
- `crates/server/src/lib.rs:79-131` (`router`) — routes built per loose state,
  no `/api/events`; engine exposes shared `EventBus` handle for future wiring.
- `crates/server/src/lib.rs:135-194` (`web_capabilities`) — tools hardwired
  `available_for_web_turn: false`; engine snapshots tool defs + deny-wins
  `EnginePolicy` as the broker-checked composition half.
- `crates/agents/src/executor.rs:83-110` — `ExecutionManager::execute/cancel`
  in-memory map; this file does not duplicate it (executor lane owns behavior).
- `sources/completion/audits/AUD-005.json:56` — registry->executor->policy chain
  unwired (`permission.rs:1` stub); `EnginePolicy` deny-wins over allow/default.
- `sources/completion/audits/AUD-010.json:48` — daemon+events+sdk+proxy+headless
  e2e missing; this file composes the daemon-owned handles + `EventBus`.
- `crates/tools/src/tool_allow.rs:8` — `MAX_TOOL_ALLOW = 128`; asserted equal
  to `MAX_POLICY_RULES` in `caps_match_cross_crate_bounds` test.
- `crates/providers/src/responses.rs:14` — `MAX_RESPONSES_INPUT_MESSAGES = 100`;
  asserted equal to `MAX_TURN_HISTORY`.

## Observed scenario
- Wrote `app_runtime.rs`, checked under temp `pub mod app_runtime;` insertion
  into `lib.rs` (reverted after; `lib.rs` untouched per lease).
- `cargo check -p opencode-rk-server --tests`: clean, `Finished dev`.
- `cargo test -p opencode-rk-server --lib app_runtime`: 10/10 pass.
- `cargo clippy -p opencode-rk-server --lib`: zero `app_runtime` lints (one
  pre-existing `lib.rs:851` `never actually loops` deny-by-default error
  reproduces on stashed tree, unrelated).
- `grep -rn app_runtime crates/ tools/`: no references; wiring left to
  integration lane (lease forbids `lib.rs` edits).

## Target boundary
- Owned file only: `crates/server/src/app_runtime.rs`. No `lib.rs`, Cargo, or
  executor edits. Integrator adds `pub mod app_runtime;` to `lib.rs`.
- `EngineHandles.tools` is a `Vec<ToolSnapshot>` definition snapshot (no
  locks/handles) so the engine introduces no second permit/process source;
  `lib.rs` `TURN_PERMITS` stays live until its lane adopts `TurnPermits`.
- `store: SessionService` duplicates the `sessions` handle by value (cheap
  `Arc` clone) to keep storage authority separately typed; no direct
  `opencode-rk-storage` dep (regular deps lack it; dev-deps only).
- Execution behavior (provider calls, tool dispatch, cancel/reclaim) stays in
  turn/executor lanes; same-turn helper returns shared `AgentId`, errors on
  divergence.

## Tests (frozen in-file `#[cfg(test)]`, 10 tests)
`caps_match_cross_crate_bounds`, `pending_queue_rejects_overflow_and_preserves_fifo`,
`pending_turn_rejects_bad_model_ref`, `single_owner_claim_release_reclaim`,
`global_engine_claim_is_exclusive`, `assert_single_owner_counts`,
`same_turn_shared_across_two_clients`, `same_turn_rejects_divergent_runtimes`,
`policy_deny_wins_and_rules_bounded`, `turn_permits_bound_concurrency`.

## Decisions
- `ponytail:` none — composition fits without new deps; `TurnPermits` wraps
  `tokio::sync::Semaphore` (already a server dep).
- `SingleOwner` uses `AtomicBool` CAS; `OwnerGuard` releases on `Drop`.
- `BoundedQueue::try_push` fails loudly (`QueueFull`); never drops silently.

## Remaining unknowns
- Integrator must add `pub mod app_runtime;` and route `lib.rs`
  `TURN_PERMITS`/turn builders through `EngineHandles` + `TurnPermits`.
- Provider-invocation, tool-dispatch-through-broker, cancel-reclaim, and
  durable-failure-event behaviors belong to turn/executor lanes with fixtures.
