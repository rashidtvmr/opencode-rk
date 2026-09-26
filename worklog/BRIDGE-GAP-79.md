# BRIDGE-GAP-79: run theme pick
Claim: ses_gap79 via completion_claims.claim.
Evidence: TS `packages/opencode/src/cli/cmd/run/theme.ts:656 resolveRunTheme` (palette probe, dark/light pick, RUN_THEME_FALLBACK); crate `crates/opentui-bridge/src/theme.rs:33 Theme`.
Boundary: new file only `crates/opentui-bridge/src/run_theme.rs`. No lib.rs/Cargo.toml/theme.rs edits. No cargo. No commit.
Tests: pick_ok, empty_defaults, label_dark, label_light, truncates_64. rustfmt --check PASS.
Decision: std-only name+mode struct; full palette deferred (ponytail note in file).
Unknowns: none.
