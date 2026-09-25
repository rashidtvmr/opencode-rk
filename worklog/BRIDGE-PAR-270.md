# BRIDGE-PAR-270 scratchpad

Claim: HeaderFlow render counter over TS-truth header_lines. session ses_par270.
Source: crates/opentui-bridge/src/session_header.rs:13 title_or_id, :23 header_lines (read-only truth).
Boundary: ONE new file crates/opentui-bridge/src/session_header_full.rs. No lib.rs/Cargo.toml/session_header.rs edits. No cargo/commit.
Tests: 5 in-file (starts_zero, bumps_once+truth-match, bumps_each_call, counts, width).
Decisions: saturating_add for renders; Default delegates to new; ponytail: no width cache, add when header() costly.
Unknowns: none. Integrator prewires `pub mod session_header_full`.
