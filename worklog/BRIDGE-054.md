# BRIDGE-054 theme_registry

Claim: TS color math + registry port (theme.rs Rgba/Theme reuse).
Source: TS `packages/tui/src/theme/index.ts` @a0d9b6c (`color.rs` ansi table pre-verified).
Target: `crates/opentui-bridge/src/theme_registry.rs` only; `lib.rs` wiring left to integrator.
Tests: 12 (`cargo test` NOT run per scope ban; hand-computed GREEN vs TS literals).
Decisions: digits-only spec -> ANSI (TS numbers); hex/transparent/none literal; gray-scale extreme-bg only + `muted_text_color(u8 lum, split@128)`; no subscribe listeners (TS-side); registry cap 64.
Unknowns: none blocking; needs `pub mod theme_registry;` + `cargo test -p opentui-bridge theme_registry`.
