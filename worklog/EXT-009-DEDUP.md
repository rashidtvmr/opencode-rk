# EXT-009-DEDUP worklog

## Claim
Convert ext_* twins to re-export shims of plugin_* canonicals; keep frozen tests green.

## Pairs (pre-edit evidence)
- plugin_transform vs ext_replay_lane: byte-identical (sha256 f831dca8...), diff exit 0.
- plugin_namespace vs ext_namespacing_lane: byte-identical (sha256 5eaa922a...), diff exit 0.
- plugin_discover vs ext_discovery_lane: code-identical sans comments
  (non-comment stripped lines equal, 109/109); raw diff = comment lines only.
- plugin_ui_boundary vs ext_ui_boundary_lane: code-identical sans one doc word
  ("boundary" vs "boundary lane" line 1); non-comment lines equal, 129/129.
- Pattern matches existing shims: ext_builtins_lane, ext_deferred_lane,
  ext_lifecycle_lane, ext_scoped_exec_lane (all 14-line `#[path]+pub use` shims).

## Edit
Rewrote 4 files to 14-line shim form (`#[path="plugin_*.rs"] mod canonical; pub use canonical::*;`),
mirroring ext_builtins_lane.rs wording. Tests untouched (still `#[path]` so compile).

## Tests
- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 rtk cargo test -p opencode-rk-tools --test ext_replay_lane --test ext_namespacing_lane --test ext_discovery_lane --test ext_ui_boundary_lane --test plugin_transform --test plugin_namespace --test plugin_discover --test plugin_ui_boundary` => 40 passed (8 suites).
- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 rtk cargo test -p opencode-rk-tools --lib` => 49 passed (1 suite).

## Hashes (post-edit)
- ext_replay_lane.rs 87286931d821aec8dd3dbc6e1bfe2a5905902b523df25a04287a104535c52f17
- ext_namespacing_lane.rs bdf7870a6bfba70fa29b2926326f41534762125efc14cc6d0bcadd37644d34ae
- ext_discovery_lane.rs 0990ba2de0db6358a707f5efeac03546d8af581ecdd7b47a2116aa30b9ba3eda
- ext_ui_boundary_lane.rs 5aa5145df927d488a58f2071b74a59b9dafc3d0afa983cbdfc5654340871a4ad

## Diff stat
4 files changed, 52 insertions(+), 605 deletions(-)

## Unknowns
None. Note: crates/tools/src/lib.rs declares no ext_*lane mods; shims compile via
tests' #[path] include, same as prior shim lanes. Integrator may delete shims on test migration.
