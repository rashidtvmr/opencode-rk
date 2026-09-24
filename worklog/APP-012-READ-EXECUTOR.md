# APP-012-READ-EXECUTOR

- Task/session: `APP-012-READ-EXECUTOR` / `ses_f2e8209d4ffeJv0NexktaSn8bF`.
- Scope: `crates/tools/src/executor.rs` only, plus this scratchpad and the task ledger.
- Claim: ledger transitioned to `in-progress` before edits.

## Source evidence

- `crates/tools/src/executor.rs:97-123`: ordinary `execute` dispatches shell/echo/unknown only.
- `crates/tools/src/executor.rs:125-200`: drafted authorized read path; malformed offset/limit silently default; path accepts empty string; timeout wraps `JoinHandle`.
- `crates/tools/src/file_ops.rs:12-13`: `MAX_READ_BYTES = 64 * 1024`.
- `crates/tools/src/file_ops.rs:176-208`: `execute_authorized` authorizes concrete file path before I/O; fixed failures are currently returned through `FileResult`.
- `crates/tools/src/file_ops.rs:228-256`: bounded range read, UTF-8 conversion, fixed ordinary read failures.
- `crates/server/tests/app012_tool_journey_red.rs:313-380`: frozen authenticated read journey expects exact bounded content.

## Contract

`read` executes only through `execute_authorized` with a supplied `PermissionBroker`; concrete nonempty path required; optional offset/limit must be unsigned JSON integers and fit `usize`; malformed, negative, overflow, or over-cap values fail closed; requested limit is capped at `file_ops::MAX_READ_BYTES`; file I/O runs in `spawn_blocking`; configured timeout applies; ordinary `execute` never executes read. Errors never include path/content/secret data.

## Decisions / unknowns

- Timeout cancellation drops the `JoinHandle`, but does not stop already-running blocking I/O. This is documented in the executor code. The read has a bounded byte budget, so the blocking operation is finite, but timeout is not hard cancellation.
- No test edits or added tests.

## Edits

- Added strict optional unsigned integer parsing. Present `offset`/`limit` values that are negative, fractional, nonnumeric, null, or outside `usize` fail closed instead of defaulting.
- Rejected missing, empty/whitespace-only, and NUL-containing paths before broker/I/O.
- Retained `execute_authorized(call, broker)` as the only read entrypoint. Ordinary `execute` still dispatches no `read` operation.
- Clamped `limit` to `file_ops::MAX_READ_BYTES`; retained `FileOperation::read_with_range` and `spawn_blocking` authorization path.
- Documented timeout behavior: dropping the blocking `JoinHandle` cannot interrupt already-running blocking I/O; the bounded read remains finite, not hard-cancelled.

## Verification

- `rtk rustfmt crates/tools/src/executor.rs`: pass.
- `rtk git diff --check`: pass.
- `rtk env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --lib executor -- --test-threads=1`: exit 0.
- `rtk env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test app012_tool_journey_red -- --test-threads=1`: 1 passed, 0 failed.
- `rtk python3 tools/convergence_gate.py`: blocked by pre-existing repository-wide ledger findings (81; includes off-plan completed rows and APP-012 parent status); not changed.

## Remaining unknowns

- Frozen APP-012 journey covers successful read only; malformed numeric, empty path, denial, and timeout behavior are source-audited but lack frozen assertions in this lane.
- Shared worktree retains unrelated pre-existing modifications in server/file_ops and other APP-012 worklogs; untouched.
