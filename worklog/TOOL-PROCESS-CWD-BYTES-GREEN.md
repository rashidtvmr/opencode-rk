# TOOL-PROCESS-CWD-BYTES-GREEN

## Claim
- Task: TOOL-PROCESS-CWD-BYTES-GREEN, type implementation
- Role: Rust path-byte security implementer
- Session: ses_f2bbf46c1ffeHpY4JN1bjjJeG5
- Route: 9router-th-dsv41-flash-free
- Worktree: /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/lane-tool-process-cwd-bytes-green
- Branch: lane/TOOL-PROCESS-CWD-BYTES-GREEN
- Base: e66ad1e (frozen RED)
- Ledger: claimed in-progress via tools/completion_claims.py; updated to
  `completed` with GREEN evidence (cwd RED 1/1, broker 3/3, hashes unchanged).

## Source evidence
- crates/tools/src/executor.rs:193-231 `prepare_canonical_cwd` (pre-fix) used
  `cwd.to_string_lossy()`/`canonical.to_string_lossy()` for NUL detection and
  byte length, inflating non-UTF-8 OS bytes via U+FFFD expansion.
- crates/tools/src/executor.rs:747 caller (validation before broker).
- crates/tools/src/executor.rs:723-737 non-Unix seam returns
  `ProcessError::UnsupportedPlatform { operation: "execute_authorized_process" }`
  before `prepare_canonical_cwd`, so non-Unix accounting is dead code there.

## Change (exact)
- Added `raw_os_bytes(&OsStr) -> &[u8]` with two cfgs:
  - `#[cfg(unix)]` uses `std::os::unix::ffi::OsStrExt::as_bytes()` (exact
    kernel bytes; stable since Rust 1.0, well under MSRV 1.85).
  - `#[cfg(not(unix))]` uses `OsStr::as_encoded_bytes()` to keep compilation
    portable; this path is unreachable (UnsupportedPlatform precedes it) and
    makes no Windows process-support claim.
- `prepare_canonical_cwd` now uses `raw_os_bytes(cwd.as_os_str())` for the NUL
  check and input byte budget, and `raw_os_bytes(canonical.as_os_str())` for the
  canonical byte budget. Empty check unchanged (`as_os_str().is_empty()`).
- Choice rationale: `as_encoded_bytes()` is NOT used on Unix because its
  self-synchronizing encoding is an unspecified, non-kernel representation; the
  contract requires exact OS byte length, so `as_bytes()` is used on Unix.
- No public API, authorization, cancellation, cleanup, timing, output,
  dependency, test, registry, or server edits.

## Tests (frozen, zero edits)
- `crates/tools/tests/process_cwd_bytes_red.rs` sha256
  `f4eea500a6aedffd6d6a93b4289bf449fabfa73ed3c0e2b1db8ec707c3d65aa7` (unchanged).
- `crates/tools/tests/phase1_shell_broker.rs` sha256
  `ef56f63a5d2d3d0fa99049cdfdf9df0fe69fb6e5c66b33484431603cc855d1e8` (unchanged).

## GREEN evidence

- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p opencode-rk-tools
  --test process_cwd_bytes_red -- --test-threads=1` -> 1 passed; 1 suite.
- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p opencode-rk-tools
  --test phase1_shell_broker -- --test-threads=2` -> 3 passed; 0 failed
  (t01, t02, t03).
- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo check -p opencode-rk-tools`
  -> Finished, no errors (3 pre-existing dead_code warnings, unrelated).
- `git diff --check` clean.
- No child spawned: the RED test holds `ready_rx.await.is_err()` (sender-drop,
  no `ProcessReady` sent) and requires `InvalidCwd` with reason
  `cwd could not be canonicalized` for the 1-byte non-UTF-8 cwd.

## Resources
- Sequential Cargo jobs/threads 2. macOS free pages ~11.7k free +
  ~457.6k inactive + ~0.35k speculative @16 KiB => ~7.7 GiB reclaimable,
  above the 2 GiB OS headroom floor.
- No invalid path created on disk; no child process.

## Known status (not fixed, out of lane)
- Stale executor tests (if any) were not run/fixed; verifier-known status stands.

## Remaining unknowns
- None. Unix is exact; non-Unix is compile-portable dead code behind
  `UnsupportedPlatform`.
