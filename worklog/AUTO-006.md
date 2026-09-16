# AUTO-006 worklog

## Claim

Ready-queue + one-file lease + gate-before-commit driver, stop-when-empty. Additive agents lane only.

## Source evidence

- HEAD `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`.
- `tasks/AUTO-006.md:11-15` outcome, `:50-67` contract, `:100-121` T01-T05.
- `PLAN.md:61-67` ADR-001/003 bounded runtime; `:143-156` one owned file per lane, serialized integration; `:202-220` stop semantics.
- `docs/TDD.md:21-56` lifecycle/RED/freeze; `docs/SECURITY.md:9-22` bounds.
- `docs/AUTONOMOUS_EXECUTION.md:6-31` ready queue + one worktree per worker + gate + serial `finalize_worktree` + `stopWhenNoReadyWork`; `:89-100` stop/attempt-cap semantics. Prototype: `tools/ralph_loop.py` (`ready_queue`, `run_task`, `record_result`, `finalize_worktree`), `tools/plan_model.py` rank sort.
- `requirements/user-requirements.json` REQ-002 AUTO-001..007.
- `tools/plan_model.py:27-49` AUTO rank 6, override AUTO-001/002 only.
- `ralph.json:320-335` AUTO-006 `in-progress`.
- `sources/backlog-exhaustion.json:74-95` AUTO-006 `explicit-blocker automation-ownership-undefined`, taskCard/worklog null — stale vs tracked impl, acceptance external.

## Observed scenario

- Pre-change agents crate had no ready-queue/one-file-lease/gate-before-commit driver: no `crates/agents/src/driver_lane.rs` before slice; `ReadyQueue` rank sort, `LeaseTable` one-file-per-lane, `Driver::gate`/`drive_task`/`commit`/`drive` stop-when-empty semantics undefined (prototype only in `tools/ralph_loop.py` ready_queue/run_task/record_result/finalize_worktree + `tools/plan_model.py` rank sort).
- Observed via suite `crates/agents/tests/driver_lane.rs` (`#[path="../src/driver_lane.rs"]` bypass `:1-2`): no original RED history — honest GREEN-only record, mirrors AUTO-004.
- Post-change full-crate GREEN `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-agents -- --test-threads=2` => 15 unit + 5 delegation + 5 driver + 5 turn + 5 delegation_gated = 35 passed, 0 failed (re-verified 2026-09-16).
- Temp-stub RED probe (2026-09-16, owned impl file only, frozen tests untouched, restored byte-identical): inverted lease-release guard (`!=` instead of `==`) => `--test driver_lane` => 4 passed, 1 failed (`auto_006_t02` panic at tests:70). Restored via backup; `diff -q` IDENTICAL.
- Shared broker-gated pool (`crates/agents/src/delegation_lane.rs` broker section, on disk) is the one-file-lease analog at admission (`BoundedLanePool::try_acquire`); probe RED fail / GREEN pass history lives in AUTO-004 worklog (temp probe, deleted); no frozen-test edits.

## Target boundary

- Owned: `crates/agents/src/driver_lane.rs` only.
- Shared pre-wire: `crates/agents/src/lib.rs` adds `pub mod driver_lane;` (+ delegation line, 2-line diff, uncommitted).
- Tests: `crates/agents/tests/driver_lane.rs` only. NEVER edit tests.

## Tests

- Frozen SHA-256 `4d9a30067d2bbf038f132decb51e5ed6c8c5f159de94e0133513c20a27405a37` (`crates/agents/tests/driver_lane.rs`).
- Impl SHA-256 `c19ad097a15b82cc5285f641e15db0af698d9dc4c19ef306283234838fa1f6dd`.
- RED baseline: original RED absent; 2026-09-16 temp-stub probe gives compiling RED (4 pass / 1 fail, lease-guard inversion) on restored-identical impl.
- GREEN receipt: `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-agents --test driver_lane -- --test-threads=2` => 5 passed, 0 failed.
- `cargo check -p opencode-rk-agents` clean.
- NOTE: working tree shows uncommitted diffs in `crates/agents/src/driver_lane.rs`, `delegation_lane.rs`, `message.rs`, `crates/agents/tests/driver_lane.rs`, `delegation_lane.rs` from other concurrent lanes (fmt-only in tests + broker-gated pool in impl). Not authored by this lane; hashes above are on-disk state at verify time.

## Decisions

- `ReadyQueue::next` returns not-started tasks with authored + synthesized lower-rank deps accepted; blocked never returned; `blocked_dependents` reports waiters; sort `(rank asc, id asc)`, truncate `limit`.
- `LeaseTable`: one file per lane; second file / live-file steal / non-owner release => `LeaseDenied`, state unmutated. Idempotent same-owner same-file re-acquire.
- `Driver::gate`: missing/short (`<20B`)/placeholder => `Stub`; marker-less real-length => `Incomplete`; only `verified:gate-pass` bodies pass. Gate never commits. `drive_task` retries to `ATTEMPT_CAP=3`, then `Blocked` with `verification failed`. `commit` ff-conflict => `Accepted` + `accepted but not integrated`, branch preserved, never force-push. `drive()` empty => `Stopped::NoReadyWork`; `stop_report` exit 0 + receipt path + sorted blocked; idempotent.
- Bounds: `max_lanes>=1`; tails 12 lines / 4 KiB + marker; receipts append-only bounded tails; no OS process per lane; no threads beyond caller pool.
- Deviation: in-process lease/gate/commit model; full parallel orchestration tuning stays operator-owned via `config/controller.settings.json` (maxConcurrentLanes 6 ceiling, small-machine 1-2).
- Gaps kept: sync only, zero tokio in agents crate; `tools/lane_gate.py` has no AUTO lane; `check_tdd_pipeline.py` partition untouched.

## Remaining unknowns

- Original RED history missing; temp-stub probe is the only compiling-RED evidence. Verifier must treat as incomplete RED per TDD.
- `drive_ready` skips tasks without leases (caller acquires first); merge/ff into mainline is caller/controller job (`finalize_worktree`), not this crate.
- Ledger blocker vs tracked impl conflict; acceptance external.

## Broker+tokio slice (2026-09-16, shared lane AUTO-004/005/006)

- Driver-role note: `BoundedLanePool::try_acquire` is the one-file-lease analog at admission — broker check first, cap enforced, no queueing, atomic Send+Sync for sharing across Tokio lane tasks (PLAN.md ADR-001 single runtime). See AUTO-004 worklog for RED fail / GREEN pass / hashes; impl `crates/agents/src/delegation_lane.rs` +80/-0, frozen tests untouched.

## Sole-writer revalidation (2026-09-16, rev 248f519)
- Pre==post sha256 c19ad097a15b82cc5285f641e15db0af698d9dc4c19ef306283234838fa1f6dd (diff -q IDENTICAL).
- Temp-stub: release guard == -> !=. RED /tmp/opencode/wA-AUTO006-red.log 4 pass / 1 fail (t02 tests:70). GREEN /tmp/opencode/wA-AUTO006-green.log 5/5.

## Sole-writer revalidation xA (2026-09-16, rev 248f519)
- Pre==post sha256 c19ad097a15b82cc5285f641e15db0af698d9dc4c19ef306283234838fa1f6dd (diff -q IDENTICAL).
- Temp-stub: release guard == -> !=. RED /tmp/opencode/xA-AUTO-006-red.log 4 pass / 1 fail (t02 tests:70 non-owner release succeeded). GREEN /tmp/opencode/xA-AUTO-006-green.log 5/5.
