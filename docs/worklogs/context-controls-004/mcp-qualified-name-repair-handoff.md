# MCP qualified-name collision repair handoff

## Package

`context-controls-004/mcp-qualified-name-repair`, base `fa845813c55a579407d322431f2fb6a5034ad676`.

## Changed paths

- `crates/server/src/mcp_context.rs`
- `crates/server/src/mcp_legacy_deny_tests.rs`
- this handoff

The frozen `mcp_context_collision_tests.rs` was not modified.

## Repair

`stable_name` now uses the first 32 hex characters (128 bits) of the BLAKE3
qualified `(server, tool)` digest and a sanitized suffix capped to the 64-byte
ASCII provider-name limit. `legacy_deny_name` reproduces the old
`mcp_{8hex}_{sanitized}` format with the exact old suffix budget of 42 bytes.

`effective_tools` fails closed on any duplicate advertised qualified name before
provider projection. Execution checks current and reconstructed legacy names at
both preapproval and effect boundaries. The legacy check reconstructs a legacy
`OperationIntent::Tool` with the same exact description, preserving persisted
resource-scoped fingerprints. Legacy names are denial identities only; they are
never used as execution identity or authorization.

The new legacy regression module calls the production `is_denied` helper for
resource-scoped legacy DENY and matching legacy ALLOW cases. Its permission-rule
environment override serializes tests and restores the prior process value on
drop before the disposable fixture is dropped. The renamed collision test is
explicitly limited to proving the old alias is replaced by distinct current
names; the frozen collision fixture remains untouched.

## Verification recommendation

Provider/build owner should run the focused server lib tests, including the
frozen collision module and `mcp_context::legacy_deny_tests`, then independently
exercise the existing pending-approval exclusion regression. This handoff is
PREVERIFIED source-only; no integrated SHA is claimed here.
