# BRIDGE-PAR-212 scratchpad
claim: BRIDGE-PAR-212 via ses_par212, scratchpad worklog/BRIDGE-PAR-212.md
evidence: crates/cli/src/tui_entry.rs:508 pad idiom `while lines.len() < height.saturating_sub(3)`; crates/opentui-bridge/src/native_frame.rs:62 `body_rows=height.saturating_sub(7)`, :70-72 pad idiom, :76 truncate; crates/opentui-bridge/src/paint_full.rs:58,71-73 same
boundary: ONE new file crates/opentui-bridge/src/frame_pad.rs; no lib.rs/Cargo.toml edits; no cargo/commit
impl: fit_height truncates then pads "" to exact height min 1; body_rows saturating_sub(7); forbid(unsafe_code), std-only
tests: 5 (pad, truncate, zero-clamp x2 asserts, body_rows chrome, body_rows saturate)
verify: rustfmt --check only (see FMT_EXIT above)
