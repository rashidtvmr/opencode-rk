# BASE-003 - Rust plus Tokio native runtime foundation (REQ-001)

Status: COMPLETE. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-001.
Dependencies: none.
Ownership locks: crates/foundation/src/lib.rs, this card.
Suggested module: `crates/foundation/src/lib.rs` (bounded primitives only).
Contracts/schema/build-manifest edits go through the integration lane.

## User-observable outcome

The native runtime has bounded structured-concurrency primitives with no
unbounded queue, no unbounded retained output, and no detached task without an
owner. Every spawned task is owned by an `OwnedTaskScope`, every byte charged
against a `ByteBudget`, every expensive service constructed once via
`LazyService`, and every periodic poll auto-cancelled via `HeartbeatGuard`.

## Source evidence

- `crates/foundation/src/lib.rs` (OwnedTaskScope, RuntimeLimits,
  bounded_channel, ByteBudget, LazyService, HeartbeatGuard): pre-existing plus
  BASE-003 alias additions.
- `sources/behavior-surface-rules.json` id `opencode.effect-runtime`
  (packages/core/src/effect/**, packages/effect-drizzle-sqlite/**,
  packages/effect-sqlite-node/**): upstream Effect runtime mapped to native
  Tokio equivalents, owned by BASE-003/BASE-007/DB-008/DB-009/OPS-007.
- `docs/SECURITY.md` non-negotiables: no unbounded queue, no detached task
  without owner, scoped cancellation, byte budgets, lazy services.
- `FEATURES.md` REQ-001: Rust plus Tokio and maximum practical resource
  savings, shared with OPS-001/OPS-007.

## Observable contract

- `RuntimeLimits { max_owned_tasks, max_queue_items, max_provider_calls,
  max_blocking_jobs, shutdown_grace_ms }`: all non-zero, else
  `InvalidLimit(name)`; `Default` is 256/256/16/4/5000.
- `OwnedTaskScope::new(&limits)`: validates limits; `spawn` rejects with
  `TaskLimitReached` at capacity and `ShuttingDown` after cancel;
  `shutdown` cancels then joins within grace else `ShutdownTimedOut`;
  `Drop` cancels.
- `bounded_channel(capacity)`: rejects zero with
  `InvalidLimit("channel capacity")`, else Tokio mpsc channel.
- `ByteBudget::new(limit)`: rejects zero with `InvalidLimit("byte_budget")`;
  `acquire`/`try_consume` CAS-loop charge else `BudgetExceeded { requested,
  available }`; `release` saturates at zero; `reserve` returns RAII
  `ByteBudgetReservation` auto-releasing on drop unless `leak`ed;
  `used`/`limit`/`available`/`remaining` observers; `reset` releases all.
- `LazyService<T>::new(init)` / `bounded(max_concurrency, init)`: rejects
  zero concurrency with `InvalidLimit("lazy_service_concurrency")`;
  construct-on-first-use via Tokio OnceCell; `get` returns `Arc<T>` and runs
  init once; `acquire` awaits semaphore permit; `try_acquire` fails fast
  with `ServiceInit` (uninitialized) or `ServiceLimitReached`;
  `is_initialized` observer.
- `HeartbeatGuard::spawn(interval, action)` /
  `spawn_with_token(parent, interval, action)`: rejects zero interval with
  `InvalidLimit("heartbeat_interval")`; periodic tick skips missed ticks;
  `ticks()` counts fired callbacks; `cancel()`/`Drop` cancels token and
  aborts the task.
- No new dependencies. No `unsafe`. No wall-clock dependence in decisions
  (interval timing is scheduling only).

## Failure states

- Zero limits, zero channel capacity, zero byte budget, zero service
  concurrency, zero heartbeat interval: `InvalidLimit`.
- Owned task spawn past bound: `TaskLimitReached`; spawn after shutdown:
  `ShuttingDown`; shutdown past grace: `ShutdownTimedOut` after abort.
- Byte charge past limit: `BudgetExceeded { requested, available }`.
- Uninitialized service `try_acquire`: `ServiceInit`; bounded service at
  capacity: `ServiceLimitReached`; init closure failure: `ServiceInit(msg)`.

## Acceptance criteria

- BASE-003-T01: scope owns tasks, enforces bound, shutdown joins in grace.
- BASE-003-T02: byte budget enforces limit, reservation auto-releases,
  try_consume/remaining/reset aliases hold.
- BASE-003-T03: lazy service constructs once on first use.
- BASE-003-T04: lazy service bounded concurrency (acquire/try_acquire).
- BASE-003-T05: heartbeat ticks, cancels on drop, rejects zero interval.
- `cargo test -p opencode-rk-foundation` green (11 tests);
  `cargo check --workspace` clean (modulo unrelated lanes).

## Test-first execution

1. RED: prior suite compiled (primitives pre-existing); alias test
   `byte_budget_aliases_try_consume_remaining_reset` added and run against
   missing methods (failed to compile = missing behavior).
2. GREEN: minimum alias implementation
   (`try_consume`/`remaining`/`reset` delegating to acquire/available/
   release); 11 tests pass (10 pre-existing preserved).
3. Evidence: crate log shows `11 passed; 0 failed`.
