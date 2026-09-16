# EXT-DEDUP: plugin_* vs ext_*_lane canonicalization

## Claim

`ext_lifecycle_lane`, `ext_builtins_lane`, `ext_deferred_lane`,
`ext_scoped_exec_lane` are now re-export shims over canonical
`plugin_lifecycle`, `plugin_builtins`, `plugin_deferred`,
`plugin_scoped_exec`. One definition each; frozen `#[path]` lane tests
still compile unmodified.

## Source evidence

- `crates/tools/src/plugin_lifecycle.rs:1-183` canonical EXT-001 registry
  (`PluginRegistry`, `PluginId`, `PluginError`, `MAX_PLUGINS=64`, ...).
- `crates/tools/src/ext_lifecycle_lane.rs` was byte-duplicate (+5-line
  header, `4c7400f2` vs `8bedf09a`); now 12-line shim
  (`#[path="plugin_lifecycle.rs"] mod canonical; pub use canonical::*;`).
- `crates/tools/src/plugin_builtins.rs:1-267` canonical EXT-002 wiring
  (`BUILTINS`, `register_builtins` with rollback, `is_builtin`).
- `crates/tools/src/ext_builtins_lane.rs` was byte-duplicate
  (`62513b1d` vs `31f6966f`); now shim, same pattern.
- `crates/tools/src/plugin_deferred.rs:1-188` canonical EXT-004 boundary
  (`DeferredLog`, `EXT4_MAX_DEFERRED=128`, `EXT4_MAX_WATCHERS=32`).
- `crates/tools/src/ext_deferred_lane.rs` was byte-duplicate
  (`7fa4410c` vs `b8b18f61`); now shim.
- `crates/tools/src/plugin_scoped_exec.rs` vs
  `crates/tools/src/ext_scoped_exec_lane.rs` were byte-identical
  (`37f08cda` both); `ext_scoped_exec_lane.rs` now shim.
- No outside users: `grep -rln <module> crates/ | grep -v
  crates/tools/src|tests` => none. `tools/lane_gate.py` has no
  `plugin_|ext_` entries. `lib.rs` wires only `plugin_*` (lines 35-44),
  never `ext_*_lane` — shims are test-only includes, zero lib impact.

## Decision (minimal additive unification)

- Direction: `ext_*_lane` re-exports canonical `plugin_*` (not vice
  versa): `plugin_*` is `lib.rs`-wired public API; lane files are
  test-only `#[path]` includes owned by frozen tests.
- No copies deleted without integrator approval: shim keeps both paths
  compiling; integrator deletes shim once lane test migrates to canonical
  `#[path]`.
- Frozen tests untouched: all 12 test files keep exact `#[path]` + symbol
  imports; only `src/ext_*_lane.rs` bodies changed.

## Not duplicates (left alone)

- `ext_hooks.rs` (`ExtHookRegistry`, pre/post/error) vs
  `plugin_hook_boundary.rs` (EXT-008 inert `HookBoundary`, Before/After,
  gate, `try_execute=>Deferred`): divergent contracts, distinct tests
  (`tests/ext_hooks.rs` uses `opencode_rk_tools::ext_hooks`, no `#[path]`).
- `plugin_manifest.rs` (name/version/permissions) vs
  `ext_manifest_lane.rs` (EXT-005 name/contract_version/capabilities over
  JSON bytes): divergent contracts; lane doc already records distinction.
- `plugin_discover.rs` vs `ext_discovery_lane.rs`: same logic, comments
  only drift; owned by EXT-010 lane, not this slice.
- `plugin_transform.rs`/`ext_replay_lane.rs`,
  `plugin_namespace.rs`/`ext_namespacing_lane.rs`,
  `plugin_ui_boundary.rs`/`ext_ui_boundary_lane.rs`: byte-identical or
  comment-only; owned by EXT-009/011/012 lanes, not this slice.

## Remaining drift / gaps

- `plugin_builtins.rs` embeds its own `PluginRegistry`/`PluginId` copy
  instead of `use plugin_lifecycle::` types (task card wants type reuse).
  Behavior matches; type-level unification needs `pub(crate) entries`
  exposure + frozen-test import rewrite => integrator + controller call.
- `crates/ext` crate + `cargo test -p opencode-rk-ext` in task cards does
  not exist; code lives in `crates/tools` (`opencode-rk-tools`). Controller
  disposition needed.
- Workspace has concurrent-lane dirty files outside this slice
  (`ext_enable.rs`, `ext_namespacing_lane.rs`, `ext_replay_lane.rs`,
  `ext_ui_boundary_lane.rs`, `lib.rs` mod-order, `plugin_deferred.rs`
  fmt-only hunks); left untouched, not claimed here.
- `ext_builtins_lane` T05 thread-count assertion is order/parallelism
  sensitive (failed 3-vs-2 under multi-test binary once); passes serial
  (`--test-threads=1`) and in the mandated 6-target serial run.

## Tests (GREEN, serial, JOBS=2 THREADS=2 timeout 110)

Mandated 6-target serial run, all pass:
`plugin_lifecycle` 5/5, `plugin_builtins` 5/5, `plugin_deferred` 5/5,
`ext_manifest_lane` 5/5, `ext_scoped_exec_lane` 5/5,
`plugin_hook_boundary` 5/5 (receipt: `/tmp/opencode/combined.log`).
Shim mirrors also green: `ext_lifecycle_lane` 5/5, `ext_builtins_lane`
5/5 (serial), `ext_deferred_lane` 5/5, `plugin_scoped_exec` 5/5.
RED probe: `plugin_lifecycle.rs` stubbed => `plugin_lifecycle` test
fails compile (`unresolved imports`); restored byte-identical
(`4c7400f2`, `git status` clean for that file).
