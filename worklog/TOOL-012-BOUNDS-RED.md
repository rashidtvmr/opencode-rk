# TOOL-012 bounds RED

Status: BLOCKED RED; timeout behavior missing in public `ShellTool::execute`.
Owner/session: `ses_f3740df3dffeYEpTFVcSVtHd1u`.
Candidate base: `fba8686` on `lane/WEB-006-integration`.

## Source evidence

- `crates/tools/src/shell_tool.rs:154-255`, `ShellTool::execute` reads both pipes with `read_to_end`, truncates only after reads, then awaits `child_ref.wait()`; `ShellConfig::timeout_secs` and builder `ShellTool::timeout_secs` are not applied.
- `crates/tools/src/shell_tool.rs:206`, `kill_on_drop(true)` supplies child cleanup when the outer watchdog drops the execution future.
- `crates/tools/src/shell_tool.rs:277-286`, `with_timeout` exists but is not used by public execution.
- `worklog/TOOL-012-CANCEL-INTEGRATION.md:44-47`, records the same remaining timeout and pre-read output-bound gaps.
- `crates/tools/tests/shell_tool_cancel.rs:72-117`, frozen direct-executable argv/PID/sentinel cancellation fixture pattern.

## Contract and proposal

Direct executable scripts receive PID/sentinel or output paths through argv; no shell command concatenation. Public execution must return `Err(ShellError::Timeout(1))` near one second, kill/reap the child within 500ms, and prevent delayed sentinel creation. Effective timeout: minimum of positive builder/config limits. Wrap concurrent bounded pipe reads plus child wait in `tokio::time::timeout`; on timeout return `ShellError::Timeout`, relying on owned kill-on-drop. Read each stream via `AsyncReadExt::take(limit + 1)` before `read_to_end`; retain only the bounded result plus visible truncation marker. No detached readers/tasks.

## RED tests

- `crates/tools/tests/shell_tool_bounds.rs`: public timeout/cleanup test and bounded stdout/stderr regression.
- Required command: `env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --test shell_tool_bounds -- --test-threads=1`.
- Expected RED: timeout watchdog elapses because public execute lacks internal timeout; bounded output test passes.

## Remaining unknowns

Implementation must preserve cancellation/reaping semantics while making timeout cover both pipe reads and wait. No RSS/OOM test attempted.

Frozen test SHA-256: 140179dc85ee5cb57685205ab33b2242554a6b1ea3fa0828eac3ed23b735b0d2
