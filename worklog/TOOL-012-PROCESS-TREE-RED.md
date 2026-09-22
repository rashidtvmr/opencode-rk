# TOOL-012 process-tree cancellation RED

Status: blocked; RED-only prerequisite. Owner/session: `ses_f372ed281ffegQ8MpCgpsZgGnC`.

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

BLOCKED pending an approved safe process-group signal primitive/dependency. This crate forbids unsafe; direct-child `start_kill` cannot target a negative process-group ID; blocking or spawned `/bin/kill` in `Drop` would violate async ownership and no-detached-process rules. No bypass proposed.

## Remaining unknowns

An approved native process-group primitive must be selected and wired by the implementation lane. This test intentionally owns no product code.
