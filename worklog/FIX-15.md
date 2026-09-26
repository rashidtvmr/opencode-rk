# FIX-15

## Claim
- Owner: `ses_f26e12aa4ffdv02iD1EILI9sAw`
- Scope: `crates/opentui-bridge/src/mouse_drag_full.rs`

## Source evidence
- `crates/opentui-bridge/src/input.rs:43-49`: mouse events carry `u16` cell coordinates.
- `crates/opentui-bridge/src/input_events.rs:145-164`: SGR parser preserves click coordinates.
- `crates/opentui-bridge/src/native_input.rs:48-52`: mouse currently maps to `Noop`.
- `crates/opentui-bridge/src/world.rs:104-120`: canvas transforms use saturating integer math.

## Contract
- `Idle -> Pressed -> Dragging`; stationary movement remains `Pressed`; release resets `Idle`.
- Node hit testing uses half-open rectangles and saturating edges.
- Wheel zoom changes by 25 percentage points, clamped to 25..=400.

## Verification
- `rustfmt --check crates/opentui-bridge/src/mouse_drag_full.rs` -> PASS.
- File length: 105 lines; in-file tests: 4.
- No cargo, commit, or integration wiring run in this lane.

## Remaining
- Integrator must register the module in `lib.rs` and connect parsed mouse events.
