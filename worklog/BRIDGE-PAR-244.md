# BRIDGE-PAR-244 scratchpad

claim: BRIDGE-PAR-244 via cc.claim, session ses_par244 — ok.
source: crates/opentui-bridge/src/message_router.rs:1-59 (MAX_MESSAGE_ID=64, MessageRoute single pending action; TS truth, read-only).
target: ONE new file crates/opentui-bridge/src/message_router_full.rs. No lib.rs/Cargo.toml/message_router.rs edits. No cargo/commit per task scope (overrides WORKER.md s5 for this lane).
design: RouterFull { routes: Vec<String> (cap 32), delivered: u64 }; add truncates 64, false on dup/full; route truncates, true + wrapping_add(1) when known; delivered getter; Default/new; forbid(unsafe_code); std-only; <110 lines; 5 tests.
tests: add_ok, add_dup_false, add_cap_32, route_known_counts, route_unknown_false.
verify: rustfmt --check only.
