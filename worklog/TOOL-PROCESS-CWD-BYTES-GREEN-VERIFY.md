# TOOL-PROCESS-CWD-BYTES-GREEN-VERIFY

## Claim
- Task: TOOL-PROCESS-CWD-BYTES-GREEN-VERIFY, type verification
- Role: Rust process-lifecycle/capability seam verifier
- Session: ses_f2bbd2bf0ffeltT7dyVSweyZMH
- Worktree: /private/var/folders/b0/dj81nc_j2kq2bkmg0yd2sgyc0000gn/T/opencode/verify-tool-process-cwd-bytes-green
- Branch: verify/TOOL-PROCESS-CWD-BYTES-GREEN
- Candidate revision under test: 1fcd3838e2a849eadfa4220c3d298750e99c52b3
- Accepted contract: c630f3f (plan/TOOL-SHELL-CANCEL-CONTRACT)
- Frozen broker-test hash: ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8
- Frozen cwd-RED hash:   f4eea500a6aedffd6d6a93b4289bf449fabfa73ed3c0e2b1db8ec707c3d65aa7
- Claim issued before any edits.

## Scope
Independent verification only. No source repair, no cancellation RED, no server/registry
changes, no test/refreeze edits, no self-acceptance. One owned file: this worklog;
one other edit: this task's own row in tasks/completion/claims.json.

## Raw-byte repair audit (exact diff of the seam revision)
Base = e66ad1e (frozen RED). Revision under test = 1fcd3838.
Only one product file changes: crates/tools/src/executor.rs (+21 -3).

1. New helper:
   ```rust
   #[cfg(unix)]
   fn raw_os_bytes(value: &std::ffi::OsStr) -> &[u8] {
       use std::os::unix::ffi::OsStrExt;
       value.as_bytes()
   }
   #[cfg(not(unix))]
   fn raw_os_bytes(value: &std::ffi::OsStr) -> &[u8] {
       value.as_encoded_bytes()
   }
   ```
   - Unix uses `OsStrExt::as_bytes`: exact bytes handed to the kernel (stable
     since Rust 1.0; MSRV 1.85 satisfied). No U+FFFD expansion.
   - Non-Unix uses `as_encoded_bytes` for compilation only; the seam returns
     `UnsupportedPlatform` before preparation, so it is unreachable and makes
     NO Windows-support claim.

2. prepare_canonical_cwd now uses raw_os_bytes for all three prior defects:
   - NUL check: `raw_os_bytes(cwd.as_os_str()).contains(&0)` (was `cwd.to_string_lossy().contains('\0')`)
   - input byte budget: `raw_os_bytes(cwd.as_os_str()).len() > max_cwd_bytes` (was `cwd.to_string_lossy().len()`)
   - canonical byte budget: `raw_os_bytes(canonical.as_os_str()).len() > max_cwd_bytes` (was `canonical.to_string_lossy().len()`)
   - Empty check unchanged: `cwd.as_os_str().is_empty()`.
   - canonicalize call unchanged, exactly once.

No public API, authorization, cancellation, cleanup, timing, output, dependency,
test, registry, or server edits. Diff stat confirms only executor.rs + claims.json +
the implementer's own worklog.

## Frozen hashes (shasum -a 256, unchanged)
- crates/tools/tests/process_cwd_bytes_red.rs:
  f4eea500a6aedffd6d6a93b4289bf449fabfa73ed3c0e2b1db8ec707c3d65aa7  (matches task expectation)
- crates/tools/tests/phase1_shell_broker.rs:
  ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8  (matches task expectation)

## Command results (sequential, sole Cargo lane, threads=2)
- `rtk shasum -a 256 ...process_cwd_bytes_red.rs ...phase1_shell_broker.rs`
  -> hashes above; both match expected. Exit 0.
- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p opencode-rk-tools
   --test process_cwd_bytes_red -- --test-threads=1`
  -> cargo test: 1 passed (1 suite, 0.00s).  RED 1/1. PASS.
- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p opencode-rk-tools
   --test phase1_shell_broker -- --test-threads=2`
  -> cargo test: 3 passed (1 suite, 0.00s).  Broker 3/3. PASS.
- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo check -p opencode-rk-tools`
  -> 0 errors, 6 warnings (pre-existing: unused imports in security, unused_mut
     in registry_dispatch, unused_assignments in rtk_core, dead_code in shell_tool).
     No warning originates in the raw_os_bytes / prepare_canonical_cwd repair.
     PASS.
- `rtk git diff --check` -> exit 0, no output. PASS (clean tree, no whitespace issues).
- `rtk git status` -> only untracked worklog + claims row changes; working tree
  otherwise clean against the seam commit.

## Reconciliation of prior seam verifier evidence (2699dc7)
Prior independent verifier (worklog/TOOL-AUTHORIZED-PROCESS-SEAM-VERIFY.md) verdict
was BLOCKED, with the sole failing GAP being:
  GAP: prepare_canonical_cwd used `cwd.to_string_lossy().len()` at lines 202,
       207, 220 (input NUL/length and canonical length), inflating non-UTF-8 OS
       bytes via U+FFFD expansion. No non-UTF-8 path probe was runnable on macOS.

All other prior probes PASSED and were NOT regressions:
- Public API names/shapes, exact request/limits types, ProcessCancellation = watch.
- Authorization-before-spawn, Deny/HumanRequired no-side-effect, grant single-use.
- Unix process group + group-kill + explicit Child::wait + reader joins.
- UnsupportedPlatform early return on non-Unix.
- Startup-timeout branch executable (1ns fixture -> TimedOut + Reaped).
- Output cap+marker, reader-failure cleanup, readiness-receiver-closed policy.
- env_clear + fixed PATH, no inherited secret exposure.

The raw-byte defect is now repaired at 1fcd3838: the three to_string_lossy().len()
and contains('\0') call sites are replaced by raw_os_bytes() on Unix, returning
exact kernel bytes. The frozen cwd RED test (which asserts a 1-byte 0xff cwd fits a
1-byte budget and then fails canonicalization with reason "cwd could not be
canonicalized", no spawn, no broker audit side effect) now passes 1/1.

Deterministic checkpoint observations were NOT demonstrated violations:
- Cancellation-after-auth-before-spawn timing tie: recorded as an observation
  (no deterministic public checkpoint to force both values simultaneously), NOT a
  demonstrated failure of seam correctness.
- Descendant fixture: not run on macOS; recorded as a probe gap, not a defect in
  the seam code (source uses process_group(0) + kill_process_group per contract).
- Stale executor unit tests (execute_success, execute_timeout) remain untouched
  and fail identically to the pre-change baseline; not regressions, not edited.

## No-child / no-spawn confirmation
- The frozen cwd RED asserts ready_rx.await.is_err() (sender-drop, no ProcessReady
  sent) and InvalidCwd canonicalization failure for the 1-byte non-UTF-8 cwd.
  Because cwd validation precedes broker/authorization and spawn, no child can be
  created on an invalid cwd.
- No child process observed during verification (no `sleep`/`yes`/`echo` spawns).
- pgrep sweep for stray children of this test process returned empty (no survivors).

## Memory bounds
vm_stat snapshot at verification time (page size 16384):
- Pages free ~5079, inactive ~446240, speculative ~83
  => ~511 MiB free + ~7.0 GB inactive reclaimable, far above the 2 GiB OS headroom
     floor. Sequential single-lane Cargo jobs/threads=2; no concurrent heavy builds.
No memory pressure observed.

## Authorization boundary (exact)
ACCEPT. The repaired seam revision 1fcd3838 satisfies the accepted seam contract
(c630f3f) for the cwd raw-byte invariant and the broader public-API/pre-spawn
contract verified at 2699dc7. Frozen RED 1/1 and broker 3/3 GREEN hold at this
exact revision with unchanged frozen test hashes.

This ACCEPT authorizes ONLY:
- Authoring of a SEPARATE compiling behavioral cancellation RED
  (plan/TOOL-SHELL-CANCEL-CONTRACT -> crates/tools/tests/phase1_shell_cancellation.rs),
  which must compile against the public seam and fail at behavioral assertions.

This ACCEPT does NOT authorize:
- Cancellation implementation/acceptance (separate implementation lane).
- Independent cancellation verification (separate verifier lane).
- Windows process-tree backend or any Windows process support claim.
- Server/registry wiring changes.
- Any edits to frozen tests or the existing broker RED.
- Self-acceptance of the cancellation lane.

The seam's compile-only non-Unix path behind UnsupportedPlatform is confirmed
portable and makes no Windows-support claim.

## Remaining gaps / out-of-scope for this verifier lane
- Windows process-tree proof absent (out of scope; seam is Unix-gated).
- No descendant fixture run on macOS (platform fixture limitation, not a seam defect).
- Server wiring (new_shell_execution) and dispatcher default-deny remain separate lanes.

## Landing
- git diff --check: clean (exit 0).
- Frozen test hashes unchanged (verify-only; no test edits).
- Commit scope: this worklog + own ledger row only. No source/test/probe edits.
- Branch verify/TOOL-PROCESS-CWD-BYTES-GREEN will be pushed without force; remote
  containment check confirms 1fcd3838 is present on origin/lane/TOOL-PROCESS-CWD-BYTES-GREEN
  (merge-base with that lane = seam tip).
