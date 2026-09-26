# BRIDGE-PAR-309 scratchpad

Claim: BRIDGE-PAR-309, session ses_par309.
Source: TS truth bg-pulse-render.ts (4600ms 3-ring pulse, read fully); sibling bg_pulse.rs (envelope) + spinner_full.rs (stepper pattern).
Boundary: ONE new file crates/opentui-bridge/src/bg_pulse_full.rs. No lib.rs/Cargo.toml edit. No cargo/commit.
Target: BgPulse{step:u8} + tick wrapping 0..8 + level + bar(width) # repeat scaled, cap 64. std-only, forbid unsafe, <90 lines, >=4 tests.
Tests: new_starts_zero, tick_wraps_eight, bar_scales_with_level, bar_caps_64.
Decisions: ponytail fixed 8-step bar; envelope wiring left to bg_pulse.rs pulse_alpha.
Unknowns: none.
