# TWINS-WATCH5: EXT + SHARE drift re-survey (READ-ONLY, 2026-09-16)

Scope: 8 EXT pairs (`crates/tools/src/plugin_*` vs `ext_*_lane`) + 5 SHARE
pairs (`crates/sessions/src/share_*` vs `*_lane`, incl. policy_lane vs
policy2_lane). No code touched, no cargo runs (fmt check only, see STUB-FMT-9),
no deletions, no lib.rs/test edits. Method: `diff -q`, `diff | grep -c '^[<>]'`,
`wc -l`, sha256-8, `pub mod` grep in both `lib.rs`, test-header grep
(`#[path]` vs `crate::`). Timeout 120 rtk. Priors: `worklog/EXT-TWINS-DISPOSITION.md`,
`worklog/SHARE-TWINS-DISPOSITION.md`, `worklog/TWINS-WATCH.md`,
`worklog/TWINS-WATCH2.md`, `worklog/TWINS-WATCH3.md`,
`worklog/TWINS-WATCH4.md`. Head: `248f519` (workdir dirty; all pair
measurements below are workdir files, same basis as WATCH4).

## 1. EXT pairs

| # | canonical (wired) | twin (unwired) | diff -q | diff-lines | wc -l | sha8 | class now | drift vs WATCH4 y/n |
|---|---|---|---|---|---|---|---|---|
| 1 | plugin_transform | ext_replay_lane | differ | 10 | 166 / 170 | b4c517e3 / f831dca8 | COMMENT-ONLY (fmt reflow + doc phrase) | n |
| 2 | plugin_namespace | ext_namespacing_lane | identical | 0 | 124 / 124 | 5eaa922a = 5eaa922a | BYTE-IDENTICAL | n |
| 3 | plugin_discover | ext_discovery_lane | differ | 34 | 160 / 128 | d7d7c5c0 / 30e24833 | COMMENT-ONLY | n |
| 4 | plugin_ui_boundary | ext_ui_boundary_lane | differ | 2 | 191 / 191 | 90883e97 / de389584 | COMMENT-ONLY (1 doc word) | n |
| 5 | plugin_builtins | ext_builtins_lane | differ | 3 | 267 / 268 | 62513b1d / 31f6966f | COMMENT-ONLY + `#![forbid(unsafe_code)]` twin-only | n |
| 6 | plugin_deferred | ext_deferred_lane | differ | 1 | 192 / 193 | c8bfbffb / cdad8c2f | COMMENT-ONLY + `#![forbid]` twin-only | n |
| 7 | plugin_lifecycle | ext_lifecycle_lane | differ | 5 | 183 / 188 | 4c7400f2 / 8bedf09a | COMMENT-ONLY (lane doc + `#![forbid]` twin-only) | n |
| 8 | plugin_scoped_exec | ext_scoped_exec_lane | identical | 0 | 286 / 286 | 4ddc1b58 = 4ddc1b58 | BYTE-IDENTICAL | n |

All sha8/wc/diff-lines byte-equal to TWINS-WATCH4 §1.

## 2. SHARE pairs

| # | canonical | twin | diff -q | diff-lines | wc -l | sha8 | class now | drift vs WATCH4 y/n |
|---|---|---|---|---|---|---|---|---|
| 9 | share_merge | share_merge_lane | differ | 130 | 228 / 232 | c97d3aab / 5bf3915e | DIVERGENT (Lane*-renamed) | n (keep-both, no shim) |
| 10 | share_queue | share_queue_lane | differ | 62 | 306 / 310 | 25057f59 / 10ef3449 | DIVERGENT (Lane*-renamed) | n (keep-both, no shim) |
| 11 | share_store | share_store_lane | differ | 4 | 248 / 248 | 418be003 / 7c32b321 | COMMENT-ONLY (2 doc words) | n (keep-both) |
| 12 | share_enterprise | share_enterprise_lane | differ | 32 | 101 / 125 | 7f7e7ac4 / b1bef9e0 | IMPL-DIVERGENT (thiserror vs manual Display + forbid) | n (keep-both) |
| 13 | share_policy_lane | share_policy2_lane | differ | 2 | 231 / 231 | 10bbb9fb / 0345fd4f | COMMENT-ONLY (1 doc line) | n (keep-both; never fold into `share_policy` enum) |

All values byte-equal to TWINS-WATCH4 §2.

