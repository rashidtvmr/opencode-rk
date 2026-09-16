# RED-VALIDITY-EXT1C — EXT-001/002/004/005/006/008 temp-stub-restore

Rev: 248f519d53ed29cbf7fef7ceb2b5010fb1a4224b. Workdir /home/rashid/projects/opencode-rk. Crate opencode-rk-tools.
Scope: OWN EXT-001/002/004/005/006/008 only. SOLE WRITER owned canonicals
`crates/tools/src/{plugin_lifecycle,plugin_builtins,plugin_deferred,ext_manifest_lane,plugin_scoped_exec,plugin_hook_boundary}.rs`
this wave. `ext_*_lane` twins untouched (dirty diffs on twins/lib.rs/tests are pre-existing sibling-lane rustfmt churn, not mine).
NEVER edited: frozen `crates/tools/tests/{plugin_lifecycle,plugin_builtins,plugin_deferred,ext_manifest_lane,plugin_scoped_exec,plugin_hook_boundary}.rs`,
`ralph.json`.
Method per file: backup orig to `/tmp/opencode/redbak-xC/*.orig`, apply minimal compiling behavioral neutralizer,
serial `cargo test -p opencode-rk-tools --test <t> -- --test-threads=1` with `free -h` first + timeout 120,
capture RED log `/tmp/opencode/xC-<id>-red.log` (compiling + behavior fail), `cp` orig back, `diff -q` verify,
rerun GREEN 5/5 `--test-threads=1`.

Pre-existing tree state (not mine): ~30+ dirty files from sibling lanes at start (rustfmt-only churn on
lib.rs/tests/src twins); unrelated warnings (`hook_bus_v2` unused import, `shell_tool`, `rtk_core`,
`mcp_catalog_search` unreachable). `plugin_namespace.rs:76` TEMP-RED-STUB xD-EXT-009 belongs to another lane,
untouched. lane_gate.py covers storage lanes only, N/A for EXT.

## Rows (RED neutralizer + failing test + GREEN)

| id | owned src | neutralizer (temp stub, reverted) | RED log | RED result (compiling + fail) | GREEN |
|---|---|---|---|---|---|
| EXT-001 | crates/tools/src/plugin_lifecycle.rs | dup-name guard neutralized (`let _ = any(...)`, dup add returns Ok) | /tmp/opencode/xC-EXT001-red.log | compiling True (`Compiling opencode-rk-tools`, `Finished test profile`); FAILED 4p/1f; `ext001_t03_validation_and_overflow_registry_unchanged` (tests/plugin_lifecycle.rs:81 `unwrap_err` on `Ok(PluginId(2))`) | 5/5 ok |
| EXT-002 | crates/tools/src/plugin_builtins.rs | `BUILTINS[1]` caps `skill.list` -> `[]` | /tmp/opencode/xC-EXT002-red.log | compiling True; FAILED 4p/1f; `ext002_t01_happy_path` (tests/plugin_builtins.rs:29 table assert) | 5/5 ok |
| EXT-004 | crates/tools/src/plugin_deferred.rs | `request_reload` records `WatchDeferred` not `ReloadDeferred` | /tmp/opencode/xC-EXT004-red.log | compiling True; FAILED 3p/2f; `ext004_t01_record_and_list` (:88 reason), `ext004_t04_no_lifecycle_mutation` (:238 unknown-id ReloadDeferred) | 5/5 ok |
| EXT-005 | crates/tools/src/ext_manifest_lane.rs | `MAX_MANIFEST_BYTES` 16384 -> `usize::MAX` (oversize accepted) | /tmp/opencode/xC-EXT005-red.log | compiling True; FAILED 3p/2f; `ext005_t01_happy_path` (:38 const assert), `ext005_t04_byte_layer_failures` (:133 TooLarge) | 5/5 ok |
| EXT-006 | crates/tools/src/plugin_scoped_exec.rs | broker deny bypassed (deny arm made no-op + extra assert) | /tmp/opencode/xC-EXT006-red.log | compiling True; FAILED 4p/1f; `ext006_t03_broker_deny_no_side_effects` (:127 `Ok(applied,4)` vs `Err(Denied)`) | 5/5 ok |
| EXT-008 | crates/tools/src/plugin_hook_boundary.rs | scope+phase+filter+label Duplicate guard removed in `declare` | /tmp/opencode/xC-EXT008-red.log | compiling True; FAILED 4p/1f; `ext008_t03_validation_registry_unchanged` (:86 `Ok(HookId(2))` vs `Err(Duplicate)`) | 5/5 ok |

Serial discipline: one cargo test at a time, `free -h` before each (avail ~2.3-2.6 GiB, swap 18 GiB free),
`--test-threads=1`, timeout 120. No frozen test/ralph.json write. `grep xC-RED crates/tools/src` sweep clean.
Note: working-tree `git diff` still shows rustfmt-only whitespace on two owned files
(`plugin_deferred.rs` push-sig wrap, `plugin_scoped_exec.rs` fmt wraps) from sibling runs — semantic content
byte-identical to backups/hashes below (verified `diff -q` per file at restore).

## Hashes (post-restore, match pre-run backups)

src:
- 4c7400f29cc299c8ae4b1e38bb3e9fe3b98111ca016065d46cc8250c7aa6a791 crates/tools/src/plugin_lifecycle.rs
- 62513b1d1c4067523e2d3b773da045bd011c694333bf35c82092794441d83f6a crates/tools/src/plugin_builtins.rs
- c8bfbffb9fbc88ba805fbce0def514bf01ffcbe55fd1518649f18774d8b582fb crates/tools/src/plugin_deferred.rs
- e54e9647616f93c14be9b23ef62c54fd89b21aa4e1749d758d4b8d3a9bbb7994 crates/tools/src/ext_manifest_lane.rs
- 4ddc1b58927eba67aef158bd46df142fd94374bed09e5d3fc3f7fb93a33fa848 crates/tools/src/plugin_scoped_exec.rs
- fcb2e08a40cd4f56be563465f0f329d4713f843d4f685a6039efecde7ca27856 crates/tools/src/plugin_hook_boundary.rs
- ace227addf1f604a265fcac7e1015ca5b7879a70b20e656829edce210f0d0fdb crates/tools/src/lib.rs

frozen (unedited):
- 27176198a1c25998ed676b08d9626a4a98cb0195b0bb754b1b23ab7e55d90d83 crates/tools/tests/plugin_lifecycle.rs
- f114154b262c57137af85d78fdd787b68c468944e4687803250886bc40ea035f crates/tools/tests/plugin_builtins.rs
- d063dab6801d79d193b45699649bae93dab997b8263fa6fb3b354f5ef93b3fbc crates/tools/tests/plugin_deferred.rs
- 92701b3f327e804647e5ba54bc032be44ac9d2dbb922c0787379c34597209ebd crates/tools/tests/ext_manifest_lane.rs
- 71988817a46537ea6e1c7d8f11042ed4050384cd3ed530601cb4259603158772 crates/tools/tests/plugin_scoped_exec.rs
- 3fe69f3ea1b971bf2dc2fedf66b7f893387b4d090a4e7a58ce8e3b4d79223f7f crates/tools/tests/plugin_hook_boundary.rs
- 4b99ccdc6fbe9df8397d15af7ed8682e43fd6de660cb5e2df71465e5835ae9c6 ralph.json

Backups: /tmp/opencode/redbak-xC/*.orig. No ext_*_lane twin touched. Deviations: none semantic.
