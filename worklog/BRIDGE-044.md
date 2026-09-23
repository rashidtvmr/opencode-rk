# BRIDGE-044 scratchpad

Claim: session_layout.rs layout-state contracts.
Evidence: opencode a0d9b6c sidebar.tsx:12-103 footer.tsx:52-90 subagent-footer.tsx:17-55+96-127 question.tsx:48-62+355-456. sidebar.rs = feature-plugins, no overlap.
Target: crates/opentui-bridge/src/session_layout.rs only. No cargo run (scope ban). Logical GREEN: bounds via truncate(), clamp_selected(), first-resolve-wins; 7 tests.
Decisions: std only (format!), no lib.rs touch; char-count bounds.
Unknowns: lib.rs wiring left to integrator.
