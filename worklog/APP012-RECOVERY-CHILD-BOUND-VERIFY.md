# APP012-RECOVERY-CHILD-BOUND-VERIFY

## Claim
- Task: `APP012-RECOVERY-CHILD-BOUND-VERIFY`
- Role: Independent verifier (read-only; never edit product or tests)
- Session: `ses_verify_app012_child_bound`
- Candidate revision: `3ad58b0554aecb79080064a7fa70b0071b1ba2a1`
- Branch: `verify/APP012-RECOVERY-CHILD-BOUND`
- Owned file: `worklog/APP012-RECOVERY-CHILD-BOUND-VERIFY.md` (only)

## Authority and source evidence
- Read `.agents/WORKER.md`, `AGENTS.md`, `PLAN.md`, `docs/TDD.md`,
  `docs/SECURITY.md`, `docs/CONVERGENCE.md`, `docs/storage/CRASH_CONSISTENCY.md`.
- Frozen child RED test: `crates/storage/tests/app012_recovery_child_bound_red.rs`.
  SHA-256 `fe78660842335e79e33787cb95bb675a80c65e541b3bd52fa9e92d1efc9138b0`.
- Frozen restart test: `crates/storage/tests/app012_restart_resume_red.rs`.
  SHA-256 `d2745fe142fb1add3cc26c9fe9715438c73a51084d5498373c990c0a8e2a8cce`.
- Implementation: `crates/storage/src/facade.rs` (owned by GREEN lane,
  `APP012-RECOVERY-CHILD-BOUND-GREEN`). Verified matches candidate `3ad58b0`
  (no diff at HEAD).
- SQL inspection targets: `crates/storage/src/facade.rs:148-233` (recovery
  function), `crates/storage/schema/v2/workspace.sql:144-212` (execution/
  attempt/tool state columns and recovery indexes).

