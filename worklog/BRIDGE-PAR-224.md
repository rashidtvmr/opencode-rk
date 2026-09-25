# BRIDGE-PAR-224 scratchpad

Claim: BRIDGE-PAR-224 via ses_par224, scratchpad worklog/BRIDGE-PAR-224.md.
Source evidence: none local (TS truth: SolidJS Show/For, model only); new file only.
Target boundary: ONE new file crates/opentui-bridge/src/solid_show_for.rs. No lib.rs/Cargo.toml edits. No cargo/commit.
Tests: 5 in-file (show true/false/cap, for_each map/cap). Verification: rustfmt --check only.
Decisions: std-only, forbid(unsafe_code), truncate by chars (4KiB), for_each cap 64 items. Helper truncate fast-paths short strings by byte len.
Status: file written, rustfmt pending.
