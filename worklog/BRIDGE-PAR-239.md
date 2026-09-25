# BRIDGE-PAR-239 scratchpad

claim: BRIDGE-PAR-239 ses_par239 in-progress.
source: packages/tui/src/keymap.tsx:20-22 LEADER_TOKEN/base/COMMAND_PALETTE_COMMAND; crates/opentui-bridge/src/keymap_dispatch.rs KeymapDispatch/dispatch/bind; crates/opentui-bridge/src/keymap_default.rs:11 default_keymap 7 binds.
target: ONE file crates/opentui-bridge/src/keymap_full2.rs, no lib.rs edit. KeymapFull{map,leader cap16}+set_leader+dispatch_label(key)->Option<String>(command)+count.
tests: 6 in-file.
decisions: dispatch_label returns command clone (per "lookup command"); leader trunc chars 16; new() seeds default_keymap.
unknowns: none.
