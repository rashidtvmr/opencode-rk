# TOOL-008

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-001.
Dependencies: none.
Test obligations: TOOL-008-T01, TOOL-008-T02, TOOL-008-T03, TOOL-008-T04, TOOL-008-T05.

## User-observable outcome

Per-tool permission granting and checking with hierarchical levels
(ReadOnly, ReadWrite, Admin), TTL expiry, revocation, and an O(1) cache
for hot-path authorization checks.

## Source evidence

- crates/tools/src/permission.rs:1 - existing stub module, `pub mod permission;` in lib.rs.
- crates/contracts/src/lib.rs:110 - `Timestamp(DateTime<Utc>)` with `now()` and `as_datetime()`.
- crates/tools/Cargo.toml - serde, serde_json, thiserror available as workspace deps.

## Observable contract

- `PermissionLevel` enum with ReadOnly, ReadWrite, Admin deriving Ord for hierarchy checks.
- `ToolPermission` carries tool_id, level, granted_at: Timestamp, expires_at: Option<Timestamp>.
- `PermissionChecker::check(tool_id, level)` returns bool using cache for O(1) lookup; expired permissions deny.
- `grant(tool_id, level, expires?)` replaces or adds; rebuilds cache after mutation.
- `revoke(tool_id)` removes and returns bool; cache entry cleared.
- `list_all()` returns snapshot Vec<ToolPermission>.
- 5 tests: check_read_only, check_read_write, grant_and_revoke, level_hierarchy, cache_updated.
- Bounded: Vec + HashMap, no unbounded queue; prune_expired() provides maintenance path.

## Remaining gaps / unknowns

None.
