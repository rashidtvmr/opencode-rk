# APP-012-LIVE-FILE-TOOL-IMPL-W2

## Claim and route
- Task: live turn dispatch for builtin `write`; assigned `@xkiro-gpt-6-luna`, allowlisted under temporary pool.
- Session: `ses_f1e7c958cffecJaX6QSmdYTjN1`; task claim already existed in `tasks/completion/claims.json` with this exact session. Helper refused duplicate claim (expected fail-closed behavior).
- Branch/revision at intake: `lane/APP-012-LIVE-FILE-TOOL-IMPL-W2` / `52807f87c4d9ff9937adb2e1a655acdeb51badbc`.

## Evidence and contract
- Frozen test `crates/server/tests/live_file_tool_dispatch.rs`, SHA256 `0366416c4863940221c74aac08deeeba3c9404cd92ff18c9411d2cab5b570829`; verifies live provider-called `write`, secret-path denial without filesystem side effect, and tool output in the next Responses round.
- Current server path in `crates/server/src/lib.rs`, `TurnStreamStage::Executing`: for enabled non-shell calls it authorizes only `OperationIntent::Tool`, then sends parsed/default `{}` args through generic `ToolExecutor`; this yields `Unknown tool: write` rather than writing.
- Existing `crates/tools/src/file_ops.rs`: `FileOperation::write(path, content)` and `FileTool::execute_authorized` authorizes `OperationIntent::File { action: Write, path }` before I/O and returns failure for Deny/RequireHuman. Use this API, do not authorize generic tool intent or perform file I/O directly.
- Bounds: preserve the turn allowlist, reject malformed arguments/path/content, pass output through `truncate_tool_output`; existing call-round limits remain in force. No new persistent state beyond the turn output/transcript lifecycle; file operation is synchronous within owned turn stream.

## Validation
- Required docs read: `.agents/WORKER.md`, `PLAN.md`, `docs/TDD.md`, `docs/SECURITY.md`, `docs/CONVERGENCE.md`.
- `python3 tools/convergence_gate.py` blocked on existing off-plan completed claims/notes (see command output in handoff); task already delegated; not an implementation authorization gate.
- Initial focused test compiled then failed because emitted `tool_call.arguments` disclosed denied content. The event now redacts arguments for `write` (while internal call/history remains intact); frozen retest passes.
- `rtk env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test live_file_tool_dispatch -- --test-threads=1` — PASS (1 test).
- `rtk env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test agent_loop_turns -- --test-threads=1` — PASS (1 test).
- `rtk env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --lib file_ops -- --test-threads=1` — PASS (5 tests).
- Broader `cargo test -p opencode-rk-tools file_ops` — did not compile the unrelated `mcp_config` test target due to existing `json!` macro errors in `crates/tools/tests/mcp_config.rs:76,87`; no tests altered.
- `rtk git diff --check` — PASS. Frozen test SHA remains unchanged.

## Remaining
Implemented bounded argument handling and direct `FileTool::execute_authorized` dispatch. Verify independent review/claim status; task is candidate GREEN, not parent acceptance. No commit/push yet.
