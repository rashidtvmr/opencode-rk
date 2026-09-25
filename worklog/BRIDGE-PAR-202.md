# BRIDGE-PAR-202 scratchpad
- claim: BRIDGE-PAR-202 via cc.claim session ses_par202, ok
- source evidence: crates/cli/src/tui_entry.rs:428 native_terminal_size()->(u32,u32) default (80,24); tui_entry.rs:466-467 width.max(20)/height.max(8); mirrored crates/opentui-bridge/src/native_frame.rs:56-57, paint_full.rs:51-52
- target boundary: ONE new file crates/opentui-bridge/src/size_clamp.rs only; no lib.rs/Cargo.toml/runtime_ctx.rs edits
- tests: clamp mins, u16 saturation, zero clamps to min, size_changed eq/ne
- decisions: std-only, forbid(unsafe_code), u32 in / u16 out (u16::MAX saturate for crossterm SetSize), size_changed is != (ponytail: no epsilon, exact cell compare)
- unknowns: none
