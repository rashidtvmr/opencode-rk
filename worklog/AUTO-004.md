# AUTO-004 worklog

## Claim

Foreground/background delegation ownership, queryable controller status, bounded cancel, no OS process per agent. Additive agents lane only.

## Source evidence

- HEAD `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`.
- `tasks/AUTO-004.md:9-17` outcome, `:48-69` contract, `:102-127` T01-T05.
- `sources/disc-003-reconciliation.json` `opencode.agent-delegation` finding cited in card.
- `PLAN.md:61-67` ADR-001 single Tokio runtime, no process per subagent; `:143-156` slice independence; `:162-180` RED lifecycle.
- `docs/TDD.md:21-56` lifecycle + RED rules; `docs/SECURITY.md:9-22` broker/bounds.
- `requirements/user-requirements.json` REQ-002 AUTO-001..007.
- `tools/plan_model.py:44` AUTO rank 6, no AUTO-004 override.
- `ralph.json:287-302` AUTO-004 `in-progress`.
- `sources/backlog-exhaustion.json:29-50` AUTO-004 `explicit-blocker automation-ownership-undefined`, taskCard/worklog null — stale vs files below, controller acceptance external.

## Observed scenario

- Pre-change agents crate had no owned delegation records and no queryable controller status/bounded cancel: no `crates/agents/src/delegation_lane.rs` before slice (only executor/session primitives); status/detach/cancel/restart semantics undefined.
- Observed via suite `crates/agents/tests/delegation_lane.rs` (`#[path="../src/delegation_lane.rs"]` bypass `:1-2`): no original RED history exists for the frozen suite — honest GREEN-only record, not fabricated RED.
- Post-change GREEN cmd `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-agents -- --test-threads=2` => 15 unit + 5 delegation + 5 driver + 5 turn + 5 delegation_gated = 35 passed, 0 failed (re-verified 2026-09-16).
- Temp-stub RED probe (2026-09-16, owned impl file only, frozen tests untouched, restored byte-identical): inverted owner check (`==` instead of `!=` in `cancel`) => `--test delegation_lane` => 3 passed, 2 failed (`auto_004_t02` panic at tests:37, `auto_004_t03` panic at tests:60). Restored via backup; `diff -q` IDENTICAL.

## Target boundary

- Owned: `crates/agents/src/delegation_lane.rs` only.
- Shared pre-wire: `crates/agents/src/lib.rs` adds `pub mod delegation_lane;` + `pub mod driver_lane;` (2 lines, uncommitted diff).
- Tests: `crates/agents/tests/delegation_lane.rs` only. NEVER edit tests.
- No Tokio dep added to agents crate; no PermissionBroker wiring in this slice.

## Tests

- Frozen SHA-256 `3ca19e36aae23c85a0ee65085d7df62b8d793c88ed2bcc08d02e7e0355b1d971` (`crates/agents/tests/delegation_lane.rs`).
- Impl SHA-256 `51d8a6c1c7fcfed589c60d1deb7eced8cbf642f7ba83772baeded5bf573e9681`.
- RED baseline: original RED absent; 2026-09-16 temp-stub probe gives compiling RED (3 pass / 2 fail, owner-check inversion) on restored-identical impl. Probe RED belongs to additive slice only, not frozen-suite RED.
- GREEN receipt: `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-agents --test delegation_lane -- --test-threads=2` => 5 passed, 0 failed.
- `cargo check -p opencode-rk-agents` clean.
- NOTE: working tree shows uncommitted diffs in `crates/agents/src/delegation_lane.rs`, `driver_lane.rs`, `message.rs`, `crates/agents/tests/delegation_lane.rs`, `driver_lane.rs` from other concurrent lanes (fmt-only in tests + broker-gated pool in impl). Not authored by this lane; hashes above are on-disk state at verify time.

## Decisions

- Sync controller-owned records. `submit` enters `Running`, visible to `status`. `detach` owner-checked, `Running->Detached`, owner retained. Unknown id `Unknown`. Non-owner `NotOwner`. Double cancel idempotent `Cancelled`; cancel-after-complete `Terminal`. `fail` bounds summary. `restart` marks live `Failed`, terminal retained.
- Bounds: `max_live` default 16, over-cap `AtCapacity`; summary cap 4096 with `...[truncated]`; `cancel_timeout` 1000ms; `child_process_count()=0` always; `task_joined` asserts reclamation.
- Deliberate deviation per card: small queryable record, not full durable child-agent state. Resume/steer/persisted context/cross-provider selection out of scope.
- Tokio gap kept: card says "Stdlib + Tokio only", impl is stdlib sync state with no `tokio`, no async, no broker (`grep crates/agents/src: no match`). `crates/agents/Cargo.toml` lacks tokio; workspace root defines tokio. PermissionBroker lives at `crates/tools/src/ext_scoped_exec_lane.rs:119` (+ duplicate `plugin_scoped_exec.rs:119`), unused by agents crate.
- `tools/lane_gate.py:31-44` covers storage lanes only, no AUTO lane.

