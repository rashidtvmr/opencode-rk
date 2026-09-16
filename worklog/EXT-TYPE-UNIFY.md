# EXT-TYPE-UNIFY worklog

## Claim

`plugin_builtins.rs` no longer embeds its own `PluginRegistry` copy.
It re-exports the canonical EXT-001 types from `plugin_lifecycle.rs`
(single type identity in the lib build; same-source `#[path]` include
in the frozen `#[path]` test build). Owned file:
`crates/tools/src/plugin_builtins.rs` only. Frozen tests untouched.

## Source evidence

- Base revision `248f519`; lane prior `62513b1d` (`plugin_builtins.rs`),
  `4c7400f2` (`plugin_lifecycle.rs`).
- `crates/tools/src/plugin_lifecycle.rs:1-183` canonical registry
  (`PluginRegistry`, `PluginId`, `PluginDecl`, `PluginState`,
  `PluginInfo`, `PluginError`, `ContractVersion`, `MAX_PLUGINS=64`,
  `MAX_CAPABILITIES=16`, `MAX_NAME_LEN=128`, `MAX_CAP_LEN=64`,
  `SUPPORTED_CONTRACT_VERSION=1`).
- `crates/tools/src/lib.rs:35,39` wires both `pub mod plugin_builtins`
  and `pub mod plugin_lifecycle`.
- Frozen tests `crates/tools/tests/plugin_builtins.rs` and
  `crates/tools/tests/plugin_lifecycle.rs` use `#[path]` includes of
  their own module; untouched by this lane (pre-existing fmt-only
  working-tree drift in both, not mine; verified `git diff` shows only
  whitespace/fmt hunks there).

## Contract decisions

- Lib build (`#[cfg(not(test))]`): `pub use crate::plugin_lifecycle::{...}`
  — true unification, one type identity. Proven by scratch probe:
  `plugin_lifecycle::PluginRegistry` value passed to
  `plugin_builtins::unregister_builtins` and `assert_same(a, b)`
  compiles and prints `IDENTITY_OK`.
- Test build (`#[cfg(test)]`): `#[path = "plugin_lifecycle.rs"]
  mod lifecycle_canonical; pub use lifecycle_canonical::{...}` — same
  source definitions, zero local copies, so the frozen `#[path]` test
  (which compiles `plugin_builtins.rs` standalone, where
  `crate::plugin_lifecycle` does not resolve) stays green unmodified.
  Same proven pattern as the `ext_*_lane` shims.
- `register_builtins` overflow pre-check switched from private field
  `reg.entries.len()` to public `reg.list().len()` (canonical
  `entries` is private). Semantics identical.
- No `ContractVersion`-typed param change: `register_builtins(reg, u32)`
  signature kept byte-identical for frozen-test compat; the alias is
  re-exported alongside for single-path parity.

## Tests

- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 110 cargo test -p
  opencode-rk-tools --test plugin_builtins --test plugin_lifecycle`:
  10 passed (2 suites: 5/5 + 5/5), exit 0.
- `cargo check -p opencode-rk-tools`: ok; only the 3 pre-existing
  warnings (`shell_tool.rs` unused_mut, `rtk_core.rs` kept, `shell_tool.rs`
  with_timeout dead) — no new warnings from this lane.
- No `todo!/unimplemented!/stub/placeholder` in `plugin_builtins.rs`
  (grep clean). No fs/net/process/env/tokio refs (grep clean).
- Diff: `crates/tools/src/plugin_builtins.rs` 28 insertions,
  193 deletions (net -165 lines of duplication removed).

## Remaining unknowns / gaps

- Pre-existing working-tree drift (other lanes): fmt-only hunks in both
  frozen test files, `lib.rs` mod-order hunk, untracked mcp test files.
  Left untouched; verifier should diff against frozen hashes.
- `plugin_lifecycle.rs` untouched (`4c7400f2` clean for that file).
