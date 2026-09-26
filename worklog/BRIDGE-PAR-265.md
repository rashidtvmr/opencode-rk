# BRIDGE-PAR-265 scratchpad

Claim: BRIDGE-PAR-265 via ses_par265, scratchpad worklog/BRIDGE-PAR-265.md.
Source evidence: crates/opentui-bridge/src/keymap_dispatch.rs:40-71 (KeymapDispatch bind/dispatch/unbind); crates/opentui-bridge/src/keymap_full2.rs:1-47 (wrapper pattern, do-not-edit).
Observed scenario: no hit/miss counting wrapper exists.
Target boundary: ONE new file crates/opentui-bridge/src/keymap_dispatch_full.rs only. No lib.rs/Cargo.toml/keymap_dispatch.rs/keymap_full2.rs edits. No cargo, no commit.
Tests: 5 in-file (hit, miss, mixed accumulate, default zero, unbind-then-miss).
Decisions: std-only, forbid(unsafe_code), saturating_add counters, Option<String> clone of command. 109 lines.
Remaining: integrator prewires `pub mod keymap_dispatch_full;`.
