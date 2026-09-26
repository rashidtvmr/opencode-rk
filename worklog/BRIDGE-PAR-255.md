# Claim BRIDGE-PAR-255
- session: ses_par255
- task: BRIDGE-PAR-255, file crates/opentui-bridge/src/focus_wire_full.rs
- source: crates/opentui-bridge/src/focus_wire.rs:1-70 (FocusWire ring router, fail-closed None)
- target boundary: FocusFull {zone,count} + focus/zone_of/count, std-only, forbid unsafe, <100 lines, >=4 tests
- decisions: default zone chat count 0; focus accepts chat|palette|sidebar only, len>32 reject, saturating count; no lib.rs edit
- tests: default_is_chat, focus_palette_ok, invalid_rejected, cap_32_enforced, count_tracks_switches
- verification: rustfmt --check only
