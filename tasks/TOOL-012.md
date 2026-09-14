# TOOL-012

Status: TODO. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-001.
Dependencies: none.
Test obligations: TOOL-012-T01, TOOL-012-T02, TOOL-012-T03, TOOL-012-T04, TOOL-012-T05.

## User-observable outcome

A `ShellTool` abstraction and `ShellConfig` that execute shell commands with
bounded output, environment control, timeout, working-directory support and
cancellation, returning a structured `ShellResult`. Includes permission-aware
command allowlisting via `allowed_commands`.

## Source evidence

- crates/tools/src/shell_tool.rs - owned file (stub).
- crates/tools/Cargo.toml - tokio with rt, process available.
- crates/tools/src/lib.rs: `pub mod shell_tool;` present.

## Observable contract

- `execute(config)` spawns a Tokio child process, collects stdout/stderr up to
  `max_output_mb`, enforces `timeout_secs`, respects `cwd` and `env`.
- `allowed_commands` is an allowlist; commands not permitted return
  `ShellError::CommandNotAllowed`.
- `ShellResult` carries `success`, `stdout`, `stderr`, `duration_ms`,
  `exit_code: Option<i32>`.
- Drop of an in-flight `ShellTool` aborts the child process.
- All operations are bounded; no unbounded queues or retained output.

## Remaining gaps / unknowns

None.
