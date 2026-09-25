# BRIDGE-PAR-211 scratchpad

claim: BRIDGE-PAR-211 via cc.claim, session ses_par211, status in-progress.
source: crates/opentui-bridge/src/unicode_width.rs:142 clip_to_width(s,max)->(String,usize); :119 line_width.
target: ONE new file crates/opentui-bridge/src/clip_line.rs only. lib.rs/Cargo.toml/unicode_width.rs untouched.
tests: 5 tests in-file (clip_ascii, clip_wide_boundary, pad_ascii, pad_truncates_and_keeps_width, pad_empty_and_zero).
decisions: clip_line delegates .0; pad_line clips then pads spaces using line_width gap; std-only, forbid(unsafe_code), <80 lines.
unknowns: none.
