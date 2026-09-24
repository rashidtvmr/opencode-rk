# APP-012-TOOL-RED

## Claim
- Task/session: `APP-012-TOOL-RED` / `ses_f2ec3a38fffer2ivp1cowx22WB`.
- Owned path: `crates/server/tests/app012_tool_journey_red.rs`.
- Scope: real authenticated HTTP/router/storage/provider loop RED; no product edits.
- Claimed before file edits via `tools/completion_claims.py`.

## Source evidence
- `crates/server/src/lib.rs::TurnStreamStage::Executing` advertised enabled
  tools and performed a generic tool-level broker check, then called
  `ToolExecutor::execute`.
- `crates/tools/src/executor.rs::ToolExecutor::execute` supported only
  shell/bash and echo, producing `Unknown tool: read`.
- `crates/tools/src/file_ops.rs::execute_authorized` authorized writes only;
  reads and lists bypassed the concrete file intent.

## Observed scenario
- To establish: disposable workspace + disposable DB; loopback scripted provider round 1 requests bounded `read`; HTTP router advertises enabled tool; broker authorizes concrete path; real bounded file op; durable tool result; round 2 exact result; final stream; restart/rebuild; outside/secret denial persists and feeds back.

## Target boundary
- Test must fail for current missing wiring (`ToolExecutor` unknown `read` and/or broker bypass), not compile/harness failure.
- Bounded provider rounds/captures, loopback only, no host/user files/secrets, no shell, no mock executor, serial test.

## Tests
- Frozen SHA-256:
  `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`.
- Integrated RED command: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test
  -p opencode-rk-server --test app012_tool_journey_red -- --test-threads=1`;
  compiled and failed `0/1` with `Unknown tool: read` versus expected bounded
  output `0123456789ab`.
- GREEN on the unchanged frozen file: same command, `1 passed, 0 failed`.
- Focused file-operation regression: `cargo test -p opencode-rk-tools --lib
  file_ops -- --test-threads=1`; `5 passed, 0 failed`.

## Decisions
- Product source and existing/frozen tests remain untouched.
- Capture source line evidence, SHA-256, exact command/failure, then mark ledger `blocked` with RED-ready-for-implementation.
- Live reads parse a concrete path and optional byte offset/limit, cap output at
  64 KiB, run synchronous file I/O in `spawn_blocking`, and authorize the path
  with `OperationIntent::File { Read, path }` after the existing tool-level
  authorization. Broker denial performs no file open/read.
- Tool outputs remain bounded by the server's existing transcript truncation
  before provider replay and durable persistence.

## Remaining unknowns
- This test proves durable persistence in the active app instance, not a rebuilt
  router/runtime after restart.
- A separate frozen RED is still required for protected `.env`/outside-root
  denial and zero-I/O behavior before tightening the current read policy.
- Human-gated approval/resume remains unwired; the current turn returns a
  terminal tool error for `RequireHuman`.
