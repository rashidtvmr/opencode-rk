# TOOL-PROCESS-CWD-BYTES-RED

## Claim
- Task: TOOL-PROCESS-CWD-BYTES-RED, type RED authoring
- Role: independent Rust security regression test author
- Session: ses_f2bc6c5b6ffeG2rVOWH3DyOkAA
- Route: 9router-oc-muse-spark-1-3-contributor-free
- Worktree: /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/red-tool-process-cwd-bytes
- Branch: red/TOOL-PROCESS-CWD-BYTES
- Candidate: c15c09f957aa81adf409d84f2d92a93b93fce9be
- Ledger: claimed in-progress via tools/completion_claims.py

## Source evidence
- crates/tools/src/executor.rs:193-231 `prepare_canonical_cwd`: cwd budget/NUL checks use `cwd.to_string_lossy().len()` (lines 202, 207) and `canonical.to_string_lossy().len()` (line 220), not raw OS bytes.
- crates/tools/src/executor.rs:713-722 public `execute_authorized_process` signature; validation before broker at 744-747.
- crates/tools/src/executor.rs:212-214 canonicalize failure maps to `ProcessError::InvalidCwd { reason: "cwd could not be canonicalized" }`.
- Seam verifier worklog commit 2699dc7 GAP: lossy byte ceiling on non-UTF-8 paths is implementation risk, no runtime probe run.
- Accepted cancellation contract c630f3f: normative public seam names/shapes.

## Observed scenario
- Non-UTF-8 cwd `OsString::from_vec(vec![0xff])` is 1 raw OS byte; `to_string_lossy()` expands to U+FFFD (3 UTF-8 bytes).
- With `max_cwd_bytes=1`: correct raw-byte accounting admits the budget check, then `canonicalize` fails (path never created) with reason `cwd could not be canonicalized`. Current lossy accounting returns `cwd exceeds max_cwd_bytes`.
- No invalid path created on disk; no child spawned; validation precedes broker so authorizer audit stays empty.

## Target boundary
- Owned file: crates/tools/tests/process_cwd_bytes_red.rs (one external integration test, Unix-guarded).
- No executor/source/dependency/existing-test edits. No test edits after freeze.

## Tests
- `cwd_budget_counts_raw_os_bytes_not_lossy_utf8`: constructs non-UTF-8 1-byte cwd, max_cwd_bytes=1, calls public `execute_authorized_process` with /bin/echo, asserts InvalidCwd reason == "cwd could not be canonicalized", readiness sender dropped without send, authorizer audit empty.

## Decisions
- Single behavioral test; boundary UTF-8 case not needed (deterministic single assertion suffices).
- `#![cfg(unix)]` guard; no Windows runtime claim.

## Remaining unknowns
- None for RED scope. Implementer must switch to raw OS byte length (`as_encoded_bytes().len()`) for both input and canonical checks plus NUL check on raw bytes.
