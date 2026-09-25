# BRIDGE-PAR-222 solid_effect_full

claim: ses_par222 in-progress
source: crates/opentui-bridge/src/signal_graph.rs:76-79 run_effects counter model; TS truth SolidJS createEffect idiom (model only)
boundary: ONE file crates/opentui-bridge/src/solid_effect_full.rs; no lib.rs/Cargo.toml/signal_graph.rs edits; no cargo/commit
tests: 5 in-file (drain, full-reject, truncation, counts-across-flushes, wrapping)
decisions: wrapping_add for ran; drain(..) flush; cap 16/256
verify: rustfmt --check PASS; 108 lines <110; forbid(unsafe_code); std-only
