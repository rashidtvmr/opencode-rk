# BRIDGE-PAR-249 scratchpad

claim: BRIDGE-PAR-249 owned file crates/opentui-bridge/src/scroll_step_full.rs, session ses_par249.
source evidence: crates/opentui-bridge/src/scroll_step.rs:16-21 ScrollStep velocity/last_ms accumulator, no offset/total.
observed scenario: no absolute offset tracker exists; scroll_step.rs covers per-tick accel only.
target boundary: ScrollFull {offset, total} + set_total clamp + step clamp 0..=total-view + offset(). No lib.rs/Cargo.toml/scroll_step.rs edits.
tests: 6 in-file (new_zero, set_total_clamp, step_down_max, step_up_zero, view>total, both_dirs).
decisions: set_total clamps offset to total (view unknown there); step uses saturating_sub + isize clamp. std-only, forbid(unsafe_code).
remaining unknowns: none.
