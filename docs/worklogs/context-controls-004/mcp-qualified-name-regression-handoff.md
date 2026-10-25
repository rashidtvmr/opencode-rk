# MCP Qualified-Name Collision Regression Handoff

## Base SHA
`0b8cee50c16d27907db47bcbef32de07c91c66b0` (feat(context): persist acknowledged native context selections)

## Artifact Summary
RED regression added (no production fix). Two new files:
- `crates/server/src/mcp_context_collision_tests.rs` (new test module)
- `crates/server/src/mcp_context.rs` -- appended `#[cfg(test)] #[path] mod collision_tests;` hook at END (existing logic unchanged).

## Test Identity
`legacy_8hex_collision_is_reproduced_and_current_name_distinguishes_pair`

## Deterministic RED Reproduction
- Scans server IDs `fixture-server-{i}` for `i in 0..250_000`, identical ASCII tool `probe`.
- Reconstructs LEGACY 8-hex identity verbatim: `blake3("{server}\0{probe}").to_hex()[..8]` (byte array `[u8;8]`).
- Records first legacy collision in a bounded `HashMap<[u8;8], String>` (~30 MB / ~1 s peak).
- Asserts `stable_name(first_server, "probe") != stable_name(second_server, "probe")`.
- On current 8-hex `stable_name` impl (mcp_context.rs:23-28): names COLLIDE => test FAILS (RED).
- Boundary assertion: long non-ASCII tool produces a `mcp_..` name capped at `MAX_TOOL_NAME_BYTES=64`.

## Expected RED State
Test fails on collision assertion under the current 8-hex truncation. No production behavior changed.

## Parent Follow-on (approval scope)
1. Freeze test hash.
2. Fix `stable_name` (longer digest within 64-byte ASCII + explicit duplicate rejection).
3. Independent review.

## Hash Readback
- Hook (`mcp_context.rs` tail): `0b8cee50` parent unchanged; hook added lines 139-141.
- Test file (`mcp_context_collision_tests.rs`): new, 0 lines pre-existing.
