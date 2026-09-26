# BRIDGE-PAR-150 scratchpad

Claim: BRIDGE-PAR-150 via ses_par150, worklog/BRIDGE-PAR-150.md.
Source evidence:
- packages/tui/src/context/route.tsx:25-42 store+navigate, :44-53 initialRoute, :11-23 variants (read fully).
- crates/opentui-bridge/src/routes_state.rs (read only, no edit).
Target boundary: ONE new file crates/opentui-bridge/src/route_ctx.rs. No lib.rs/Cargo.toml/routes_state.rs/plugin_routes.rs edits. No cargo/commit.
Tests: in-file #[cfg(test)] 6 tests (go/back roundtrip, empty false, back-empty false, name cap, depth cap, chain).
Decisions: fail-closed bool/Option per routes_state.rs style; chars().count caps; stack push prev via mem::replace; MAX_ROUTE 128, MAX_DEPTH 16.
Unknowns: none. Awaiting rustfmt --check.
