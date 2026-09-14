# BASE-007 - Runtime lifecycle manager

Status: GREEN. Kind: product. Requirements: configuration-runtime surface.

## Outcome

Ordered startup/shutdown hooks with phase tracking. `LifecycleManager::new()` starts Init. `start()` runs startup hooks low-priority-first, Init->Starting->Running. `stop()` runs shutdown hooks high-priority-first, Running->Stopping->Stopped.

## Module

- Owned: `crates/foundation/src/lifecycle.rs` (`pub mod lifecycle;` pre-existing in lib.rs). std Mutex only, no async.
- Types: `Phase`, `HookId(u64)`, `HookFn`, `LifecycleError` (AlreadyStarted, NotRunning, RegistrationClosed, LockPoisoned, HookFailed).
- Registration rejected unless phase Init. Hook order stable via insertion counter.

## Tests (in-module, 5)

- phase_transitions, startup_hooks_ordered, shutdown_hooks_reverse, no_hooks_after_running, double_start_fails.

## Commands

- `cargo test -p opencode-rk-foundation`: 22 passed.
- `cargo check --workspace`: 0 errors.
