# TOOL-001

Status: IN PROGRESS. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-001.
Dependencies: none.
Test obligations: TOOL-001-T01, TOOL-001-T02, TOOL-001-T03, TOOL-001-T04, TOOL-001-T05.

## User-observable outcome

A `ToolRegistry` exposing built-in core tools (bash, grep, file, read, write, edit)
with id/name/tag discovery, enable/disable lifecycle, and a count accessor.

## Source evidence

- crates/tools/src/registry.rs:1 - existing stub module, `pub mod registry;` in lib.rs.
- crates/tools/Cargo.toml - serde_json, thiserror available as workspace deps.

## Observable contract

- Register returns stored tool retrievable by id.
- disable/enable toggle `enabled` and return bool for found/unknown.
- count includes all registered (built-in) tools.
- find_by_tag returns only tools tagged with the given tag.
- get_by_name resolves name to the matching tool or None.
- No unbounded queue; HashMap-backed, all operations O(1)/O(n) over tool count.

## Remaining gaps / unknowns

None.
