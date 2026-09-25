# BRIDGE-PAR-257 scratchpad

- claim: BRIDGE-PAR-257 via ses_par257, OK
- source: crates/opentui-bridge/src/paint_adapter.rs:14-50 (PaintAdapter new/build_frame, cap 256)
- target: crates/opentui-bridge/src/paint_adapter_full.rs only; no lib.rs/Cargo.toml/paint_adapter.rs/paint_callsite.rs edits; no cargo/commit
- tests: caps_applied, bumps_counter, delegates_frame, empty_placeholder, saturates
- decisions: std-only, forbid(unsafe_code), saturating_add for renders, delegate via PaintAdapter::new+build_frame
- unknowns: none; module wiring left to orchestrator (lib.rs untouched per scope)
