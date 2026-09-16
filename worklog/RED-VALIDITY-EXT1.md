# RED-validity EXT-001/002/004/005/006/008 (lane-owned plugin modules)

Scope: ONLY `crates/tools/src/plugin_lifecycle.rs`, `plugin_builtins.rs`,
`plugin_deferred.rs`, `ext_manifest_lane.rs`, `plugin_scoped_exec.rs`,
`plugin_hook_boundary.rs`. Frozen tests, `lib.rs`, `ralph.json` untouched.
Twin `ext_*_lane.rs` files untouched (dedupe-unresolved, other lane).
Method per file: `sha256` pre, one-guard temp stub, RED capture, restore,
`sha256` post equality, GREEN rerun.

Pre hashes (also post, byte-identical):
- `plugin_lifecycle.rs` `4c7400f2…a6a791`
- `plugin_builtins.rs` `62513b1d…83f6a`
- `plugin_deferred.rs` `c8bfbffb…8b582fb`
- `ext_manifest_lane.rs` `e54e9647…3a9bbb7994`
- `plugin_scoped_exec.rs` `4ddc1b58…3fa848`
- `plugin_hook_boundary.rs` `fcb2e08a…7ca27856`

All RED runs: `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120`,
ext suites serial `--test-threads=1`. All logs compiling (`Compiling
opencode-rk-tools`, `Finished test profile`), fail = behavior assertion,
not missing module.

| id | stub (temp, reverted) | RED log | RED | GREEN |
|----|----------------------|---------|-----|-------|
| EXT-001 | `plugin_lifecycle.rs:120` dup check `if false && …` | `/tmp/opencode/rB-EXT-001-red.log` | `FAILED 4 passed 1 failed`; `ext001_t03…FAILED` | 5/5 |
| EXT-002 | `plugin_builtins.rs:138` dup check `if false && …` (first attempt overflow stub at `register_builtins:220` passed 5/5, discarded; dup stub fails) | `/tmp/opencode/rB-EXT-002-red.log` | `FAILED 3 passed 2 failed`; `ext002_t03_atomicity FAILED`, `ext002_t04_no_lifecycle_redefinition FAILED` | 5/5 |
| EXT-004 | `plugin_deferred.rs:102` `push` overflow `if false && …` | `/tmp/opencode/rB-EXT-004-red.log` | `FAILED 4 passed 1 failed`; `ext004_t03_caps_and_drain FAILED` (`Ok(129)` vs `Err(Overflow)`) | 5/5 |
| EXT-005 | `ext_manifest_lane.rs:92` size cap `if false && …` | `/tmp/opencode/rB-EXT-005-red.log` | `FAILED 4 passed 1 failed`; `ext005_t04_byte_layer_failures FAILED` | 5/5 |
| EXT-006 | `plugin_scoped_exec.rs:247` broker check `if false && …` | `/tmp/opencode/rB-EXT-006-red.log` | `FAILED 4 passed 1 failed`; `ext006_t03_broker_deny_no_side_effects FAILED` (`Ok(applied)` vs `Err(Denied)`) | 5/5 |
| EXT-008 | `plugin_hook_boundary.rs:145` dup check `if false && …` | `/tmp/opencode/rB-EXT-008-red.log` | `FAILED 4 passed 1 failed`; `ext008_t03_validation_registry_unchanged FAILED` (`Ok(HookId(2))` vs `Err(Duplicate)`) | 5/5 |

Notes:
- EXT-002 first overflow-only stub still GREEN (duplicate path caught it);
  valid RED required neutralizing duplicate guard. Log keeps final failing run.
- One transient `mcp_catalog_search.rs` E0425 seen mid-run came from another
  lane's dirty worktree edit resolving concurrently, not this lane; file now
  matches HEAD (`8c2142c3…`), owned six files byte-identical pre/post.
- Pre-existing warnings only (`hook_bus_v2` unused import, `shell_tool`,
  `rtk_core`); no test edits.
