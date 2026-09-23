# WEB-009 disconnect implementation

Status: blocked

Source evidence: `crates/server/src/lib.rs:create_turn_stream` owns the HTTP
stream state and drops it when the response body disconnects. Its `Executing`
stage previously awaited `ToolExecutor::execute`; that executor uses
`tokio::process::Command::output`, whose dropped future does not own a process
group cancellation boundary. `crates/tools/src/shell_tool.rs:ShellTool` owns a
process group and kills it on cancellation/drop.

Contract: a response-body disconnect drops the in-flight tool execution and
provider stream, returns the turn permit, persists no tool/assistant success,
and leaves successful connected turns unchanged. Only `crates/server/src/lib.rs`
is product-owned here. Frozen test hash remains
`95fbe83a0361256d9e0c229cfc97f3cc0b222e8c6d89625ac5db582ce18199e8`.

Initial GREEN attempt compiled but fixture PID publication failed: the shell
path did not reach the fixture before the deadline. Investigating while keeping
the existing outer `PermissionBroker` authorization boundary.

Architectural blocker: `crates/server/src/lib.rs` sees only the opaque
`ShellTool::execute` future. Its first poll reaches spawn, but cannot robustly
guarantee child scheduling and fixture-start publication before the
client-visible `tool_call`; stream drop must still retain process-group
ownership for synchronous cleanup. A tools-layer spawned/cancellable handle or
explicit start handshake is required. Arbitrary sleep is not acceptable.

Failed command/runtime: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p
opencode-rk-server --test runtime_wiring_disconnect_http -- --test-threads=1`.
Runtime approximately 4.02 seconds. Failure: `tool fixture PID publication
deadline exceeded`. No product diff remains; only this evidence scratchpad and
the blocked ledger row are changed. Frozen test hash remains
`95fbe83a0361256d9e0c229cfc97f3cc0b222e8c6d89625ac5db582ce18199e8`.
