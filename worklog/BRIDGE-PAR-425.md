# BRIDGE-PAR-425 (unclaimed, file-only per orchestrator)

Lease: one file `crates/opentui-bridge/src/side_mcp_todo_full.rs`. No claims.json touch. No lib.rs/Cargo.toml. No commit.

Claim: sidebar MCP names + todo count, TS `sidebar/mcp.tsx:1-30` (connected/bad dots over mcp list), `sidebar/todo.tsx:1-30` (visibility gate, TodoItem rows).
Source evidence: `todo_item_full.rs:1-80` (pattern: forbid unsafe, char caps, line/summary fns, inline tests).
Target boundary: struct SideMcpTodo {mcp cap16/64, todos u32} + add_mcp + bump_todo + summary cap256. std-only, forbid(unsafe_code), <90 lines, >=4 tests.
Tests: empty_summary, add_and_summary, mcp_capped_at_16, name_capped_64_and_summary_256, bump_saturates (5 tests).
Decisions: saturating_add for todos; summary format `mcp:N todos:M names`; chars() caps (unicode-safe, matches TodoItem pattern); ponytail: no per-item status.
Unknowns: wiring into lib.rs left to orchestrator.