## Verification boundaries
- Verifier NEVER edits product code or tests. Only reads and runs.
- Under global Cargo semaphore: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`.
- Disposable fixtures only (existing tests use `tempfile::tempdir()`).
- Must preserve/check frozen hashes before/after.

## Targets to verify (from task card)
1. child target 2/2: `cargo test -p opencode-rk-storage --test app012_recovery_child_bound_red -- --test-threads=1`
2. restart target 18/18: `cargo test -p opencode-rk-storage --test app012_restart_resume_red -- --test-threads=1`
3. storage --lib: `cargo test -p opencode-rk-storage --lib`
4. storage --tests: `cargo test -p opencode-rk-storage --tests`
5. cargo check: `cargo check -p opencode-rk-storage`
6. rustfmt check: `rustfmt --edition 2021 --check crates/storage/src/facade.rs`
7. diff check: `git diff --check`
8. lane_gate --run: `python3 tools/lane_gate.py --json --run`

## Verification results

### 1. Child target 2/2
`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-storage --test app012_recovery_child_bound_red -- --test-threads=1`
Result: `2 passed; 0 failed; 0 ignored` (0.22s after 21.58s compile)

### 2. Restart target 18/18
`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-storage --test app012_restart_resume_red -- --test-threads=1`
Result: `18 passed; 0 failed; 0 ignored` (0.28s after 0.33s compile)

### 3. Storage lib 122/122
`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-storage --lib`
Result: `122 passed; 0 failed; 0 ignored` (2.07s)

### 4. Storage --tests 207/207
`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-storage --tests`
Result: 207 total tests across 13 targets, all passed (122 lib + 2 child + 18
restart + 65 other integration tests), 0 failed. 5+3+8+2 pre-existing warnings
in test files, unrelated to leased code.

### 5. Cargo check
`CARGO_BUILD_JOBS=1 cargo check -p opencode-rk-storage`
Result: pass (0 errors). Only pre-existing CLI manifest warning
(`main.rs` in multiple bin targets).

### 6. Rustfmt check
`rustfmt --edition 2021 --check crates/storage/src/facade.rs`
Result: pass (no output, exit 0)

### 7. Diff check
`git diff --check`
Result: pass (no output, exit 0). No uncommitted changes to tracked files.

### 8. Lane gate
`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 python3 tools/lane_gate.py --json --run`
Result: all 12 lanes PASS (8 source module lanes + 4 test lanes).

### Frozen hash preservation
Pre-run and post-run SHA-256 verified identical:
- child test `fe78660842335e79e33787cb95bb675a80c65e541b3bd52fa9e92d1efc9138b0` (matches)
- restart test `d2745fe142fb1add3cc26c9fe9715438c73a51084d5498373c990c0a8e2a8cce` (matches)

## SQL inspection (boundedness, terminal immutability, later-open drainage,
no replay, transaction ownership)

Inspection of `crates/storage/src/facade.rs:148-233` (`recover_existing`):

- **Transaction ownership**: one `Immediate` transaction opened at line 149,
  committed at line 231. All recovery work is fenced inside this single
  transaction: owner_generation CAS (lines 160-168), recovery root seed (lines
  173-194), child updates (lines 195-222), root state conversion (lines 223-229),
  temp table drop (line 230). If the CAS at line 166 fails, the function returns
  an error and the transaction is rolled back (no partial state).

- **Boundedness**: `STARTUP_RECOVERY_LIMIT = 500` (line 18). Root selection
  (line 193) and both child update scans (lines 207, 221) are all capped with
  `LIMIT ?2`/`LIMIT ?1` bound to this constant. No unbounded Rust vector is used;
  the root set is materialized in a TEMP table with at most 500 rows (line 173-177).

- **Terminal immutability**: child UPDATEs target only states `(0,1)` (open/
  dispatched) for provider_attempts (line 203) and `(0,1)` for tool_calls (line
  217). Terminal states 2 (success), 3 (failure), 5 (abandoned) for attempts and
  2/3/4 for tools are excluded from the `WHERE a.state IN (0,1)` / `t.state IN
  (0,1)` predicates. Root UPDATE at lines 223-229 only targets `state=1` (running).
  No terminal execution (states 2/3/5) is touched. Test
  `startup_does_not_rewrite_terminal_states` confirms.

- **No replay/insertion**: recovery performs only UPDATEs (lines 160-161, 195-229).
  No INSERTs occur in the recovery transaction. No SELECT-from-outbox-replay logic.
  Test `consumed_approval_state4_blocks_cross_restart_replay` confirms replay
  fence.

- **Later-open drainage**: each startup open selects at most 500 prior-generation
  roots from states `(0,1,4)` (line 183). Uncertain roots (state 4) are included
  if they still own open children (lines 184-190). After root conversion to state 4
  (lines 223-229), a subsequent open re-evaluates the same SQL with the root now
  in state 4 and its children already converted. The child scan predicate
  `a.state IN (0,1)` will not match already-uncertain children, so no double-
  processing. Residual open children beyond the 500 row limit drain on the next
  open, as proven by `recovery_does_not_strand_children_after_bounded_root_scan`
  (second open asserts 0 open, 501 uncertain) and
  `startup_recovery_is_bounded_and_drains_across_passes`.

- **TEMP table**: `DROP TABLE recovery_roots` at line 230 before commit ensures no
  state leaks across transactions or connections. The TEMP table is connection-
  local and does not persist in the schema.

## Process survivors
`ps aux | grep -E "cargo|rustc|app012"` post-run: no survivors found.
Only pre-existing npm/wrangler process noted in GREEN worklog remains (unrelated).

## Warnings
5 warnings in lib test build (unused mut, unused variable, dead code) - all in
pre-existing code outside `facade.rs`. 8 warnings in stress_v2 integration tests,
2 in backup_v2, 1 in catalog_v2, 1 in writer_v2 - all pre-existing test-file
lints, not in leased files.

## Status
**ACCEPT-SCOPED candidate verification.** This verifies only bounded recovery
child drainage at `3ad58b0`; it does not accept APP012, merge to `origin/main`,
or close generation, owner-lock, approval, runner, or installed-journey gaps.
