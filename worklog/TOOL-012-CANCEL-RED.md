# TOOL-012 cancellation RED

Status: blocked; RED-only prerequisite. Claim held by `ses_f374740d1ffelJzHNdPdMt8D4Y`.

## Source evidence

- `1a2e6ba crates/tools/src/shell_tool.rs:79-90` — `ShellTool` owns an optional Tokio `Child`.
- `1a2e6ba crates/tools/src/shell_tool.rs:154-208` — `execute` consumes the tool, spawns the direct command, stores the child.
- `1a2e6ba crates/tools/src/shell_tool.rs:255-273` — `cancel` calls async `child.kill()` without polling its future; `Drop` calls `cancel`, then discards the child.
- `1a2e6ba tasks/TOOL-012.md:22-31` — contract requires bounded execution and child abortion on drop.

## Contract / boundary

The test invokes an executable fixture directly, with PID and sentinel paths as separate argv values. It aborts the real in-flight `ShellTool::execute`, waits for child death via `/bin/kill -0`, then waits beyond the fixture delay and asserts no sentinel. PID parsing, polling, waits, and output are bounded. Fixture `Drop` sends explicit `SIGKILL` only as containment after assertions/failure; it is not the behavior under test.

## RED

Command: `env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --test shell_tool_cancel -- --test-threads=1`

Observed RED: test compiled, then failed semantically at `crates/tools/tests/shell_tool_cancel.rs:103` after 0.53s: `child PID 31070 remained alive after cancellation`. The test exited before the delayed sentinel assertion; fixture containment guard issued explicit `/bin/kill -KILL 31070` during cleanup. No permission, allowlist, executable-mode, or spawn failure occurred.

Frozen test SHA-256: `a20570a7345524ea50e7233868871835e2061a7e5a2cb519d29acc25c7847848` (`shasum -a 256 crates/tools/tests/shell_tool_cancel.rs`).

## Minimal implementation proposal

Set `Command::kill_on_drop(true)` before spawn, plus synchronous `start_kill()` in `Drop`/cancel. This seam does not claim timeout or pre-read output bounds are fixed; those remain blockers.
