# BRIDGE-PAR-229

Claim: ses_par229, scratchpad worklog/BRIDGE-PAR-229.md.
Source: packages/tui/src/plugin/adapters.tsx:1-60 (Slot/register/dispose pattern); crate::slot_registry::SlotRegistry (mount/unmount/owners_of, SlotKind) - read only; sibling plugin_adapters.rs (MAX_ADAPTERS cap style), plugin_slots.rs SlotRegistry.
Boundary: ONE new file crates/opentui-bridge/src/plugin_adapter_full.rs. No lib.rs/Cargo.toml/plugin_routes.rs/slot_registry.rs edits. No cargo/commit.
Target: PluginAdapter {reg: SlotRegistry, mounted: Vec<String> cap 16} + mount/unmount/mounted_list. std-only, forbid(unsafe_code), <120 lines.
Tests: 6 in-file (mount_ok, dup, empty, cap16, unmount-missing, remount). rustfmt --check PASS. 113 lines.
Decisions: mounts tracked on SlotKind::Status; mount fail-closed rolls back (no push when reg rejects, e.g. >64B owner); cap enforced before reg call.
Unknowns: none. lib.rs prewire left to integrator.
