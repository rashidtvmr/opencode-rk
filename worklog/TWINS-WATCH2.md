# TWINS-WATCH2: EXT + SHARE drift re-survey (READ-ONLY, 2026-09-16)

Scope: 8 EXT pairs (`crates/tools/src/plugin_*` vs `ext_*_lane`) + 5 SHARE
pairs (`crates/sessions/src/share_*` vs `*_lane`, incl. policy_lane vs
policy2_lane). No code touched, no cargo runs, no deletions, no lib.rs/test
edits. Method: `diff -q`, `diff | grep -c '^[<>]'`, `wc -l`, sha256,
`pub mod` grep in both `lib.rs`, test-header grep (`#[path]` vs `crate::`).
Timeout 120, rtk prefix. Priors: `worklog/EXT-TWINS-DISPOSITION.md`,
`worklog/SHARE-TWINS-DISPOSITION.md`, `worklog/TWINS-WATCH.md`.

## 1. EXT pairs

| # | canonical (wired) | twin (unwired) | diff -q | diff-lines | wc -l | sha8 | class now | drift vs prior y/n |
|---|---|---|---|---|---|---|---|---|
| 1 | plugin_transform | ext_replay_lane | differ | 10 | 166 / 170 | b4c517e3 / f831dca8 | COMMENT-ONLY (fmt reflow in `record()` + `project` doc "seq order" vs "insertion order") | n vs TWINS-WATCH (already COMMENT-ONLY there); n vs EXT-DISPOSITION logic-wise only insofar as guard-port already landed before TWINS-WATCH — see §3 |
| 2 | plugin_namespace | ext_namespacing_lane | identical | 0 | 124 / 124 | 5eaa922a = 5eaa922a | BYTE-IDENTICAL | n |
| 3 | plugin_discover | ext_discovery_lane | differ | 34 | 160 / 128 | d7d7c5c0 / 30e24833 | COMMENT-ONLY (stripped code equal per TWINS-WATCH c64d8657) | n |
| 4 | plugin_ui_boundary | ext_ui_boundary_lane | differ | 2 | 191 / 191 | 90883e97 / de389584 | COMMENT-ONLY (1 doc word) | n |
| 5 | plugin_builtins | ext_builtins_lane | differ | 3 | 267 / 268 | 62513b1d / 31f6966f | COMMENT-ONLY + `#![forbid(unsafe_code)]` twin-only | n |
| 6 | plugin_deferred | ext_deferred_lane | differ | 1 | 192 / 193 | c8bfbffb / cdad8c2f | COMMENT-ONLY + `#![forbid]` twin-only | n |
| 7 | plugin_lifecycle | ext_lifecycle_lane | differ | 5 | 183 / 188 | 4c7400f2 / 8bedf09a | COMMENT-ONLY (lane doc block + `#![forbid]` twin-only) | n |
| 8 | plugin_scoped_exec | ext_scoped_exec_lane | identical | 0 | 286 / 286 | 4ddc1b58 = 4ddc1b58 | BYTE-IDENTICAL | n |

Note: pair 1 `plugin_transform.rs` wc now 166 vs 170 recorded in TWINS-WATCH
(4 lines trimmed by third-party fmt reflow of the `record()` chain); twin
side unchanged (170, f831dca8 = same sha). Semantic content unchanged.

## 2. SHARE pairs

| # | canonical | twin | diff -q | diff-lines | wc -l | sha8 | class now | drift vs prior y/n |
|---|---|---|---|---|---|---|---|---|
| 9 | share_merge | share_merge_lane | differ | 130 | 228 / 232 | c97d3aab / 5bf3915e | DIVERGENT (all pub symbols Lane*-renamed) | n (keep-both, no shim) |
| 10 | share_queue | share_queue_lane | differ | 62 | 306 / 310 | 25057f59 / 10ef3449 | DIVERGENT (all pub symbols Lane*-renamed) | n (keep-both, no shim) |
| 11 | share_store | share_store_lane | differ | 4 | 248 / 248 | 418be003 / 7c32b321 | COMMENT-ONLY (2 doc words) | n (keep-both, shim pointless) |
| 12 | share_enterprise | share_enterprise_lane | differ | 32 | 101 / 125 | 7f7e7ac4 / b1bef9e0 | IMPL-DIVERGENT (thiserror vs manual Display + forbid) | n (keep-both, dep/unsafe surface) |
| 13 | share_policy_lane | share_policy2_lane | differ | 2 | 231 / 231 | 10bbb9fb / 0345fd4f | COMMENT-ONLY (1 doc line) | n (keep-both; rename fallback, never fold into `share_policy` visibility enum) |

## 3. plugin_transform guard-port — explicit flag

Guard PORTED on both sides. `plugin_transform.rs:128-139` and
`ext_replay_lane.rs:132-143` carry byte-identical
`set_scope_disabled` bodies, including
`if !self.disabled.contains(&scope) && self.entries.iter().any(|t| t.scope == scope)`
plus the invariant doc ("Unknown scope is a harmless no-op ... stays bounded
by `EXT11_MAX_TRANSFORMS`"). Prior EXT-TWINS-DISPOSITION:25-29 DIVERGENT
(unbounded `disabled` push in canonical) no longer holds — already recorded
as COMMENT-ONLY in TWINS-WATCH §3, confirmed unchanged here. HOLD reason for
pair 1 gone; pair 1 droppable like pairs 2-8 (integrator call; NOT done here).

## 4. lib.rs wiring (which side)

- `crates/tools/src/lib.rs:35-44`: wires all 10 `plugin_*`
  (`plugin_builtins`, `plugin_deferred`, `plugin_discover`,
  `plugin_hook_boundary`, `plugin_lifecycle`, `plugin_manifest`,
  `plugin_namespace`, `plugin_scoped_exec`, `plugin_transform`,
  `plugin_ui_boundary`). Zero `ext_*_lane` wired. Unchanged vs TWINS-WATCH §4.
- `crates/sessions/src/lib.rs:23-25`: wires `share_merge`, `share_policy`,
  `share_queue` (+3 unrelated: `part_events`, `runner`, `tui_info_panel`
  at :47-49). NOT wired: `share_store`, `share_enterprise`, any `*_lane`.
  Unchanged vs TWINS-WATCH §4 (pre-existing third-party workdir mods remain;
  not mine — read-only lane).

## 5. Test headers (`#[path]` vs crate import)

All sampled suites include module under test via
`#[path = "../src/<file>.rs"]` + local `mod`; `grep crate::` over sampled
test files = 0 hits. No suite resolves via `lib.rs`; dropping a twin src
breaks only its own test file (migrate/delete same commit). Stale-header
note stands: `share_merge.rs` / `share_merge_lane.rs` test headers claim
"NOT wired into `lib.rs`" but `lib.rs:23` wires `share_merge` — harmless,
integrator call.

## 6. Verdict

Drift rows: 0 of 13 vs TWINS-WATCH (pair-1 wc 170->166 is third-party fmt
reflow, no semantic change; twin sha unchanged). Vs EXT-TWINS-DISPOSITION:
1 resolved drift (pair-1 guard port, already recorded in TWINS-WATCH).
GREEN: n/a (read-only lane, no cargo runs per task).
