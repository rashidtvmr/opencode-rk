# FOUNDATION-WIRING-FINAL

## Claim
`repo_ref`, `repo_cache_store`, `ops_parser_lane` existed as .rs files but unwired in lib.rs. Single additive edit wires all three.

## Edit
`crates/foundation/src/lib.rs`: added `pub mod ops_parser_lane;`, `pub mod repo_cache_store;`, `pub mod repo_ref;` (alphabetical placement).

## Evidence
- `cargo check -p opencode-rk-foundation`: 0 errors, 1 warning (dead_code `CacheStore.root` in repo_cache_store.rs:141, pre-existing, untouched).
- `cargo test -p opencode-rk-foundation --test repo_ref`: 5 passed.
- `cargo test -p opencode-rk-foundation --test repo_cache_store`: 5 passed.

## Re-verify 2026-09-16 (sole-writer wave)
- lib.rs: 30 `pub mod`, `repo_ref` + `repo_cache_store` + `repo_ref_ext` + `ops_repo_ref` present. No edit.
- Foundation: 10 passed / 0 failed (`/tmp/opencode/w1-foundation.log`).
- Agents: 35 passed / 0 failed (`/tmp/opencode/w1-agents.log`). delegation_gated.rs sha256 0dd9e565e23cb1d3d0e29edcd47917eaff7a656eabe4500fee0707b0b23adec7.
