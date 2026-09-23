# BRIDGE-031 sidebar.rs

Claim: sidebar panel state.
Evidence: TS a0d9b6c sidebar/files.tsx:14-52, context.tsx:13-47, lsp.tsx:7-47, mcp.tsx:7-79, todo.tsx:8-31, footer.tsx:9-80. Crate style spinner.rs/selection.rs (`#![forbid(unsafe_code)]`, MAX bound, tests).
Target: crates/opentui-bridge/src/sidebar.rs (created). lib.rs NOT touched (scope).
Tests: 5 (empty nav, clamp, bound, reset, shapes). No cargo run per scope; logically green.
Unknowns: lib.rs wiring left to owner; no visual test.
