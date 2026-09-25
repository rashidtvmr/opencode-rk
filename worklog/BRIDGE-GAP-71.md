# BRIDGE-GAP-71 scratchpad
claim: BRIDGE-GAP-71 ses_gap71
source: scrollback.surface.ts (RunScrollbackStream retained-surface tail window, commitRows tail), run_scrollback.rs:16 Scrollback rows/CAP 2000
target: crates/opentui-bridge/src/run_scrollback_surface.rs only
tests: window_tail, offset_shifts, up_saturates, down_saturates, reset_zero, height_caps_at_200
decisions: offset=rows-up-from-bottom; visible clamps offset to len; height capped 200 in new(); scroll_up saturating_add, scroll_down saturating_sub
unknowns: none
