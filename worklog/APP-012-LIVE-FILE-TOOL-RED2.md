# APP-012-LIVE-FILE-TOOL-RED2

## Claim

- Session: `ses_f1e82e6afffe1jJD5WeWFPt2Ou`
- Branch: `red/APP-012-LIVE-FILE-TOOL-W2`, base `ecec045`
- Owned file: `crates/server/tests/live_file_tool_dispatch.rs`
- Status: in-progress (RED test-author lane)

## Source evidence

- `crates/server/src/lib.rs:1183-1215` advertises only registry tools admitted by `OPENCODE_RK_TURN_TOOLS`.
- `crates/server/src/lib.rs:1539-1561` authorizes generic tools and then invokes `ToolExecutor`; currently an enabled `write` call reaches the executor and returns `Unknown tool: write`.
- `crates/server/src/lib.rs:1611-1643` persists tool outputs and emits tool events, so denial must be visible without a successful-write transcript.
- `crates/server/tests/agent_loop_turns.rs:45-270` supplies bounded disposable app, scripted TCP provider, HTTP server, NDJSON, and transcript helpers; this test copies/adapts those helpers locally.

## Contract and bounds

With `OPENCODE_RK_TURN_TOOLS=write`, a Responses provider requests `write` with an absolute disposable `.env` path and fake content. The live endpoint must route it through the broker, emit a recognizable denial (`denied` or `secret-bearing`), never emit `Unknown tool: write`, never create `.env`, and feed denial to exactly one scripted second provider round. The fake provider accepts at most two bounded requests; request bodies are capped at 256 KiB and socket reads at 20 seconds.

## RED evidence

Test authored in `live_file_tool_dispatch.rs`; no product or existing test files changed. Run the focused target with one build/test job and a timeout. Expected RED on current implementation is the broker/secret denial assertion because current dispatch returns generic executor `Unknown tool: write`.

## Remaining

Compile command passed. Genuine RED passed as expected: focused test failed at line 310 because output was `Unknown tool: write` instead of broker denial. Test SHA256 was recorded after RED. Ledger status is blocked with exact missing behavior; commit and push lane branch follow.