## Remaining unknowns

- Original RED history cannot be reconstructed without editing frozen tests — forbidden. Verifier must treat GREEN-only frozen evidence + temp-stub probe as incomplete RED per TDD.
- Tokio/broker integration undefined as owned slice; needs task-card amendment or separate lane.
- Ledger `explicit-blocker` vs on-disk tracked impl conflict; controller/verifier acceptance external, not claimed.

## Broker+tokio slice (2026-09-16, shared lane AUTO-004/005/006)

- Claim: additive broker-gated admission + minimal bounded pool in owned file.
- Source: `crates/agents/src/delegation_lane.rs` `submit_gated`/`BrokerDecision`/`broker_allows`/`BoundedLanePool`; mirrors `crates/security/src/lib.rs:151` PermissionBroker verdict dependency-free; pool atomic, Send+Sync, no thread/spawn, caller Tokio runtime per PLAN.md ADR-001.
- RED (temp probe `crates/agents/tests/pool_red_probe.rs`, deleted after): `pool_red_probe_broker_deny_blocks_admission` FAILED — `RED: broker deny must block admission, live=1` (plain `submit` admits with no gate).
- GREEN (temp probe, deleted after): `pool_green_probe_broker_gated_bounded_pool` ok — Deny/RequireHuman => `Err(BrokerDenied)` + `live_count()==0`; Allow admits; pool cap-1 second acquire false, broker-shut false, over-release no underflow.
- Regression: `cargo test -p opencode-rk-agents` => 15 unit + 5 delegation + 5 driver + 5 turn = 30 passed; clippy 0 warnings. Frozen tests untouched (`delegation_lane.rs` `19be3b82…`, `driver_lane.rs` `e59843ad…`).
- Impl SHA-256 `3c671d9fc1fad65a1cf56e4fd20f272c467a56fa94e953735881c52aa95bbaba`; diff `crates/agents/src/delegation_lane.rs` +80/-0 (pure additive within owned file).
- Tokio note: no new tokio dep in agents crate (workspace tokio already); pool runtime-neutral atomic, no Semaphore needed — stdlib sufficient per ladder rung 2.

## Sole-writer revalidation (2026-09-16, rev 248f519)
- Pre==post sha256 51d8a6c1c7fcfed589c60d1deb7eced8cbf642f7ba83772baeded5bf573e9681 (diff -q IDENTICAL).
- Temp-stub: detach guard != -> ==. RED /tmp/opencode/wA-AUTO004-red.log 3 pass / 2 fail (t02 tests:37, t03 tests:60). GREEN /tmp/opencode/wA-AUTO004-green.log 5/5.

## Sole-writer revalidation xA (2026-09-16, rev 248f519)
- Pre==post sha256 51d8a6c1c7fcfed589c60d1deb7eced8cbf642f7ba83772baeded5bf573e9681 (diff -q IDENTICAL).
- Temp-stub: detach guard != -> ==. RED /tmp/opencode/xA-AUTO-004-red.log 3 pass / 2 fail (t02 tests:37 NotOwner, t03 tests:60 NotOwner). GREEN /tmp/opencode/xA-AUTO-004-green.log 5/5.

## fix-agents delegation_lane (248f519)
- Claim: detach owner check `entry.owner != owner` correct; reported t02/t03 NotOwner not reproducible at rev 248f519 + current tree. No impl change needed.
- Evidence: crates/agents/src/delegation_lane.rs:207-211 guard correct; tests t02:36 t03:59 pass owner_a matching submit owner.
- Verify: `CARGO_BUILD_JOBS=1 cargo test -p opencode-rk-agents -- --test-threads=1` -> 35/35 ok, log /tmp/opencode/fix-agents.log (grep ok count 35). delegation_lane 5/5.
- Diff: crates/agents/src/delegation_lane.rs +83 -20 (pre-existing workdir additions: BrokerDenied/submit_gated/BoundedLanePool + rustfmt reflow; sole-writer lane untouched otherwise). sha256 51d8a6c1c7fcfed589c60d1deb7eced8cbf642f7ba83772baeded5bf573e9681.
- Frozen files untouched: tests/delegation_lane.rs, ralph.json, other files (workdir shows unrelated pre-existing modifications, not made this wave).
