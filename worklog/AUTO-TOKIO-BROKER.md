# AUTO-TOKIO-BROKER audit — agents lane (AUTO-004)

## Evidence (repo commit 248f519)

- `grep -rn tokio crates/agents/` → no matches. `grep -n tokio crates/agents/Cargo.toml`
  → no matches. Crate deps: chrono, contracts, serde, thiserror, uuid only
  (`crates/agents/Cargo.toml:9-14`).
- `grep -rn PermissionBroker|BrokerDecision crates/agents/src/` → 7 hits, all in
  `crates/agents/src/delegation_lane.rs:329-374`: local `BrokerDecision` mirror
  enum (`Allow|Deny|RequireHuman`), `broker_allows()`, `submit_gated()`,
  `BoundedLanePool` (atomic counter, sync-only). No `PermissionBroker` import;
  no `opencode-rk-security` dep.
- Trusted broker lives at `crates/security/src/lib.rs:151` (`PermissionBroker`;
  nearby `RuleEffect::{Allow, Deny, Ask}` at `:100-103`). Mirror variant names
  drift: `RequireHuman` vs security `Ask`.
- Location drift: card suggests new crate `opencode-rk-delegation`
  (`tasks/AUTO-004.md:63-65`); implementation is `crates/agents/src/delegation_lane.rs`.
  Card says "Suggested", so non-binding.
- GREEN: `cargo test -p opencode-rk-agents` EXIT 0, full log at
  `/tmp/opencode/uH-agents.log` (35 lines tail captured; serial JOBS=2 THREADS=2,
  `timeout 120`, `rtk free -h` showed 2.1 GiB avail). All T01–T05 + gated +
  driver + turn suites pass.

## Card quotes (tasks/AUTO-004.md)

- `:66` "Stdlib + Tokio only; no new dependency, no network, no secret logging."
- `:97-98` "delegation runs as in-process Tokio tasks on the single daemon
  runtime (PLAN.md ADR-001/ADR-003); spawning an OS process per agent fails
  the bound test. No detached task without an owner; no background thread
  beyond the runtime."
- `:93-96` cancel "joins the task and drops output within `cancel_timeout_ms`";
  T04 asserts `task_joined(id)` + `elapsed <= cancel_timeout_ms`.
- Broker: card never mentions PermissionBroker/broker. Zero hits implied —
  no broker requirement in contract, bounds, or T01–T05.

## Verdict

- **tokio: REQUIRED, NOT MET.** Card explicitly mandates Tokio tasks on the
  single daemon runtime. Current `task_joined()` (`delegation_lane.rs:287-289`)
  checks a sync `bool` flag; `cancel` is synchronous state flip, so T04's
  latency/task assertions pass vacuously. Behavioural contract (submit/detach/
  status/cancel/bounds) holds; runtime mechanism does not. Needs card amendment
  (downgrade to sync state machine) OR separate integration lane — dep change
  needs controller approval either way.
- **broker: mirror SUFFICES.** Card imposes no broker obligation; local
  `BrokerDecision` mirror + `BoundedLanePool` + `submit_gated` (tested by
  `crates/agents/tests/delegation_gated.rs`, 82 lines) is additive hardening,
  not a contract item. Wiring real `PermissionBroker` would add an
  agents→security dep the card never asked for. Note variant-name drift
  (`RequireHuman` vs security `Ask`) if ever unified.

## Integration proposal (NOT applied — needs controller approval)

1. Deps: `crates/agents/Cargo.toml` += `tokio.workspace = true`
   (workspace already has tokio 1 + sync/time; reuse, no new dep). If the
   agents crate must stay sync-only per prior lane note, place runtime use in
   the suggested `crates/delegation` crate instead.
2. Pool: back `BoundedLanePool` permit with `tokio::sync::Semaphore` (cap =
   permits); keep atomic `live()` for sync introspection, or make
   `try_acquire` use `Semaphore::try_acquire_owned`.
3. Spawn: `submit()` spawns work as `tokio::spawn` on daemon runtime, stores
   `JoinHandle` in `Entry` (replace `live_task: bool`); `cancel()` aborts +
   `timeout(cancel_timeout, handle).await`, maps timeout/failure to
   `State::Failed`/`Cancelled` with truncated summary; `task_joined()` checks
   `handle.is_finished()`.
4. Broker call site: keep mirror for admission; if real verdicts required,
   call `PermissionBroker` at `submit_gated` boundary only (deny → no state,
   matching current semantics). Do NOT take agents→security dep without
   controller sign-off; alternative is a callback/fn-pointer injection so
   agents stays dependency-free.
5. Test plan: keep frozen T01–T05 untouched; new lane adds async tests with
   `#[tokio::test]` + small `cancel_timeout` proving abort actually stops a
   pending task (e.g. sleep-forever work cancelled within bound), semaphore
   exhaustion under concurrent submits, and no OS-process assertion retained.
   Run serial `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120`.
