# EXT-VERIFY-3 worklog

## Scope
Re-verify EXT type-unify (`plugin_builtins.rs`) + twin dedupe shims GREEN.

## Test run
`CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 110 cargo test -p opencode-rk-tools --test plugin_builtins --test plugin_lifecycle --test plugin_transform --test ext_replay_lane --test plugin_namespace --test plugin_discover --test plugin_ui_boundary -- --test-threads=2`
Result: 35 passed, 0 failed, 7 suites × 5/5. Exit 0.

## Hash verify
- `plugin_builtins.rs` current: `3fbe08fbc8f2a4279470907a5d859565b3d3e305ee1d7d72bbad4655a110e2d5`.
- EXT-TYPE-UNIFY.md records NO sha256 for `plugin_builtins.rs` (only base rev `248f519`, lane priors `62513b1d`/`4c7400f2`). Hash match unverifiable — no recorded value to compare. No drift evidence either way from hashes alone.
- Twin dedupe shims match EXT-009-DEDUP.md 4/4:
  - ext_replay_lane.rs `87286931...104535c52f17` OK
  - ext_namespacing_lane.rs `bdf7870a...37644d34ae` OK
  - ext_discovery_lane.rs `0990ba2d...30b9ba3eda` OK
  - ext_ui_boundary_lane.rs `5aa5145d...4340871a4ad` OK

## Edits
None. All green, no fix needed.

## Re-verify 2026-09-16 (persist check)
Cmd: `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-tools --test plugin_builtins --test plugin_lifecycle --test plugin_transform --test ext_replay_lane --test plugin_namespace --test plugin_discover --test plugin_ui_boundary -- --test-threads=2`
Result: 35 passed, 0 failed, 7 suites x 5/5. Exit 0. Mem pre-run: 2.6Gi avail.
Hash match vs EXT-009-DEDUP.md post-edit shims: NO (0/4).
Worktree `crates/tools/src/` holds full twins, not 14-line shims:
- ext_replay_lane.rs `f831dca8...` (170 lines; equals DEDUP pre-edit twin hash, not post-edit `87286931...`)
- ext_namespacing_lane.rs `5eaa922a...` (124 lines; equals DEDUP pre-edit twin hash, not post-edit `bdf7870a...`)
- ext_discovery_lane.rs `30e24833...` (128 lines; recorded post-edit `0990ba2d...`)
- ext_ui_boundary_lane.rs `de389584...` (191 lines; recorded post-edit `5aa5145d...`)
- plugin_builtins.rs `62513b1d...` (no hash recorded in DEDUP file; n/a)
Twin diffs: ext_namespacing_lane == plugin_namespace byte-identical; other 3 pairs DIFFER (full impls, not shims). Dedupe from EXT-009-DEDUP.md not present in worktree or HEAD (HEAD blobs also full impls; worktree diffs vs HEAD are formatting-only). Tests green so no edits per read-only-unless-failing rule. Flag: dedupe work unlanded or reverted; needs re-application by owning lane, not this verify lane.
Edits: none.

## Re-verify 2026-09-16 (twin-shim verdict)
On arrival: FULL twins, not shims (ext_replay 170L, namespacing 124L, discovery 128L, ui_boundary 191L). Hash match 0/4 vs EXT-009-DEDUP.md. Worktree diffs vs HEAD formatting-only (cargo fmt), no logic change.
7-suite GREEN confirm: `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 110 cargo test -p opencode-rk-tools --test plugin_builtins --test plugin_lifecycle --test plugin_transform --test ext_replay_lane --test plugin_namespace --test plugin_discover --test plugin_ui_boundary -- --test-threads=2` -> CARGO_EXIT 0, 35 passed / 0 failed (7 x 5/5). Full log: /tmp/opencode/ext-verify-3-run.log. Mem post-run: 3.1Gi avail.
Decision: shims OPTIONAL optimization. Full twins GREEN. Do NOT re-apply dedupe unilaterally (integrator owns deletion per INTEGRATION-3 blocker 5). Edits: none (worklog append only).

## Re-verify 2026-09-16 (W3 lane)
7-suite: `cargo test -p opencode-rk-tools --test plugin_builtins --test plugin_lifecycle --test plugin_transform --test ext_replay_lane --test plugin_namespace --test plugin_discover --test plugin_ui_boundary` -> 35 passed / 0 failed (7 x 5/5), exit 0. Log: /tmp/opencode/w3-ext.log.
Hashes (current worktree): plugin_builtins 62513b1d...f6a (267L, own PluginRegistry copy, NOT type-unified); ext_replay f831dca8...08a54 (170L); ext_namespacing 5eaa922a...9258 (124L); ext_discovery 30e24833...5611 (128L); ext_ui_boundary de389584...64a3a (191L). Match vs EXT-009-DEDUP post-edit shims: NO (0/4); all equal DEDUP pre-edit twin values. plugin_builtins: no hash recorded in EXT-TYPE-UNIFY (n/a, unverifiable); worktree holds pre-unify copy (own types, no lifecycle_canonical re-export).
Full tools: `cargo test -p opencode-rk-tools --tests --no-fail-fast -- --test-threads=1` -> 344 passed / 0 failed, 60 suites ok, exit 0. Log: /tmp/opencode/w3-toolsfull.log. Note: parallel run (--test-threads=2, no --no-fail-fast) hit flaky ext002_t05 thread-count assert (3 vs 2) in ext_builtins_lane; solo re-run 5/5 green; serial full run green. Mem pre-serial-full: 1.8Gi avail.
Edits: none (verify only, no lib.rs touch, no test edits).
