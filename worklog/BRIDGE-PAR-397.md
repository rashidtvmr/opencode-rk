# BRIDGE-PAR-397 scratchpad (UNCLAIMED: orchestrator owns claims.json, no claim attempted)

- Task: ONE new file `crates/opentui-bridge/src/plugin_slots_full.rs`. No edit to lib.rs/Cargo.toml/plugin_slots.rs. No cargo/commit.
- Truth: `packages/tui/src/plugin/slots.tsx:25-65` register/dispose; `crates/opentui-bridge/src/plugin_slots.rs` SlotRegistry (bounded 64, host enum).
- Boundary: `SlotList { names: Vec<String> }` cap 32, each 64B, methods add/has/len. std-only, forbid(unsafe_code), <80 lines, >=3 tests.
- Tests: add_has_len, rejects_bad_names (empty/oversize/dup), caps_at_32.
- Decisions: add->bool false on empty/oversize/dup/full; has exact match; len passthrough. No is_empty (skip per ponytail, add when caller needs it).
- Verify: `rustfmt --check` only.
