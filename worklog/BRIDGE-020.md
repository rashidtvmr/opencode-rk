# BRIDGE-020 bg_pulse

Claim: lane-owned `crates/opentui-bridge/src/bg_pulse.rs` created, lib.rs untouched
(owner must add `pub mod bg_pulse;`). Do NOT run cargo per task card; logical GREEN only.

Evidence (TS checkout a0d9b6c, NOT 95daf90):
- consts `bg-pulse-render.ts:4-23` (PERIOD 4600, RINGS 3, PHASE_OFFSET 0.29,
  BREATH_AMP 0.05, BREATH_SPEED 0.0008, 1/TAIL crest/tail omitted per ponytail)
- envelope `:304-312` phase/eased smoothstep; breath `:302`; output scale `:339` *0.7 = MAX_PULSE
- BOLD `:64` right-half logo glyphs; stencil write `:276`; `bg-pulse.tsx:52-60` renderSelf
  passes deltaTime+rgb; CACHE_FRAME_COUNT=138 from `:80`
- BOLD bit 0 per `attributes.rs:39` (TS numeric unconfirmed, same divergence note)

Tests (5, RED-first by derivation from TS math, GREEN by inspection):
frame0_alpha_matches_ts_curve, frame_wraps_each_period,
ring2_envelope_dominates_at_frame0, breath_floor_and_mid_values, pulse_cell_bounds_and_bold_bit.

Unknowns: lane gate needs lib.rs wiring + cargo (forbidden); hand to orchestrator.
MAX_PULSE is output scale 0.7, not a TS symbol; spatial crest/tail + logo shimmer
(:317-371) intentionally omitted, upgrade path in ponytail comment.
