# WEB-009 disconnect cancellation

Task: WEB-009 disconnect sub-contract only. Parent remains NOT ACCEPTED for durable tool/reference/accessibility gaps.
Session: ses_f337cf349ffeUMaIvRBsbGGRuL
Base: 9ded2b9 (TOOL-012 startup API)

## Source evidence

- `crates/server/src/lib.rs:863-888`: `TurnStreamState` owns the live provider/tool state and turn permit.
- `crates/server/src/lib.rs:1223-1299`: `Executing` currently creates request-local `ToolExecutor` and executes every allowed call through it.
- `crates/tools/src/shell_tool.rs:163-175`: `ShellTool::execute_with_startup` accepts readiness path plus bounded startup sender.
- `crates/server/tests/runtime_wiring_disconnect_http.rs:389-440`: frozen real HTTP disconnect contract; response body drop must reclaim provider, permit, shell parent/descendant, and avoid durable tool/assistant writes.

## Contract

Allowed `bash`/`shell` calls receive one broker authorization, execute through a constant `bash -c` wrapper with separate command/path argv, publish one validated startup event before client-visible tool call, retain at most `MAX_CALLS_PER_ROUND` owned guards, and abort/await/clean up on stream drop or normal completion. Non-shell execution remains `ToolExecutor` behavior.

## Tests

Frozen WEB-009 disconnect hash: 95fbe83a0361256d9e0c229cfc97f3cc0b222e8c6d89625ac5db582ce18199e8.
RED/GREEN commands and receipts appended below.

## GREEN receipt (ses_f3c4de578ffelQv59xDXmOs03B)

`ShellExecutionGuard` holds a directly owned `Pin<Box<dyn Future<Output = Result<ShellResult, ShellError>> + Send>>`
produced by `ShellTool::execute_with_startup`. No `tokio::task::JoinHandle`, no spawn, no abort token.

- `wait_for_startup`: `tokio::select!` polls the startup `mpsc::Receiver<u32>` and the pinned execution
  future concurrently. On startup readiness, the pending future is retained back in `self.execution`.
  On early completion (before readiness), the result is stored in `self.result`.
- `finish`: consumes `self`; drains `self.result` if set, else awaits `self.execution`.
- Drop path: guard drops -> pinned future drops -> `ShellTool` drops (owned by future) -> `ShellTool::Drop`
  calls `self.cancel()` -> `kill_process_group` on the child's process group. `ShellArtifact` cleanup
  follows. Stream drop reclaims permit and terminates shell parent/descendant processes.
- Removed all `eprintln!` debug prints from the prior stopped session.
- Retained: `SHELL_STARTUP_WRAPPER` argv, `ShellArtifact` readiness dir, single broker authorize per
  call, `MAX_SHELL_COMMAND_BYTES`/`MAX_SHELL_ERROR_BYTES`/`MAX_CALLS_PER_ROUND` bounds, non-shell
  `ToolExecutor` path unchanged.

Command:
```
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test runtime_wiring_disconnect_http -- --test-threads=1
```
Result: 1 passed, 0 failed. 208/208 server lib tests pass (no regression).

## Remaining gaps

Durable tool/reference persistence and accessibility producer/UI contracts remain outside this disconnect lane; parent WEB-009 remains unaccepted.
