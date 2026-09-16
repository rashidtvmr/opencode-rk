# REVERIFY-ZQ — verify-only re-run (rev 248f519)

Mode: VERIFY-ONLY. No product edits. No frozen/lib.rs/ralph.json touches.
Serial: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`, `--test-threads=1`, timeout 120 per run.
Workdir: /home/rashid/projects/opencode-rk. HEAD = 248f519d53ed29cbf7fef7ceb2b5010fb1a4224b.
Note: worktree dirty (432 paths `git status --short`), but HEAD pinned as instructed.

## 1. Agents (15 = 3 suites x 5)
Cmd: `cargo test -p opencode-rk-agents --test delegation_lane --test driver_lane --test delegation_gated -- --test-threads=1`
Log: /tmp/opencode/zQ-agents.log
Result: 15/15 pass (delegation_gated 5, delegation_lane 5, driver_lane 5). EXIT 0.

Hashes:
- crates/agents/tests/delegation_lane.rs 3ca19e36aae23c85a0ee65085d7df62b8d793c88ed2bcc08d02e7e0355b1d971
- crates/agents/tests/driver_lane.rs 4d9a30067d2bbf038f132decb51e5ed6c8c5f159de94e0133513c20a27405a37
- crates/agents/tests/delegation_gated.rs 0dd9e565e23cb1d3d0e29edcd47917eaff7a656eabe4500fee0707b0b23adec7

## 2. EXT/tools plugin lanes (50 = 10 suites x 5)
Suites: lifecycle, builtins, deferred, manifest, scoped_exec, hook_boundary, namespace, discover, transform, ui_boundary.
Cmd (run 1): `-p opencode-rk-tools --test plugin_lifecycle --test plugin_builtins --test plugin_deferred --test plugin_manifest --test plugin_scoped_exec`
Cmd (run 2): `-p opencode-rk-tools --test plugin_hook_boundary --test plugin_namespace --test plugin_discover --test plugin_transform --test plugin_ui_boundary`
Log: /tmp/opencode/zQ-ext.log (appended across both runs)
Result: 50/50 pass, 10x `test result: ok. 5 passed`. EXIT 0 both runs.

Hashes:
- plugin_lifecycle.rs 27176198a1c25998ed676b08d9626a4a98cb0195b0bb754b1b23ab7e55d90d83
- plugin_builtins.rs f114154b262c57137af85d78fdd787b68c468944e4687803250886bc40ea035f
- plugin_deferred.rs d063dab6801d79d193b45699649bae93dab997b8263fa6fb3b354f5ef93b3fbc
- plugin_manifest.rs af8baa1b6b8c1ba27c152f3a4f3dd997188f9338081706c5e7d1cbad780b8716
- plugin_scoped_exec.rs 71988817a46537ea6e1c7d8f11042ed4050384cd3ed530601cb4259603158772
- plugin_hook_boundary.rs 3fe69f3ea1b971bf2dc2fedf66b7f893387b4d090a4e7a58ce8e3b4d79223f7f
- plugin_namespace.rs 1768220718be90f39986309464e9389d4db6cb5a91b50e3fbcff3b254bd75627
- plugin_discover.rs 06c872b301e3b75f7e0f4d65f85815d44301678c7dc511121b0a8dce5dd51c66
- plugin_transform.rs 55fc1d44c99381f14da2998b7add36aa6eb3d02dc5fea9e7dfda0989d9dbf86e
- plugin_ui_boundary.rs db18b1e37c5f9fe13d58bbd6d22b83e2a404aa0f472cd286f6ee01a40e57d7c1

EXT-005 bytes presence: **n** — `crates/ext` does not exist; `crates/ext/src/manifest.rs` absent (EXT-005 lane owns that file, untouched per instructions). Manifest lane currently lives at `crates/tools/src/ext_manifest_lane.rs` (5551 bytes, present). No claim made about EXT-005 acceptance.

## 3. TOOL (40 = 8 suites x 5)
Suites: tool_allow, tool_query, tool_sandbox, rtk_core, rtk_pass, rtk_testfilter, slash_defs, terse_render.
Cmd (run 1): `-p opencode-rk-tools --test tool_allow --test tool_query --test tool_sandbox --test rtk_core`
Cmd (run 2): `-p opencode-rk-tools --test rtk_pass --test rtk_testfilter --test slash_defs --test terse_render`
Log: /tmp/opencode/zQ-tool.log (appended)
Result: 40/40 pass, 8x `test result: ok. 5 passed`. EXIT 0 both runs.

Hashes:
- tool_allow.rs 5e6334a93312a40e5b2708b9f7bc845121e8fd3bf1327b11c90ec6c41461b8cc
- tool_query.rs faad97e54c3fadda09f3e2750fa06bfbb489c8c571152abd15b43cf05324f3df
- tool_sandbox.rs 2ed3a71b5dc028b55bd7545193acc5dc43ef5ab04c6f8f6f5f5b498c75acc93c
- rtk_core.rs e37185db2058a18727ef0b49b5b039d84c554fc2e28d244d8305871065ed29f0
- rtk_pass.rs 0f514345d7bd2626c537b2279712df0b4652ad7c2f1468ec1eda5267adc0e9fc
- rtk_testfilter.rs 8f9c7f4c68db719faf0212bb322c1331c8840d8bb2bbee6b3b5f3cd02fadfbb9
- slash_defs.rs 1c3c842c820908a8256146b23eb7acaaef82be587868ecf745bf3f4f0d82a541
- terse_render.rs 5bc162a8da8277ed2a359261b8eabc3838146162b1c44153df1714828e8b9f20

## Totals
- agents: 15/15. ext/plugin lanes: 50/50. tool: 40/40. Grand: 105/105 green.
- Memory headroom stayed ~2.6-2.9 GiB available; no OOM/swap stall observed.
