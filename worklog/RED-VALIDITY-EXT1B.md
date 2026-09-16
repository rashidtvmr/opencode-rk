# RED-VALIDITY-EXT1B — EXT-001/002/004/005/006/008 temp-stub-restore

Rev: 248f519. Workdir /home/rashid/projects/opencode-rk. Crate opencode-rk-tools.
Scope: SOLE WRITER owned canonicals only this wave; ext_*_lane twins untouched.
Frozen files NEVER edited: crates/tools/tests/{plugin_lifecycle,plugin_builtins,plugin_deferred,ext_manifest_lane,plugin_scoped_exec,plugin_hook_boundary}.rs + ralph.json.
Method per file: backup orig to /tmp/opencode/redbak/*.orig, apply minimal behavioral neutralizer (compiling), run serial `cargo test -p opencode-rk-tools --test <t> -- --test-threads=1` with `free -h` first + JOBS=1/THREADS=1/timeout 120 semantics, capture RED log /tmp/opencode/wB-<id>-red.log (compiling + fail), restore byte-identical orig, rerun GREEN 5/5 --test-threads=1.

Pre-existing workspace state (not mine): working tree already had ~30 modified files from sibling lanes at start; unrelated lib warnings (shell_tool dead code, rtk_core, plugin_transform TEMP-RED-STUB wC-EXT-011) visible in build logs. My 6 owned src files restored byte-identical; remaining diffs on owned paths are rustfmt-only whitespace from sibling runs, no semantic change. No ext_*_lane twin touched. No frozen test/ralph.json write (hashes below match pre-run).

## Rows (RED neutralizer + failing test + GREEN)

| id | owned src | neutralizer (temp stub, reverted) | RED log | RED result (compiling+fail) | GREEN restore check |
|---|---|---|---|---|---|
| EXT-001 | crates/tools/src/plugin_lifecycle.rs | removed Duplicate-name guard in add (dup add returns Ok) | /tmp/opencode/wB-EXT001-red.log | compiling True; FAILED 4p/1f; fail ext001_t03_validation_and_overflow_registry_unchanged (tests/plugin_lifecycle.rs:81 unwrap_err on Ok PluginId(2)) | 5/5 ok |
| EXT-002 | crates/tools/src/plugin_builtins.rs | BUILTINS[1] caps skill.list -> [] | /tmp/opencode/wB-EXT002-red.log | compiling True; FAILED 4p/1f; fail ext002_t01_happy_path (tests/plugin_builtins.rs:29 BUILTINS table assert) | 5/5 ok |
| EXT-004 | crates/tools/src/plugin_deferred.rs | request_reload records WatchDeferred not ReloadDeferred (note: first attempt EXT4_MAX_DEFERRED=2 broke compile via const arithmetic in test; replaced with reason-swap compiling stub) | /tmp/opencode/wB-EXT004-red.log | compiling True; FAILED 3p/2f; fails ext004_t01_record_and_list (:88 reason), ext004_t04_no_lifecycle_mutation (:238 unknown-id ReloadDeferred) | 5/5 ok |
| EXT-005 | crates/tools/src/ext_manifest_lane.rs | MAX_MANIFEST_BYTES 16384 -> usize::MAX (oversize accepted) | /tmp/opencode/wB-EXT005-red.log | compiling True; FAILED 3p/2f; fails ext005_t01_happy_path (:38 const assert), ext005_t04_byte_layer_failures (:133 TooLarge) | 5/5 ok |
| EXT-006 | crates/tools/src/plugin_scoped_exec.rs | broker deny bypassed (`let _ = broker.assert`) | /tmp/opencode/wB-EXT006-red.log | compiling True; FAILED 4p/1f; fail ext006_t03_broker_deny_no_side_effects (:127 Denied) | 5/5 ok |
| EXT-008 | crates/tools/src/plugin_hook_boundary.rs | removed scope+phase+filter+label Duplicate guard in declare (note: first attempt MAX_HOOK_DECLS=1 broke compile via fill-loop const use; replaced with dup-guard removal) | /tmp/opencode/wB-EXT008-red.log | compiling True; FAILED 4p/1f; fail ext008_t03_validation_registry_unchanged (:86 Duplicate) | 5/5 ok |

Serial discipline: one cargo test at a time, `free -h` before each (avail ~2.3-3.0 GiB, swap 18 GiB free), `--test-threads=1`, timeout 120. lib.rs untouched (sole-writer claim held; only rustfmt-order whitespace from sibling present, no semantic edit by me).

## Hashes (post-restore, byte-identical to pre-run backup)

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

Backups: /tmp/opencode/redbak/*.orig. Stubs (for audit, NOT in tree): /tmp/opencode/redbak/*.stub.
Deviations: none semantic. Two neutralizer v1 attempts (deferred cap=2, hook cap=1) failed to compile due to test const-arithmetic/fill loops; documented above and replaced with compiling behavioral stubs. Failing-then-passing per-file proves test sensitivity; no test edited.
