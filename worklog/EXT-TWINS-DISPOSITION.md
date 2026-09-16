# EXT-TWINS-DISPOSITION: plugin_* vs ext_*_lane twin audit

Prior: full twins GREEN; EXT-009-DEDUP shims reverted/unlanded (EXT-VERIFY-3:36-38); integrator owns deletion.

## Evidence (2026-09-16 worktree)

`lib.rs` wires: `plugin_builtins, plugin_deferred, plugin_discover, plugin_hook_boundary,
plugin_lifecycle, plugin_manifest, plugin_namespace, plugin_scoped_exec, plugin_transform,
plugin_ui_boundary` (lib.rs:35-44). Zero `ext_*_lane` mods wired.
External users: none — `grep` for all 16 module names outside `crates/tools/src|tests` = 0 hits.
Tests: all 16 suites self-contained via `#[path]` to own src file, 5 `#[test]` each (80 total);
no suite depends on `lib.rs`. Deleting a twin src breaks only its own test file.

| # | plugin_* (wired) | ext_*_lane (unwired) | src lines | sha256 (short) | verdict |
|---|---|---|---|---|---|
| 1 | plugin_transform | ext_replay_lane | 163 / 170 | 0885c157 / f831dca8 | DIVERGENT (logic) |
| 2 | plugin_namespace | ext_namespacing_lane | 124 / 124 | 5eaa922a = 5eaa922a | BYTE-IDENTICAL |
| 3 | plugin_discover | ext_discovery_lane | 160 / 128 | d7d7c5c0 / 30e24833 | COMMENT-ONLY (stripped diff 0) |
| 4 | plugin_ui_boundary | ext_ui_boundary_lane | 191 / 191 | 90883e97 / de389584 | COMMENT-ONLY (1 doc word) |
| 5 | plugin_builtins | ext_builtins_lane | 267 / 268 | 62513b1d / 31f6966f | COMMENT-ONLY (mod-name word + `#![forbid(unsafe_code)]` twin-only) |
| 6 | plugin_deferred | ext_deferred_lane | 192 / 193 | c8bfbffb / cdad8c2f | COMMENT-ONLY (`#![forbid]` twin-only) |
| 7 | plugin_lifecycle | ext_lifecycle_lane | 183 / 188 | 4c7400f2 / 8bedf09a | COMMENT-ONLY (lane doc block + `#![forbid]` twin-only) |
| 8 | plugin_scoped_exec | ext_scoped_exec_lane | 286 / 286 | 4ddc1b58 = 4ddc1b58 | BYTE-IDENTICAL |

Pair 1 divergence (real behavior, both suites GREEN because tests assert projection bytes only):
- `ext_replay_lane::set_scope_disabled` guards `entries.iter().any(|t| t.scope == scope)` —
  unknown-scope disable records nothing (`disabled` bounded by `EXT11_MAX_TRANSFORMS`).
- `plugin_transform::set_scope_disabled` pushes any unknown scope into `disabled` (unbounded growth).
- Doc-only deltas: `project` "seq order" vs "insertion order" (same iteration).

## Disposition

KEEP-canonical in all pairs = `plugin_*` (lib.rs-wired side). Do NOT delete anything here;
integrator executes drops + test-file migration.

| pair | recommendation | rationale |
|---|---|---|
| 1 transform/replay | KEEP `plugin_transform` wired; HOLD `ext_replay_lane` (do not drop yet) | Only divergent pair. Twin behaviorally superior (bounded `disabled`). Port 1-line unknown-scope guard + invariant docs into canonical first; then drop twin. Test impact on drop: -5 dup, canonical 5 retained. |
| 2 namespace/namespacing | KEEP `plugin_namespace`; DROP twin src+test | Byte-identical. -5 dup tests, 0 behavior loss. |
| 3 discover/discovery | KEEP `plugin_discover`; DROP twin src+test | Code-identical; canonical has richer docs. -5 dup tests. |
| 4 ui_boundary | KEEP `plugin_ui_boundary`; DROP twin src+test | 1-word doc delta. -5 dup tests. |
| 5 builtins | KEEP `plugin_builtins`; DROP twin src+test | Comment-only; port twin's `#![forbid(unsafe_code)]` (1 line) into canonical at drop. -5 dup tests. |
| 6 deferred | KEEP `plugin_deferred`; DROP twin src+test | Comment-only; port `#![forbid(unsafe_code)]`. -5 dup tests. |
| 7 lifecycle | KEEP `plugin_lifecycle`; DROP twin src+test | Comment-only; port `#![forbid(unsafe_code)]` (lane doc block stays dropped). -5 dup tests. |
| 8 scoped_exec | KEEP `plugin_scoped_exec`; DROP twin src+test | Byte-identical. -5 dup tests. |

Totals: drop 7 pairs (after pair-1 guard port) = -35 dup tests; retained 40 canonical (8x5) + 0 twins.
`#[path]` coupling: each twin test file breaks iff its src file deleted — migrate/delete in same commit.

## GREEN (7-suite, 35)
Cmd: `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-tools
--test plugin_builtins --test plugin_lifecycle --test plugin_transform --test ext_replay_lane
--test plugin_namespace --test plugin_discover --test plugin_ui_boundary -- --test-threads=1`
(same 7-suite set as EXT-VERIFY-3 for comparability). Log: /tmp/opencode/uM-ext.log.
Mem pre-run: 2.9Gi avail (see log head).
Result: 35 passed / 0 failed (7 x 5/5), exit 0.

## aF-guard verification (2026-09-16, rev 248f519, JOBS=1 THREADS=1)
Scope: VERIFY-ONLY. No source edits. Serial runs, `--test-threads=1`.
Cmd1: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 120 cargo test -p opencode-rk-tools --test plugin_transform -- --test-threads=1`
Cmd2: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 120 cargo test -p opencode-rk-tools --test ext_replay_lane -- --test-threads=1`
Log: /tmp/opencode/aF-guard.log. Mem pre-run: 2.6Gi avail (see log head).
Result: plugin_transform 5/5 (exit_pt=0), ext_replay_lane 5/5 (exit_er=0). Total 10/10, 0 failed.
sha256 plugin_transform.rs (src): b4c517e36a3d5141bb9561ef0bf526aabd54372e2c43a239fab6caaa1d004d43
sha256 ext_replay_lane.rs (test): 7f7c7b1e0674877d563bd730f9fe23657fa2876e6f0d9e9dc3e3f5349377e4af
sha256 plugin_transform.rs (test): 55fc1d44c99381f14da2998b7add36aa6eb3d02dc5fea9e7dfda0989d9dbf86e
Guard-present: y — `entries.iter().any` at src lines 104 (dup-key) + 130 (unknown-scope guard in set_scope_disabled).
Untouched: frozen/, lib.rs, ralph.json.
