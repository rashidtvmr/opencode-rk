# TOOL-014

Status: TODO. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: none (discovered during DISC-002 surface extraction).
Dependencies: none.
Test obligations: TOOL-014-T01, TOOL-014-T02, TOOL-014-T03, TOOL-014-T04, TOOL-014-T05.

## User-observable outcome

A bounded `OutputStore` that retains per-tool execution results
(`ToolOutput: tool_id, output, timestamp, success`) with a hard byte budget.
Supports push, per-tool retrieval, recent-N listing, clear, and size stats.
All retention is bounded by `max_size`; oldest entries are evicted first, so
no unbounded retained output exists.

## Source evidence

- crates/tools/src/output_store.rs - owned file (was stub).
- crates/tools/src/lib.rs:10 - `pub mod output_store;` present.
- ralph.json:2457 - TOOL-014 task record with test obligations
  TOOL-014-T01..T05.
- crates/tools/src/shell_tool.rs - sibling module pattern for structure
  (bounded output, eviction via truncate_to_limit).

## Observable contract

- `OutputStore::new(max_size)` creates an empty store with a byte budget in
  bytes.
- `push(ToolOutput)` appends; if `current_size` exceeds `max_size`, the oldest
  entries are evicted until the budget fits again.
- `get(tool_id) -> Vec<&ToolOutput>` returns all entries for the tool id in
  push order.
- `recent(n)` returns the last `n` entries overall in push order; n larger
  than the store returns everything.
- `clear()` empties the store and resets size accounting to zero.
- `stats() -> (total, size)` returns entry count and total retained bytes.
- `ToolOutput::new(tool_id, output, success)` stamps `timestamp` from the
  system clock (UNIX epoch millis).
- Size accounting is byte-based; all operations are O(n) or better and
  retention is bounded by `max_size`.

## Remaining gaps / unknowns

None.