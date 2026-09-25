# BRIDGE-PAR-395 (unclaimed, file-only lane)

- Claim: skipped per task prompt (orchestrator owns claims.json; unclaimed file-only write).
- Source: `packages/tui/src/theme/index.ts:130-164` DEFAULT_THEMES (32 literal + quoted keys one-dark/osaka-jade/lucent-orng/catppuccin-frappe/catppuccin-macchiato = 33 total); `crates/opentui-bridge/src/theme_assets.rs:6-40` ASSET_NAMES (33, sorted basenames). Sorted order matches index.ts asset import order a-z.
- Target: `crates/opentui-bridge/src/theme_index_full.rs` only. No lib.rs/Cargo.toml edits.
- API: `theme_count()->usize`, `theme_name(usize)->&'static str` (empty OOB), `has_theme(&str)->bool`. Delegates to theme_assets::ASSET_NAMES. std-only, forbid(unsafe_code).
- Tests: 4 (count 33, name bounds/roundtrip, has known/unknown, index-matches-has loop).
- Decisions: reuse ASSET_NAMES instead of duplicating list (single source, stays sorted); ponytail note for future JSON defs loading.
- Unknowns: none.
