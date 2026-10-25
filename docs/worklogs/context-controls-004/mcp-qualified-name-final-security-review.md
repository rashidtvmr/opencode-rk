# MCP qualified-name final security review

**Verdict: APPROVE**  
**Base:** `fa845813`  
**Reviewed:** `crates/server/src/mcp_context.rs`, `crates/server/src/mcp_legacy_deny_tests.rs`, `crates/server/src/mcp_context_collision_tests.rs`  
**Scope:** Independent source/diff review. No product/test edits, Cargo/build/tests, Git writes, DB, UI, or delegation.

## Evidence

- `mcp_context.rs:23-63`: BLAKE3 identity uses 128-bit/32-hex digest; sanitized ASCII suffix remains within 64 bytes.
- `mcp_context.rs:86-99`: duplicate advertised names fail closed before returning provider tools.
- `mcp_context.rs:47-51,66-79`: legacy name reproduces old 8-hex truncation/suffix bound; legacy `OperationIntent::Tool` changes only `name`, retaining the current description for resource fingerprint matching.
- `mcp_context.rs:103-136`: preapproval and post-approval effect-boundary checks test current and legacy DENY names. Permission request/action and approval intent use only the new name; legacy ALLOW does not grant execution (`run_owner.rs:157-178` checks current request action).
- `mcp_legacy_deny_tests.rs:115-151,153-193`: resource-scoped legacy DENY fingerprint compatibility; legacy ALLOW does not make the current intent denied. `mcp_context_collision_tests.rs:33-85`: known 8-hex collision pair produces distinct new names and respects byte bound.
- Python `hashlib.sha256` manifest generated from current bytes; frozen collision test hash assertion against requested value passed. No tests/build run by reviewer; parent-reported 253 tests not independently verified.

## SHA-256 manifest

| Path | SHA-256 |
|---|---|
| `crates/server/src/mcp_context.rs` | `f39ac724573fcf32d45cdef41ead4a4e88977196be03cf9fa514414d36f9caf1` |
| `crates/server/src/mcp_legacy_deny_tests.rs` | `5c83d4ddf195c9cfcbddd94572bb1be77a346a916b0cc9863e1cf82458297744` |
| `crates/server/src/mcp_context_collision_tests.rs` | `ad8c2501953786d88b60ce7d4b92bcb65fc6b10e139a666c4b0d4e5693085f5f` |
