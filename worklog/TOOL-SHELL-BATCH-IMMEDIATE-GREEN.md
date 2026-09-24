# TOOL-SHELL-BATCH-IMMEDIATE-GREEN - scratchpad

- Task: TOOL-SHELL-BATCH-IMMEDIATE-GREEN.
- Session: ses_f2b85a5fcffePZZKuqaYPSdcUj.
- Branch/worktree: lane/TOOL-SHELL-BATCH-IMMEDIATE-GREEN.
- Owned source: crates/tools/src/registry_dispatch.rs.
- Frozen RED: crates/tools/tests/registry_batch_immediate_red.rs,
  SHA-256 `aaad6ab33406a3d5cecf8ca8d5ce0ae6ca16ba7aa22ce264d1abf5240efdc0af`.
- Frozen broker: crates/tools/tests/phase1_shell_broker.rs,
  SHA-256 `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8`.

## Source evidence

- Base commit: `6e1695dbd01981d22c95291b7fe576aeb56e5d20`.
- `dispatch_batch` builds `ready: Vec<Option<Ready>>` at
  `registry_dispatch.rs:241-274`.
- Its spawn extraction at `registry_dispatch.rs:278-303` calls `slot.take()`
  for every slot. `Ready::Immediate` therefore becomes `None`.
- Rebuild at `registry_dispatch.rs:324-339` converts those missing immediate
  entries to `Unknown("slot-{idx}")`, losing exact single-dispatch outcomes.
- Single `dispatch` at `registry_dispatch.rs:182-218` is the parity reference.

## RED reproduction

`env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p opencode-rk-tools
--test registry_batch_immediate_red -- --test-threads=2` compiled and failed
0 passed, 6 failed. Failures were immediate unknown, disabled, oversized,
shell-denied, ordered parity, denied/empty cases; each exposed
`Unknown("slot-i")` substitution.

## Target boundary

Extract only `Ready::Spawn` values. Keep `Ready::Immediate` slots intact.
Rebuild in request order. Preserve semaphore guard and existing durable-write
behavior only for spawned records. No test, dependency, policy, or unrelated
source edits.

## Verification

- RED reproduced at base: `env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo
  test -p opencode-rk-tools --test registry_batch_immediate_red
  -- --test-threads=2` -> 0 passed, 6 failed.
- GREEN frozen target: same command -> 6 passed, 0 failed.
- Frozen broker: `env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p
  opencode-rk-tools --test phase1_shell_broker -- --test-threads=2` -> 3
  passed, 0 failed.
- `cargo test -p opencode-rk-tools --lib -- --test-threads=2` -> 107 passed,
  4 failed. Failures are pre-existing environment/stale-contract cases:
  `executor::tests::execute_success`, `executor::tests::execute_timeout`,
  `mcp_spawn::tests::disc111_t04_crash_bounded_retry_no_silent_restart`, and
  `registry_dispatch::tests::disc103_t05_batch_respects_max_parallel_bound`
  (the latter expects unbrokered `bash` execution, while the frozen broker
  contract denies it).
- `python3 tools/validate_repository.py` -> expected pre-existing backlog
  exhaustion failure (51 errors); no verifier files changed.
- `python3 tools/lane_gate.py --help` -> supported; `--run` is not run until
  ledger evidence is complete.
- Frozen hashes unchanged: RED
  `aaad6ab33406a3d5cecf8ca8d5ce0ae6ca16ba7aa22ce264d1abf5240efdc0af`; broker
  `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8`.
- `git diff --check` clean. No child-process survivors observed. `free -h`
  recorded before validation; Cargo runs were sequential with jobs/threads 2.
