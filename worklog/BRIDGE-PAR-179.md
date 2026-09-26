# BRIDGE-PAR-179 scratchpad
- claim: BRIDGE-PAR-179 / ses_par179 / worklog/BRIDGE-PAR-179.md (CLAIMED)
- source: crates/opentui-bridge/src/keymap_dispatch.rs:23-42 (KeyAction, KeymapDispatch, bind/dispatch); packages/tui/src/keymap.tsx:20-22 (COMMAND_PALETTE_COMMAND=command.palette.show)
- target: ONE new file crates/opentui-bridge/src/keymap_default.rs (do NOT touch lib.rs, Cargo.toml, keymap_dispatch.rs, keymap.rs)
- API: default_keymap()->KeymapDispatch (7 prebinds) + describe(&KeymapDispatch)->Vec<String> (sorted, cap 64)
- tests: 6 in-file (palette, all-prebinds, labels, sorted, cap64, fallthrough)
- verify: rustfmt --check only (no cargo)
