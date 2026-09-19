# LANE-MCP-LIVE scratchpad

## Claim
- Task: LANE-MCP-LIVE (RAW_FEATURE 1.4 MCP gap)
- Session: ses_worker_mcp_live
- Owned files: crates/tools/src/mcp_session.rs, crates/tools/tests/mcp_session.rs

## Source evidence
- crates/tools/src/mcp_spawn.rs: house style, SSRF/broker boundary, error taxonomy pattern
- crates/tools/src/mcp.rs: existing McpClient (simulated), JSON-RPC framing patterns

## Design
Typed MCP stdio CLIENT SESSION state machine:
- States: Uninitialized → Initializing → Ready → (CallingTool) → Shutdown/Error
- JSON-RPC id correlation via HashMap<u64, mpsc channel>
- Bounded outbound write queue (queue_cap)
- Tool list cache with FIFO eviction (tool_cap)
- Tool call with timeout + cancellation via tokio::sync::mpsc
- Bounded error taxonomy: SpawnFailed, Protocol, Timeout, ServerExited, Cancelled
- No process spawning — broker composes over real stdio transport later
- Drive against in-memory framed transport (receive/drain_outbound)

## Tests (15/15 GREEN)
- T01: initial state is Uninitialized
- T02: init handshake transitions to Ready
- T03: init error transitions to Error (Protocol)
- T04: sequential request correlation
- T05: interleaved request correlation
- T06: tool call timeout
- T07: cancel pending call
- T08: tool list cache eviction (cap=2, 5 tools → 2 cached)
- T09: server exited error
- T10: spawn failed error
- T11: init timeout
- T12: server cancel notification (advisory, stays Ready)
- T13: list_tools before init rejected (Protocol)
- T14: call_tool before init rejected (Protocol)
- T15: drain outbound

## Hashes
- RED frozen: sha256 c0670fd3 (src) + 1a539e6e (test)
- GREEN: sha256 0b6eda38 (src) + 6aa290eb (test)

## Decisions
- Use tokio::sync::mpsc (bounded=1) per request for response correlation
- pending_tool_list HashSet tracks which request IDs are tools/list (for cache update on response)
- cancel_pending sends Cancelled through channels without clearing receivers; wait_result treats channel close as Cancelled
- no tokio::spawn in receive() — keeps sync tests runtime-free
- FIFO eviction for tool cache: append then truncate to cap

## Remaining unknowns
- Integration with mcp_spawn.rs broker (future lane)
- Real stdio transport framing (JSON lines over stdin/stdout)
