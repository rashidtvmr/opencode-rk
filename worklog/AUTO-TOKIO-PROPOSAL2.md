# AUTO-TOKIO-PROPOSAL2 — tokio integration for AUTO-004 delegation lane (rev 248f519)

## Confirmation evidence

- `grep -rn tokio crates/agents/` → 0 matches (EXIT:1, verified 2026-09-16). Workspace
  `Cargo.toml:37` already has `tokio 1 {fs, macros, net, rt-multi-thread, signal, sync, time}`;
  `crates/agents/Cargo.toml:9-14` deps are chrono, contracts, serde, thiserror, uuid only.
- `grep -rn BrokerDecision crates/agents/src/` → 6 hits, all
  `crates/agents/src/delegation_lane.rs:329,342,348,353,354,374`. No `PermissionBroker`
  import; no `opencode-rk-security` dep in agents crate.
- Trusted broker: `crates/security/src/lib.rs:151` (`pub struct PermissionBroker`);
  `RuleEffect::{Allow,Deny,Ask}` at `:99-103`; lane mirror
  `BrokerDecision::{Allow,Deny,RequireHuman}` at `delegation_lane.rs:342-346` drifts
  (`RequireHuman` vs `Ask`).
- Current impl: sync-only. `Entry.live_task: bool` (`:122`), `task_joined()` checks bool
  flag (`:287-289`), `cancel` is sync state flip (`:241-246`), `BoundedLanePool` is atomic
  counter (`:359-398`). Card mandates at `tasks/AUTO-004.md:66` ("Stdlib + Tokio only")
  and `:97-98` ("delegation runs as in-process Tokio tasks on the single daemon runtime").

## Verdict

- **tokio: REQUIRED, NOT MET.** Behavioural contract (submit/detach/status/cancel/bounds)
  holds; runtime mechanism does not. T04 `task_joined` + latency assertions pass vacuously
  on sync flag.
- **broker mirror SUFFICES.** Card never mentions PermissionBroker. Wiring real broker
  would add agents→security dep card never asked for.

## Integration proposal (needs controller approval — NOT applied)

1. **Dep additions (reuse workspace, no new crate dep):**
   `crates/agents/Cargo.toml += tokio.workspace = true` (gets `sync`, `time`, `macros`).
   For semaphore-only use: `tokio = { workspace = true, features = ["sync"] }` — but
   workspace unified features already include sync/time/macros; plain
   `tokio.workspace = true` reuses existing. Alternative placement: new
   `crates/delegation` crate per card `:63-65` if agents must stay sync-only.
2. **Pool backed by tokio:** `BoundedLanePool { semaphore: Arc<tokio::sync::Semaphore> }`,
   cap = permits; `try_acquire` → `Semaphore::try_acquire_owned`; keep atomic `live()`
   for sync introspection or derive from `semaphore.available_permits()`.
3. **Spawn/join:** `submit()` → `tokio::spawn` work on daemon runtime, store `JoinHandle`
   in `Entry` (replace `live_task: bool`); `cancel()` → `handle.abort()` +
   `tokio::time::timeout(cancel_timeout, handle).await`, timeout/failure →
   `State::Failed`/`Cancelled` with truncated summary; `task_joined()` →
   `handle.is_finished()`.
4. **Broker call site:** keep mirror for admission (zero-dep). If real verdicts required,
   call `PermissionBroker::authorize` (`security/src/lib.rs:151,202`) at `submit_gated`
   boundary only (deny → no state, current semantics). Do NOT add agents→security dep
   without sign-off; alternative is callback/fn-pointer injection
   (`submit_gated(owner, mode, work, impl Fn() -> BrokerDecision)`) so agents stays
   dependency-free. Map `Decision::RequireHuman{..}` → `BrokerDecision::RequireHuman`
   (document `Ask` vs `RequireHuman` naming drift if unified).
5. **Test plan (frozen T01–T05 untouched):** new lane adds `#[tokio::test]` async tests:
   (a) sleep-forever work cancelled within `cancel_timeout` bound, assert
   `handle.is_finished()`; (b) semaphore exhaustion under concurrent submits →
   `AtCapacity`, live count unchanged; (c) no-OS-process assertion retained.
   Run serial `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 120`.

## Card-amendment alternative (cheaper path)

Amend `tasks/AUTO-004.md:66,97-98` to downgrade runtime to sync state machine
("process-local records; async runtime deferred to daemon integration lane"), keeping
all T01–T05 semantics. Then current impl is GREEN-as-is, zero dep change, zero risk.
Recommended if daemon runtime wiring is out of M6 scope.

## GREEN validation

- Pending: `cargo test -p opencode-rk-agents`, log `/tmp/opencode/aD-agents.log`.
