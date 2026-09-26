# BRIDGE-PAR-241 scratchpad

Claim: BRIDGE-PAR-241 ses_par241 worklog/BRIDGE-PAR-241.md owned.
Source: crates/opentui-bridge/src/editor_bridge.rs:10-19 trunc char-boundary, EditorBridge open/close; editor_ctx.rs:35-42 request_open bool pattern, MAX_FILE_BYTES 512.
Boundary: ONE new file editor_full2.rs. No lib.rs/Cargo.toml/editor_bridge.rs/editor_ctx.rs edits. No cargo/commit.
Target: EditorFull {open,path cap512,lines cap1024 each 4KiB} + open_at/insert->bool + close + line_count.
Tests: 6 in-file (new_closed_empty, open_empty_false, open_sets_path_clears, path_truncates, insert_closed_false, insert_caps).
Decisions: std-only, forbid(unsafe_code), trunc helper copied pattern, insert requires open + MAX_LINES gate, open_at clears lines.
Unknowns: none. Integrator must add pub mod editor_full2.
