# BRIDGE-PAR-254 scratchpad

- Claim: BRIDGE-PAR-254 via cc.claim session ses_par254, scratchpad worklog/BRIDGE-PAR-254.md. OK.
- Source evidence:
  - crates/opentui-bridge/src/poll_ticker.rs:8-11 PollTicker {tick:u32, every:u32}; 16-21 new(every) zero->20; 25-30 bump()->bool wrapping_add + tick%every==0.
  - crates/opentui-bridge/src/loop_driver.rs:1 forbid(unsafe_code), LoopState/step pattern (read-only ref).
- Target boundary: ONE new file crates/opentui-bridge/src/loop_driver_full.rs. No edit lib.rs/Cargo.toml/loop_driver.rs/poll_ticker.rs. No cargo/commit.
- Tests: >=4 unit tests in-file covering new default cadence, fire increments frames, non-fire no increment, frames() getter, wrapping.
- Decisions: both fields pub (matches spec literal); std-only; forbid(unsafe_code); keep <100 lines; rustfmt clean.
- Unknowns: none. Module wiring left to integrator (lib.rs untouched per scope).
