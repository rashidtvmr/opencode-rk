# TWINS-WATCH: EXT + SHARE disposition-drift survey (READ-ONLY, 2026-09-16)

Scope: 8 EXT pairs (`crates/tools/src/plugin_*` vs `ext_*_lane`) + 4 SHARE pairs
(`crates/sessions/src/share_*` vs `*_lane`). No code touched, no cargo runs.
Method: `diff -q`, `diff | grep -c '^[<>]'`, `wc -l`, sha256, comment-stripped
comparison, `pub mod` grep in both `lib.rs`, test-header grep (`#[path]` vs `crate::`).
Timeout 120, rtk prefix. Prior dispositions: EXT=`worklog/EXT-TWINS-DISPOSITION.md`,
SHARE=`worklog/SHARE-TWINS-DISPOSITION.md`.

## 1. EXT pairs

| # | canonical (wired) | twin (unwired) | diff -q | diff-lines | wc -l | sha8 | class now | prior class | match y/n |
|---|---|---|---|---|---|---|---|---|---|
| 1 | plugin_transform | ext_replay_lane | differ | 4 | 170 / 170 | 4c8d68f7 / f831dca8 | COMMENT-ONLY (fmt reflow + 1 doc line) | DIVERGENT (logic) | **N — DRIFT (see §3)** |
| 2 | plugin_namespace | ext_namespacing_lane | identical | 0 | 124 / 124 | 5eaa922a = 5eaa922a | BYTE-IDENTICAL | BYTE-IDENTICAL | y |
| 3 | plugin_discover | ext_discovery_lane | differ | 34 | 160 / 128 | d7d7c5c0 / 30e24833 | COMMENT-ONLY (stripped code hash equal c64d8657) | COMMENT-ONLY | y |
| 4 | plugin_ui_boundary | ext_ui_boundary_lane | differ | 2 | 191 / 191 | 90883e97 / de389584 | COMMENT-ONLY (1 doc word; stripped equal cfeeea9e) | COMMENT-ONLY | y |
| 5 | plugin_builtins | ext_builtins_lane | differ | 3 | 267 / 268 | 62513b1d / 31f6966f | COMMENT-ONLY + 1 attr line (`#![forbid(unsafe_code)]` twin-only; stripped diff = that line only) | COMMENT-ONLY (+forbid) | y |
| 6 | plugin_deferred | ext_deferred_lane | differ | 1 | 192 / 193 | c8bfbffb / cdad8c2f | COMMENT-ONLY + `#![forbid]` twin-only | COMMENT-ONLY (+forbid) | y |
| 7 | plugin_lifecycle | ext_lifecycle_lane | differ | 5 | 183 / 188 | 4c7400f2 / 8bedf09a | COMMENT-ONLY (lane doc block + `#![forbid]` twin-only; stripped diff = forbid only) | COMMENT-ONLY (+doc block, +forbid) | y |
| 8 | plugin_scoped_exec | ext_scoped_exec_lane | identical | 0 | 286 / 286 | 4ddc1b58 = 4ddc1b58 | BYTE-IDENTICAL | BYTE-IDENTICAL | y |

Raw diffs (current): pair 1 = `record()` rustfmt-only reflow (`any()` chain 1 line vs
5 lines) + `project` doc "seq order" vs "insertion order" (same iteration, output
sorted by key both sides). Pairs 5/6/7 stripped-code diff is exactly `0a1 >
#![forbid(unsafe_code)]`. Pair 5 also has `plugin_builtins` vs `ext_builtins_lane`
word inside a `//!` doc line (comment, stripped away).

## 2. SHARE pairs

| # | canonical | twin | diff -q | diff-lines | wc -l | sha8 | class now | prior disposition | match y/n |
|---|---|---|---|---|---|---|---|---|---|
| 9 | share_merge | share_merge_lane | differ | 130 | 228 / 232 | c97d3aab / 5bf3915e | DIVERGENT (all pub symbols Lane*-renamed; stripped code differs) | keep-both, no shim | y |
| 10 | share_queue | share_queue_lane | differ | 62 | 306 / 310 | 25057f59 / 10ef3449 | DIVERGENT (all pub symbols Lane*-renamed; stripped code differs) | keep-both, no shim | y |
| 11 | share_store | share_store_lane | differ | 4 | 248 / 248 | 418be003 / 7c32b321 | COMMENT-ONLY (2 doc words: `` `share_store` `` vs `` `share_store_lane` `` x2; stripped code equal fde314e7) | keep-both (shim pointless) | y |
| 12 | share_enterprise | share_enterprise_lane | differ | 32 | 101 / 125 | 7f7e7ac4 / b1bef9e0 | IMPL-DIVERGENT (names identical; canonical `thiserror::Error`, lane manual `Display`+`std::error::Error` + `#![forbid]` + lane-note doc block; stripped code differs) | keep-both (dep/`unsafe` surface) | y |

## 3. DRIFT — guard-port lane changed `plugin_transform.rs` (explicit flag)

Prior (EXT-TWINS-DISPOSITION:25-29): `ext_replay_lane::set_scope_disabled` guarded
unknown scopes (`entries.iter().any(|t| t.scope == scope)`), `plugin_transform`
pushed any scope (unbounded `disabled`). Disposition: HOLD twin until 1-line guard
ported into canonical.

Current (`crates/tools/src/plugin_transform.rs:123-139` vs
`ext_replay_lane.rs:127-143`): BOTH sides now carry the guard
`if !self.disabled.contains(&scope) && self.entries.iter().any(|t| t.scope == scope)`
plus the invariant doc ("Unknown scope is a harmless no-op ... stays bounded by
`EXT11_MAX_TRANSFORMS`"). Comment-stripped code hashes equal (2c917df7 = 2c917df7).
Prior DIVERGENT verdict no longer holds — pair 1 is now COMMENT-ONLY. HOLD reason
gone; pair 1 now droppable like pairs 2-8 (integrator call; do NOT delete here).

## 4. `lib.rs` wiring (which side)

- `crates/tools/src/lib.rs:35-44`: wires all 10 `plugin_*` (`plugin_builtins`,
  `plugin_deferred`, `plugin_discover`, `plugin_hook_boundary`, `plugin_lifecycle`,
  `plugin_manifest`, `plugin_namespace`, `plugin_scoped_exec`, `plugin_transform`,
  `plugin_ui_boundary`). Zero `ext_*_lane` mods wired.
- `crates/sessions/src/lib.rs:23-25`: wires `share_merge`, `share_policy`,
  `share_queue`. NOT wired: `share_store`, `share_enterprise`, any `*_lane`,
  `store`. (Pre-existing workdir modifications present in sessions src; not mine —
  this lane is read-only.)

## 5. Test headers (`#[path]` vs crate import)

All 24 suites (16 tools + 8 sessions) include the module under test via
`#[path = "../src/<file>.rs"]` + local `mod`; zero `crate::` imports found across
all sampled test files (grep `crate::` = 0 hits). No suite resolves via `lib.rs`,
so dropping a twin src breaks only its own test file (migrate/delete same commit).

## 6. Verdict

Drift rows: 1 of 12 (pair 1 transform/replay, DIVERGENT -> COMMENT-ONLY, guard
ported into canonical). All other 11 pairs match prior dispositions.
GREEN: n/a (read-only lane, no cargo runs per task).