## 3. plugin_transform guard-port — explicit flag

Guard PORTED both sides per WATCH4 §3 (workdir files re-measured identical
sha8/wc/dl, so bodies unchanged). Not re-diffed line-by-line this lane; no
evidence of change. Pair-1 droppable like pairs 2–8 (integrator call; NOT done here).

## 4. EXT-005 canonicalization — explicit flag

`plugin_manifest.rs` (55 lines, sha `c8c6682c`) vs `ext_manifest_lane.rs`
(142 lines, sha `e54e9647`): differ, 167 diff-lines. NOT duplicates —
different contracts (`PluginManifest{name,version,permissions}` +
`MAX_PERMISSIONS=32` vs EXT-005 `Manifest{name,contract_version,capabilities}` +
`validate_manifest_bytes`). Both KEEP. No change vs prior; left alone.

## 5. lib.rs wiring (which side) — DRIFT y (one line, third-party)

- `crates/tools/src/lib.rs`: wires all 10 `plugin_*` (:36-45) plus
  `plugin_hook_boundary`, `plugin_manifest`. **NEW vs WATCH4 §5:
  `pub mod ext_manifest_lane;` at :19 (workdir, uncommitted; `git diff HEAD`
  shows it as the single added mod line).** `grep -c lane` now = 1 (was 0).
  Pairs 1–8 twins still zero wired. Non-lane `ext_*` wired at :14-21
  (`ext_commands`, `ext_compat`, `ext_enable`, `ext_hooks`, `ext_lifecycle`,
  `ext_perms`, `ext_rate`, `ext_secure`) — third-party, not twins, not mine.
- `crates/sessions/src/lib.rs`: wires `share_merge`, `share_policy`,
  `share_queue` (:23-25) + `share_audit/count/expiry/invite/list/revoke/scope/token`,
  `share`, `remote_share`, `part_events`, `runner`, `tui_info_panel`
  (workdir additions vs HEAD `248f519`, already recorded in WATCH4 §5),
  ui_001-013. NOT wired: `share_store`, `share_enterprise`, any `*_lane`
  (`grep -c lane` = 0). No drift vs WATCH4 (third-party wiring, not mine).

## 6. Test headers (`#[path]` vs crate import)

- Tools twin suites: 19 files (`plugin_*` + `ext_*lane*`); 18 carry
  `#[path = "../src/<own>.rs"]`. Sole exception `plugin_manifest.rs` test
  imports via crate (`use opencode_rk_tools::plugin_manifest::...`) —
  correct, canonical is wired, not a lane twin. `grep -rn crate::` over all
  19 = 0 hits outside that expected import line (count 0 in twin files).
  No suite resolves its twin via `lib.rs`.
- Sessions lane suites (10 files, pairs 9–13): all `#[path]`-included
  (1–2 `#[path]` attrs each); `grep crate::` = 0.
  `crates/sessions/tests/share_policy.rs` (non-lane) imports via crate
  (`use opencode_rk_sessions::share_policy::...`) — wired module, expected.
- Stale-header note: `grep -n "NOT wired"` over `share_merge.rs` /
  `share_merge_lane.rs` now returns nothing (WATCH4 §6 quote gone —
  workdir `M` on `share_merge.rs` touched headers; harmless either way,
  integrator call).

## 7. WEB full suites — carried flag (not re-verified, read-only lane)

Per WATCH4 §7: WEB-014 T01-T05 in `crates/sessions/tests/chat_nav_lane.rs`,
WEB-017 T01-T05 in `crates/server/tests/web_artifact.rs`, frozen GREEN;
WEB-VERIFY4 total 107 passed, 0 failed (log `/tmp/opencode/yK-web.log`).
No cargo runs per this task; no change claimed.

## 8. Verdict

Drift rows: 0 of 13 pair bodies vs TWINS-WATCH4 (all sha8/wc/diff-lines identical).
Wiring drift: **y, one line** — `ext_manifest_lane` now wired in
`crates/tools/src/lib.rs:19` (workdir, third-party; EXT-005 lane, not pairs 1–8).
Vs EXT-TWINS-DISPOSITION: 1 resolved drift (pair-1 guard port, recorded since
TWINS-WATCH). GREEN: n/a (read-only lane, no cargo runs per task).

## Files written by this lane

- `worklog/TWINS-WATCH5.md`, `worklog/STUB-FMT-9.md` only (owned files).
