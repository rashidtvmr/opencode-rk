# BRIDGE-PAR-134 keymap_dispatch

Claim: cc.claim BRIDGE-PAR-134 ses_par134 worklog/BRIDGE-PAR-134.md OK (in-progress).
Source evidence:
- TS truth /home/rashid/projects/opencode/packages/tui/src/keymap.tsx:1-80 (OpencodeKeymapProvider/useKeymap, mode stack, layer fields; no sessionBindingCommands/preventDefault/fallthrough matches - grep 0 hits).
- Style model crates/opentui-bridge/src/focus_wire.rs:1 (`#![forbid(unsafe_code)]`, doc-boundary pattern).
Target boundary: ONE new file crates/opentui-bridge/src/keymap_dispatch.rs. No lib.rs/Cargo.toml/keymap.rs/keymap_full.rs edits. No cargo/commit.
Tests: 6 in-file #[cfg(test)] (bind/dispatch, overwrite, missing none, unbind, cap-full, truncate/empty-key).
Decisions: Vec<(String,KeyAction)> linear scan (cap 64, std-only); trunc() char-based caps; dup overwrite true; empty key false; dispatch None = fallthrough.
Unknowns: none.
