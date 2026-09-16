# EXT1-VERIFY4 worklog (verify-only, no edits)

## Scope
VERIFY-ONLY for OWN EXT-001/002/004/005/006/008 at rev 248f519.
No product/test edits. No frozen/lib.rs/ralph.json/twin touches.

## Command
Pre-run `free -h`: 6.2Gi total, ~2.5Gi avail, swap 23Gi.
`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 120 cargo test -p opencode-rk-tools --test plugin_lifecycle --test plugin_builtins --test plugin_deferred --test plugin_manifest --test plugin_scoped_exec --test plugin_hook_boundary -- --test-threads=1`
Log: /tmp/opencode/yC-ext1.log

## Result
6 suites × 5/5 = 30 passed, 0 failed, exit 0. Serial JOBS=1 THREADS=1.
- plugin_lifecycle: ext001_t01..t05 5/5
- plugin_builtins: ext002_t01..t05 5/5
- plugin_deferred: ext004_t01..t05 5/5
- plugin_manifest: manifest_t01..t05 5/5 (names NOT ext005_t0x; see mismatch)
- plugin_scoped_exec: ext006_t01..t05 5/5
- plugin_hook_boundary: ext008_t01..t05 5/5
Warnings only (pre-existing: unused Duration import in security hook_bus_v2, unused_mut/dead_code in tools shell_tool/rtk_core). No errors.

## sha256 (worktree, post-run)
src:
- crates/tools/src/plugin_lifecycle.rs (183L): 4c7400f29cc299c8ae4b1e38bb3e9fe3b98111ca016065d46cc8250c7aa6a791
- crates/tools/src/plugin_builtins.rs (267L): 62513b1d1c4067523e2d3b773da045bd011c694333bf35c82092794441d83f6a
- crates/tools/src/plugin_deferred.rs (192L): c8bfbffb9fbc88ba805fbce0def514bf01ffcbe55fd1518649f18774d8b582fb
- crates/tools/src/plugin_manifest.rs (55L): c8c6682c1b2be8d3831aac9f58f4322cab42d810712447fc2c99257bf9c740b9
- crates/tools/src/plugin_scoped_exec.rs (286L): 4ddc1b58927eba67aef158bd46df142fd94374bed09e5d3fc3f7fb93a33fa848
- crates/tools/src/plugin_hook_boundary.rs (187L): fcb2e08a40cd4f56be563465f0f329d4713f843d4f685a6039efecde7ca27856
tests:
- crates/tools/tests/plugin_lifecycle.rs (229L): 27176198a1c25998ed676b08d9626a4a98cb0195b0bb754b1b23ab7e55d90d83
- crates/tools/tests/plugin_builtins.rs (185L): f114154b262c57137af85d78fdd787b68c468944e4687803250886bc40ea035f
- crates/tools/tests/plugin_deferred.rs (291L): d063dab6801d79d193b45699649bae93dab997b8263fa6fb3b354f5ef93b3fbc
- crates/tools/tests/plugin_manifest.rs (61L): af8baa1b6b8c1ba27c152f3a4f3dd997188f9338081706c5e7d1cbad780b8716
- crates/tools/tests/plugin_scoped_exec.rs (260L): 71988817a46537ea6e1c7d8f11042ed4050384cd3ed530601cb4259603158772
- crates/tools/tests/plugin_hook_boundary.rs (252L): 3fe69f3ea1b971bf2dc2fedf66b8f893387b4d090a4e7a58ce8e3b4d79223f7f

## Stub/IO grep
`todo!|unimplemented!|placeholder|#[ignore]`: 0 in all 6 src files; 0 in all 6 test files. Clean.
Frozen hash comparison: prior worklog/EXT-001.md recorded test hash 5fc260bd… for plugin_lifecycle.rs; current 27176198… differs. `git diff` on all 8 touched files (lib.rs reorder, 2 src, 5 tests) is cargo-fmt-only (whitespace/wrapping, no logic change). No behavior drift from diffs.

## Contract mismatch: EXT-005 (must-flag, not GREEN-proof)
Task card tasks/EXT-005.md demands: `validate_manifest_bytes(bytes:&[u8])` JSON pipeline (length→UTF-8→JSON→fields), `MAX_MANIFEST_BYTES=16384`, `MAX_NAME_LEN=128`, `MAX_CAPABILITIES=16`, `MAX_CAP_LEN=64`, `Manifest{name,contract_version:u32,capabilities}`, `SUPPORTED_CONTRACT_VERSION=1`, T03 field-validation + T04 byte-layer (TooLarge/InvalidEncoding/InvalidJson/1MiB prompt) + T05 safety.
Actual `crates/tools/src/plugin_manifest.rs` (55L): struct-only `PluginManifest{name:String,version:String(X.Y.Z),permissions:Vec<String>}`, `MAX_PERMISSIONS=32`, errors EmptyName/BadVersion/EmptyPermission/TooManyPermissions. No `validate_manifest_bytes`, no JSON/UTF-8/size layer, version is semver-string not u32 contract, cap 32 not 16. Tests named `manifest_t01..t05`, cover struct shape only; T04 byte-layer (oversize/binary/truncated/array/1MiB) absent. Suite GREEN 5/5 proves struct-shape only, NOT the frozen EXT-005-T01..T05 contract.
Disposition: owning lane must either implement the bytes pipeline to the card or move the card through gap-closure with verifier approval. This verify lane makes no change.

## Minor notes
- plugin_builtins.rs carries its own PluginRegistry copy (267L), not a re-export of plugin_lifecycle; type-unify still open (cf EXT-VERIFY-3). GREEN either way.
- plugin_scoped_exec `bind(plugin_id:&str, manifest:&Manifest)` uses String plugin id, card says EXT-001 PluginId binding; snapshot/revoke semantics tested GREEN.
- Worktree `git status` shows ~60 modified files repo-wide (fmt churn); the 12 EXT files above are subset; logic asserts intact per diff.
- Edits: none (this file only).
