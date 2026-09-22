# TOOL-012 process-tree cancellation RED

Status: ready for delegated implementation. Reclaimed by integration session
`ses_f3c4de578ffelQv59xDXmOs03B` after the prior RED author stopped, then
released after dependency pre-wiring.

## Source evidence

- `783b19f crates/tools/src/shell_tool.rs:187-208` — `ShellTool::execute` spawns the direct executable with `kill_on_drop(true)`, but does not establish a process group.
- `783b19f crates/tools/src/shell_tool.rs:264-282` — cancellation/drop uses direct-child `start_kill()` and discards the child; no descendant-tree signal.
- `783b19f crates/tools/src/shell_bounds.rs:169-189,254-261` — process-group/tree precedent uses `process_group(0)` plus negative-PGID signaling, but belongs to a separate sync bounds module and is not wired into `ShellTool`.
- `783b19f crates/tools/tests/shell_tool_cancel.rs:72-117` — frozen direct executable, separate argv PID/sentinel paths, bounded polling and containment pattern.

## Contract / boundary

The new Unix Tokio test creates a disposable executable script. The script records parent and background descendant PIDs to separate argv paths, waits for the descendant, then writes a sentinel. `ShellTool::execute` receives the exact script path in its allowlist and argv; no shell command is concatenated by the test. The test aborts and awaits the owned `JoinHandle`, then requires both PIDs dead within 750ms and no delayed sentinel after a bounded wait. `FixtureGuard` explicitly sends `SIGKILL` to both recorded PIDs only as failure containment.

## RED

Command: `env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --test shell_tool_process_tree -- --test-threads=1`

Observed RED: command compiled, then failed semantically at `crates/tools/tests/shell_tool_process_tree.rs:117` after 0.78s: `process-tree cancellation leaked parent PID 37827 or descendant PID 37828`. The parent was already absent (`kill: 37827: No such process`); descendant PID 37828 remained alive. Fixture guard killed both PIDs before cleanup.

Frozen test SHA-256: `f7f2e3a49e4125943c17f366911dbaa4ff18b0e69424bcb1584c76ab39809a27` (`shasum -a 256 crates/tools/tests/shell_tool_process_tree.rs`).

## Implementation blocker

Resolved by the user's instruction to resolve all blockers: promote the already
locked `rustix 1.1.4` package to a direct workspace dependency with its safe
`process` feature. This crate still forbids unsafe; direct-child `start_kill`
cannot target a negative process-group ID, and no blocking or detached
`/bin/kill` workaround is permitted.

## GREEN implementation evidence

- `crates/tools/src/shell_tool.rs` now creates a Unix process group with Tokio's
  safe `Command::process_group(0)`, stores the leader PGID, and uses
  `rustix::process::kill_process_group(..., Signal::KILL)` on cancellation/drop.
- Non-Unix retains Tokio's direct-child `start_kill` behavior via `cfg`.
- Focused tests passed sequentially with `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`:
  `shell_tool_process_tree` (1 passed), `shell_tool_cancel` (1 passed),
  `shell_tool_bounds` (2 passed).
- Frozen test hash unchanged: `f7f2e3a49e4125943c17f366911dbaa4ff18b0e69424bcb1584c76ab39809a27`.

## Remaining unknowns

An approved native process-group primitive must be selected and wired by the implementation lane. This test intentionally owns no product code.
