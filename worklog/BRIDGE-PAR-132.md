# BRIDGE-PAR-132 scratchpad

claim: in-progress ses_par132 worklog/BRIDGE-PAR-132.md
source evidence:
- crates/opentui-bridge/src/plugin_slots.rs:16-29 SlotName (12 host slots), :80-82 SlotRegistry entries Vec, :101-134 register/get/unregister (do NOT edit)
- packages/tui/src/plugin/api.ts:1-52 createPluginRoutes register/dispose closure, no slot/mount vocabulary found (grep slot|mount|owner zero hits)
observed: sibling plugin_slots.rs covers host SlotName registry; this lane adds compact 4-kind mount/unmount registry
target boundary: ONE new file crates/opentui-bridge/src/slot_registry.rs, no lib.rs/Cargo.toml/plugin_slots.rs edits, no cargo/commit
tests: 5 in-file (mount ok, dup false, cap 32, unmount missing false, owners filter)
decisions: bool-return mount/unmount per spec (not Result); owner cap 64 bytes fail-closed; slots cap 32 fail-closed; std-only forbid(unsafe_code)
remaining: rustfmt --check only per scope
