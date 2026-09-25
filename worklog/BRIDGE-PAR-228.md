# BRIDGE-PAR-228 scratchpad

claim: BRIDGE-PAR-228 ses_par228 worklog/BRIDGE-PAR-228.md OK (in-progress).
source: crates/opentui-bridge/src/rule_line.rs:10-16 (RULE_CAP=120, rule_line floors 1) + clip_line.rs:13-22 (pad_line clip+space-pad display width).
target: ONE new file crates/opentui-bridge/src/border_cursor_full.rs, forbid(unsafe_code), std-only, <110 lines.
impl: border_top->rule_line; border_frame pads each line width.min(RULE_CAP), wraps |bars|, take(64); cursor_mark line chars take(256)+U+2588, cap 257.
tests: 7 in-file (top_delegates, frame_bars_pad, frame_clips, frame_caps_rows, frame_empty, cursor_appends, cursor_caps).
verify: rustfmt --check exit 0; wc -l 87. No cargo per scope. lib.rs/Cargo.toml untouched.
