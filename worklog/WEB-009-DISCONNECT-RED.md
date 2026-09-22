# WEB-009 disconnect RED

Status: BLOCKED. Independent test-author lane. Candidate base `30fabf9` / current lane revision before this test.

## Contract

`crates/server/tests/runtime_wiring_disconnect_http.rs` drives the real loopback HTTP streaming endpoint with a daemon `RuntimeWiring` whose `EnginePolicy` allows `bash` and `OPENCODE_RK_TURN_TOOLS=bash`. A loopback provider emits one Responses function call. The command safely quotes disposable fixture paths, records parent and descendant PIDs, waits before writing a delayed sentinel. A browser-like TCP client reads through `tool_call`, then closes both directions.

Assertions cover bounded PID publication, provider connection closure, shared permit return, parent/descendant death, absent sentinel, and no durable tool/assistant success. Cleanup signals only recorded fixture PIDs.

## Source evidence

- `crates/server/src/lib.rs:924-1455`, `create_turn_stream`: request-local `ToolExecutor`; no disconnect cancellation hook.
- `crates/server/src/lib.rs:1223-1298`, `TurnStreamStage::Executing`: executes `ToolExecutor::execute` directly.
- `crates/tools/src/executor.rs:124-171`, `execute_shell`: `bash -c` child awaited under timeout; dropping the future does not kill the process tree.
- `crates/server/src/runtime_wiring.rs:584-589`, `try_acquire_turn`: owned permit returns on stream drop, independent of tool-process cleanup.
- `crates/server/src/turn_parts.rs:169-182`: pure state-machine disconnect hook exists, but live HTTP stream does not use it.

## RED evidence

Frozen test compiled. Exact command:

`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test runtime_wiring_disconnect_http -- --test-threads=1`

Result: FAIL, one test. Failure:

`crates/server/tests/runtime_wiring_disconnect_http.rs:419:5: tool parent survived browser disconnect`

This is the intended missing live-path behavior. The test reached the real `tool_call` event, recorded fixture PIDs, returned the shared turn permit, observed the provider response connection close, then detected the recorded parent still live. No product code or existing test changed.

SHA-256 frozen test:

`crates/server/tests/runtime_wiring_disconnect_http.rs` SHA-256: `95fbe83a0361256d9e0c229cfc97f3cc0b222e8c6d89625ac5db582ce18199e8`.

Remaining unknown: the failure short-circuits before final descendant/sentinel/durable assertions; cleanup runs only against recorded fixture PIDs.
