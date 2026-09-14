# TOOL-003 - Tool execution engine

Status: IN PROGRESS. Kind: feature.
Requirements: Synchronous tool execution with timeout support, batch execution for concurrency.

## User-observable outcome

- `ToolExecutor` struct with configurable timeout settings.
- `ToolCall` struct for structured tool invocation (tool_id, name, input, timeout).
- `ToolResult` struct for execution results (output, success, duration_ms, error).
- `execute()` method with timeout handling.
- `execute_batch()` method for concurrent execution.
- 5 unit tests covering success, timeout, failure, batch execution, and duration tracking.

## Source evidence

- `crates/tools/src/registry.rs` - Tool definition patterns.
- `Crates/tools/src/lib.rs` - Module structure.
- `Cargo.toml` - Dependencies: serde_json, tokio with rt-multi-thread, sync, time features.

## Observable contract

- `TimeoutConfig { default_timeout_ms: u64, max_timeout_ms: u64 }` - Default constructor with sensible defaults.
- `ToolCall::new(tool_id, name, input: Value)` - Create a tool call.
- `ToolCall::with_timeout(ms: u64)` - Add optional timeout.
- `ToolResult { tool_id, output, success, duration_ms, error }` - Execution result.
- `ToolExecutor::new()` - Create executor with default config.
- `ToolExecutor::with_timeout_config(config)` - Create with custom config.
- `ToolExecutor::execute(call) -> ToolResult` - Execute single call with timeout.
- `ToolExecutor::execute_batch(Vec<ToolCall>) -> Vec<ToolResult>` - Concurrent execution using tokio JoinSet.

## Failure states

- Timeout: Returns `success: false` with "timeout" error message.
- Unknown tool: Returns `success: false` with "Unknown tool" error.
- Missing required field: Returns `success: false` with descriptive error.
- Process failure: Returns `success: false` with exit status info.

## Acceptance criteria

- `cargo test -p opencode-rk-tools` passes all 5 tests.
- `cargo check --workspace` compiles without errors.
- `cargo fmt` applied.

## Test-first execution

1. RED: Stub file compiled; tests written against missing types.
2. GREEN: Full implementation; tests pass.
3. Evidence: Run tests after implementation.

## Implementation notes

- Uses `tokio::process::Command` for async shell execution.
- Uses `tokio::time::timeout` for timeout enforcement.
- Batch uses `tokio::task::JoinSet` for concurrent execution.
- Supports `bash` and `echo` tools for demonstration.