# TOOL-004: ToolExecutor Batch Execution Enhancement

Status: NOT STARTED. Kind: feature.
Requirements: Enhanced batch execution with order preservation.

## Scope
- File: `crates/tools/src/executor.rs`
- Modify: `execute_batch()` to preserve input order using indexed futures
- Add: 4 additional unit tests for batch execution scenarios

## Deliverable
- `ToolExecutor::execute_batch(calls: Vec<ToolCall>) -> Vec<ToolResult>` using tokio JoinSet
- Concurrent execution with proper task joining
- Results returned in same order as input calls

## Context
- Existing ToolExecutor, ToolCall, ToolResult types in executor.rs
- Requires tokio rt-multi-thread and macros features for `#[tokio::test]`
- Current implementation completes `batch_executes_all` test but loses order

## Observable Contract
- `execute_batch` returns results in same order as input calls
- Timeouts are handled per-call without affecting other executions
- Concurrent tasks run in parallel (verified via timing assertions)
- Failed tasks produce error results, not panics

## 5 Required Tests
1. `batch_executes_all` - All calls execute and produce results
2. `batch_handles_timeout` - Timeout in one call doesn't affect others
3. `batch_preserves_order` - Results match input order
4. `concurrent_execution` - Multiple calls run truly concurrently
5. `timeout_isolated` - Timeout behavior is isolated per call

## Failure States
- JoinSet panic on task panic (recoverable)
- Individual call timeout produces error result (not cancellation)
- Empty batch returns empty results

## Acceptance Criteria
```bash
cargo test -p opencode-rk-tools
cargo check --workspace
```

## Dependencies
- TOOL-001, TOOL-003 (ToolExecutor foundation)

## Implementation Notes
- Use `JoinSet` with `(usize, ToolCall)` pairs to track original indices
- Collect results into pre-allocated Vec at correct indices
- Add tokio test features: rt-multi-thread, macros to Cargo.toml