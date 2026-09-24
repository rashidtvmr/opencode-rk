# TOOL-SHELL-BATCH-IMMEDIATE-VERIFY - scratchpad

- Task: TOOL-SHELL-BATCH-IMMEDIATE-VERIFY (independent verification, no source/test edits).
- Session: ses_f2b7e2a32ffevyBUi63wcT2y4x.
- Branch/worktree: verify/TOOL-SHELL-BATCH-IMMEDIATE at /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/verify-tool-shell-batch-immediate.
- Candidate: 55acf2942d950fea0b4c90c336ec10713f0fa81d (GREEN); RED base 6e1695dbd01981d22c95291b7fe576aeb56e5d20.
- Owned files: worklog/TOOL-SHELL-BATCH-IMMEDIATE-VERIFY.md + own ledger row only.
- Frozen hashes: RED aaad6ab33406a3d5cecf8ca8d5ce0ae6ca16ba7aa22ce264d1abf5240efdc0af; broker ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8.

## Source evidence
- registry_dispatch.rs:241-274 ready build (Immediate vs Spawn classification).
- registry_dispatch.rs:278-308 spawn extraction (GREEN: matches Spawn guard + take only Spawn).
- registry_dispatch.rs:318-347 rebuild in request order (Immediate pushed as-is; spawn pulls by_idx + store write only for spawned).
- registry_dispatch.rs:182-218 single dispatch parity reference.
- registry_dispatch.rs:43-44 SHELL_DENIED_NO_BROKER fixed denial; 369-388 is_shell_alias/shell_denied_record.
- RED->GREEN diff: crates/tools/src/registry_dispatch.rs only hunk at 276-308 (+ GREEN worklog + claims row); zero test-file diff (git diff 6e1695d..HEAD -- crates/tools/tests/ empty).
- RED test contract: 6 tests t01-t06 (unknown/disabled/oversized/shell-denied/ordered-parity/denied+empty), assert exact error/record parity, order, no store writes, permits reclaimed.

## Audit checks
- Only Spawn extracted; Immediate slots survive: PASS. GREEN guards `if !matches!(slot, Some(Ready::Spawn{..})) { continue; }` before `slot.take()`; Immediate slots stay `Some(Immediate)` into rebuild (registry_dispatch.rs:279-284).
- Immediate values / original IDs / errors / records / order survive: PASS. Rebuild pushes `Some(Ready::Immediate(r))` as-is; fallback `Unknown("slot-{idx}")` now reachable only for a Spawn slot whose join failed (bounded fail-closed), not for any Immediate class. Frozen t05 asserts per-index parity vs single dispatch.
- Permits remain bounded: PASS. Immediates (incl. shell denial) return before any acquire in both paths; spawns use `acquire_owned` under semaphore; frozen tests assert `available_permits == max_permits` in all 6.
- Denied/fail-closed cause no store writes: PASS. Store writes only in rebuild for spawned results (registry_dispatch.rs:333-339); Immediate path has no write; frozen tests assert `stats()==(0,0)` / `get(...).is_empty()` per class, t05 asserts only valid echo recorded (1).
- No unsafe policy expansion: PASS. Diff touches only extraction guard; AllowAll/DenyAll/is_shell_alias/shell_denied_record/policy checks unchanged; shell still denied before permit/spawn/store in both paths (single :204-206, batch :261-265).

## Commands (sequential, sole Cargo slot)
- `cargo test -p opencode-rk-tools --test registry_batch_immediate_red -- --test-threads=1` -> 6 passed, 0 failed.
- `cargo test -p opencode-rk-tools --test phase1_shell_broker -- --test-threads=1` -> 3 passed, 0 failed.
- `cargo test -p opencode-rk-tools --lib registry_dispatch -- --test-threads=1` -> 6 passed / 1 failed (disc103_t05 only); with `--skip disc103_t05` -> 6 passed.
- `cargo test -p opencode-rk-tools --lib -- --test-threads=1` -> 107 passed, 4 failed.
- `cargo check -p opencode-rk-tools` -> 0 errors, 6 warnings (pre-existing dead-code/unused-assign; no new deny).
- `python3 tools/lane_gate.py` -> feature lanes PASS; 4 UNRUN test:* lanes (need --run harness; pre-existing, unrelated to this lane).
- `python3 tools/validate_repository.py` -> FAIL backlog exhaustion (pre-existing stale backlog errors; not repaired per scope).
- `git diff --check` -> clean. Hashes unchanged (both frozen sha256 match). Test-file diff RED->HEAD empty.
- Survivors: `pgrep -f "registry_batch|phase1_shell|dispatch_batch"` empty. vm_stat pages-free rose 8249 -> 22877 across runs; host responsive; runs sequential.

## Broad-lib 4 failures classification (reproduced, cheap, no test edits)
1. `executor::tests::execute_success` (executor.rs:489 `assert!(result.success)`) - stale: generic `execute` now denies shell (executor.rs:134-143 returns fixed denial), so a `bash` "echo hello" fixture can never succeed. Stale intent, unrelated to batch change.
2. `executor::tests::execute_timeout` (executor.rs:510 expects "timed out") - same stale cause: shell denied before any spawn/timeout path.
3. `mcp_spawn::tests::disc111_t04_crash_bounded_retry_no_silent_restart` (mcp_spawn.rs:752 `SpawnFailed("No such file or directory")`) - environment: fixture program `/bin/false` absent on this macOS (`ls /bin/false` missing; `/bin/sh` present).
4. `registry_dispatch::tests::disc103_t05_batch_respects_max_parallel_bound` (registry_dispatch.rs:601 expects batch bash `success`) - stale: contradicts frozen broker contract (batch shell = denial record, no spawn/store). Known stale per GREEN worklog; frozen t04/t05 encode the denial intent instead.
- All 4 fail identically independent of the Immediate fix (they encode pre-broker or env assumptions); none caused by this diff.

## Decisions
- ACCEPT. Ready for integration proposal. Not merged, not main-pushed, parent acceptance not claimed.

## Remaining
- Commit/push worklog + own ledger row only; verify fetch remote==HEAD clean.
