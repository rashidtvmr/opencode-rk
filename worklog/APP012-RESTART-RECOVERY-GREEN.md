# APP012-RESTART-RECOVERY-GREEN

## Claim

- Task: `APP012-RESTART-RECOVERY-GREEN`
- Session: `ses_f2b483051ffeYvlZ1O07D1ssHP`
- Branch: `lane/APP012-RESTART-RECOVERY`
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/lane-app012-restart-recovery`
- Owned product file: `crates/storage/src/facade.rs`
- Scratchpad and ledger row are the only additional owned files.

## Source evidence

- `crates/storage/src/facade.rs:27-43`: `StorageFacade::open` selects
  `SchemaV2::open_existing` for a non-empty database and returns the facade.
- `crates/storage/src/facade.rs:135-143`: `close` writes
  `workspace_state.clean_shutdown=1` and checkpoints WAL.
- `crates/storage/src/schema_v2.rs:21,98-150`: existing opens apply
  `synchronous=FULL`, validate format/checksum, and do not perform startup
  fencing or recovery.
- `crates/storage/schema/v2/workspace.sql:144-185`: executions and provider
  attempts; recovery indexes use owner generation and state.
- `crates/storage/schema/v2/workspace.sql:186-212`: tool state 5 is uncertain,
  open states 0/1 have NULL finished timestamps, and `tools_recovery_idx`
  covers states 0/1/5.
- `docs/storage/CRASH_CONSISTENCY.md:152-177`: startup clears the clean marker,
  advances ownership under FULL, marks unsettled work uncertain, and never
  silently replays ambiguous effects.
- `crates/storage/tests/app012_restart_resume_red.rs:643-837`: frozen startup
  contract; initial run reproduced 13 pass / 5 fail.

## Contract and boundary

For an existing format-2 DB, `StorageFacade::open` runs one bounded IMMEDIATE
transaction before returning. It reads both startup markers, rejects invalid or
overflowing generation, increments exactly once, sets `clean_shutdown=0`, then
marks at most 500 prior-generation running execution roots uncertain. Open
provider attempts for selected roots become state 4; dispatched tools become
state 5. Terminal rows and finish timestamps remain untouched. Root selection is
deterministic, indexed, parameter-bounded, and LIMIT-capped. Transaction errors
rollback all startup changes.

Fresh file and `open_memory` initialization remain unchanged. `close()` retains
the clean marker behavior.

## Verification

- Frozen RED hash: `d2745fe142fb1add3cc26c9fe9715438c73a51084d5498373c990c0a8e2a8cce`.
- Frozen installed APP-012 hash: `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`.
- Initial focused RED: 13 passed, 5 failed for missing startup fencing/recovery.
- GREEN: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test --no-fail-fast -p
  opencode-rk-storage --test app012_restart_resume_red -- --test-threads=1`
  -> 18 passed.
- Regression: `cargo test -p opencode-rk-storage --lib -- --test-threads=1`
  -> 122 passed; `cargo test -p opencode-rk-storage --tests -- --test-threads=1`
  -> 205 passed; `cargo check -p opencode-rk-storage` -> 0 errors, one existing
  duplicate CLI target warning.
- Targeted restart/schema: 11 passed. Facade unit tests: 5 passed.
- `git diff --check` -> clean. Frozen hashes unchanged.
- `python3 tools/convergence_gate.py` -> blocked by pre-existing off-plan ledger
  findings; not changed. `tools/lane_gate.py` has no APP012 lane registration.
- Host memory snapshot: `vm_stat` recorded 8,813 free pages and 377,879
  inactive pages after tests. No parallel Cargo commands.

## Residual gaps

Broker-level conditional resume, OS ownership-lock proof, and daemon/live-turn
execution/approval caller wiring remain outside this file and task.
