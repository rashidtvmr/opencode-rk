# TOOL-012 child cancellation integration

Status: child drop/cancellation seam GREEN; TOOL-012 remains BLOCKED and NOT ACCEPTED.
Owner/session: `ses_f3c4de578ffelQv59xDXmOs03B`.
Candidate base: `58e75bc` on `lane/WEB-006-integration`.

## Frozen contract and evidence

- Frozen RED: `crates/tools/tests/shell_tool_cancel.rs`, SHA-256
  `a20570a7345524ea50e7233868871835e2061a7e5a2cb519d29acc25c7847848`.
- At base `1a2e6ba`, `ShellTool::execute` stored a Tokio `Child`, but the command
  did not enable kill-on-drop and `ShellTool::cancel` constructed the async
  `child.kill()` future without polling it.
- The compiling RED invoked a disposable executable script directly with PID and
  sentinel paths as separate argv values. Aborting the execution future left PID
  31070 alive beyond the 500 ms bound; fixture cleanup then contained it.

## Implemented boundary

- `tokio::process::Command::kill_on_drop(true)` binds child lifetime to the
  execution future/tool owner.
- `ShellTool::cancel` now calls synchronous `Child::start_kill()`, so `Drop`
  actually initiates termination even though it cannot await.
- Tokio retains responsibility for reaping the child; the frozen test observes
  PID disappearance before 500 ms and no delayed sentinel side effect.
- No shell command concatenation was added. The production process invocation
  remains direct command plus argv with cleared environment.

## GREEN evidence

All commands used `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1` and
`--test-threads=1`.

- `cargo test -p opencode-rk-tools --test shell_tool_cancel` — 1/1 in 1.12s.
- `cargo test -p opencode-rk-tools shell_tool --lib` — 9/9 in 1.13s.
- Frozen test bytes remained at the exact hash above.
- `python3 tools/validate_repository.py` retained 51 pre-existing backlog
  exhaustion failures; protection-policy checks passed.
- Test state was confined to a temporary directory; no user database, external
  network or real credential was accessed.

## Remaining gaps

- `ShellTool::execute` still reads stdout/stderr to completion before applying
  truncation, so pre-read memory is not bounded by `max_output_mb`.
- The documented internal timeout path is not applied around the real execution;
  the existing timeout unit test exercises a helper rather than the public call.
- The web turn path still uses `ToolExecutor`, not `ShellTool`, so this repair is
  a prerequisite and does not yet prove browser-disconnect child cancellation.
- TOOL-012 therefore remains blocked rather than completed; this is candidate
  integration evidence only.
