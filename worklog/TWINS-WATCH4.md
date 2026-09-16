# TWINS-WATCH4: EXT + SHARE drift re-survey (READ-ONLY, 2026-09-16)

Scope: 8 EXT pairs (`crates/tools/src/plugin_*` vs `ext_*_lane`) + 5 SHARE
pairs (`crates/sessions/src/share_*` vs `*_lane`, incl. policy_lane vs
policy2_lane). No code touched, no cargo runs, no deletions, no lib.rs/test
edits. Method: `diff -q`, `diff | grep -c '^[<>]'`, `wc -l`, sha256-8,
`pub mod` grep in both `lib.rs`, test-header grep (`#[path]` vs `crate::`).
Timeout 120. Priors: `worklog/EXT-TWINS-DISPOSITION.md`,
`worklog/SHARE-TWINS-DISPOSITION.md`, `worklog/TWINS-WATCH.md`,
`worklog/TWINS-WATCH2.md`, `worklog/TWINS-WATCH3.md`. Head: `248f519`.

## 1. EXT pairs

| # | canonical (wired) | twin (unwired) | diff -q | diff-lines | wc -l | sha8 | class now | drift vs WATCH3 y/n |
|---|---|---|---|---|---|---|---|---|
| 1 | plugin_transform | ext_replay_lane | differ | 10 | 166 / 170 | b4c517e3 / f831dca8 | COMMENT-ONLY (fmt reflow + doc phrase) | n |
| 2 | plugin_namespace | ext_namespacing_lane | identical | 0 | 124 / 124 | 5eaa922a = 5eaa922a | BYTE-IDENTICAL | n |
| 3 | plugin_discover | ext_discovery_lane | differ | 34 | 160 / 128 | d7d7c5c0 / 30e24833 | COMMENT-ONLY | n |
| 4 | plugin_ui_boundary | ext_ui_boundary_lane | differ | 2 | 191 / 191 | 90883e97 / de389584 | COMMENT-ONLY (1 doc word) | n |
| 5 | plugin_builtins | ext_builtins_lane | differ | 3 | 267 / 268 | 62513b1d / 31f6966f | COMMENT-ONLY + `#![forbid(unsafe_code)]` twin-only | n |
| 6 | plugin_deferred | ext_deferred_lane | differ | 1 | 192 / 193 | c8bfbffb / cdad8c2f | COMMENT-ONLY + `#![forbid]` twin-only | n |
| 7 | plugin_lifecycle | ext_lifecycle_lane | differ | 5 | 183 / 188 | 4c7400f2 / 8bedf09a | COMMENT-ONLY (lane doc + `#![forbid]` twin-only) | n |
| 8 | plugin_scoped_exec | ext_scoped_exec_lane | identical | 0 | 286 / 286 | 4ddc1b58 = 4ddc1b58 | BYTE-IDENTICAL | n |

All sha8/wc/diff-lines byte-equal to TWINS-WATCH3 §1.

## 2. SHARE pairs

| # | canonical | twin | diff -q | diff-lines | wc -l | sha8 | class now | drift vs WATCH3 y/n |
|---|---|---|---|---|---|---|---|---|
| 9 | share_merge | share_merge_lane | differ | 130 | 228 / 232 | c97d3aab / 5bf3915e | DIVERGENT (Lane*-renamed) | n (keep-both, no shim) |
| 10 | share_queue | share_queue_lane | differ | 62 | 306 / 310 | 25057f59 / 10ef3449 | DIVERGENT (Lane*-renamed) | n (keep-both, no shim) |
| 11 | share_store | share_store_lane | differ | 4 | 248 / 248 | 418be003 / 7c32b321 | COMMENT-ONLY (2 doc words) | n (keep-both) |
| 12 | share_enterprise | share_enterprise_lane | differ | 32 | 101 / 125 | 7f7e7ac4 / b1bef9e0 | IMPL-DIVERGENT (thiserror vs manual Display + forbid) | n (keep-both) |
| 13 | share_policy_lane | share_policy2_lane | differ | 2 | 231 / 231 | 10bbb9fb / 0345fd4f | COMMENT-ONLY (1 doc line) | n (keep-both; never fold into `share_policy` enum) |

All values byte-equal to TWINS-WATCH3 §2.

## 3. plugin_transform guard-port — explicit flag

Guard PORTED both sides, bodies byte-identical (re-verified this lane):
`plugin_transform.rs:128-139` vs `ext_replay_lane.rs:132-143` both carry
`if !self.disabled.contains(&scope) && self.entries.iter().any(|t| t.scope == scope)`
plus invariant doc (bounded by `EXT11_MAX_TRANSFORMS`). Pair-1 droppable like
pairs 2-8 (integrator call; NOT done here).

## 4. EXT-005 canonicalization — explicit flag

`plugin_manifest.rs` (55 lines) vs `ext_manifest_lane.rs` (142 lines):
differ, 167 diff-lines. NOT duplicates — different contracts
(`PluginManifest{name,version,permissions}` + `MAX_PERMISSIONS=32` vs
EXT-005 `Manifest{name,contract_version,capabilities}` +
`validate_manifest_bytes`; see `worklog/EXT-005.md:8-13`,
`worklog/EXT-DEDUP.md`). Both KEEP. No change vs prior; left alone.

## 5. lib.rs wiring (which side)

- `crates/tools/src/lib.rs:35-44`: wires all 10 `plugin_*`. Zero
  `ext_*_lane` wired (`grep -c lane` = 0). Non-lane `ext_*` wired at :14-21
  (`ext_commands`, `ext_compat`, `ext_enable`, `ext_hooks`, `ext_lifecycle`,
  `ext_perms`, `ext_rate`, `ext_secure`) — third-party, not twins, not mine.
- `crates/sessions/src/lib.rs:23-25`: wires `share_merge`, `share_policy`,
  `share_queue` (+ `share_audit/count/expiry/invite/list/revoke/scope/token`,
  `part_events`, `runner`, `tui_info_panel`, ui_001-013). NOT wired:
  `share_store`, `share_enterprise`, any `*_lane` (`grep -c lane` = 0).
  Unchanged vs WATCH3 (third-party wiring, not mine).

## 6. Test headers (`#[path]` vs crate import)

- All 16 tools twin suites `#[path = "../src/<own>.rs"]` + local `mod`;
  `grep -rn crate::` over all 16 = 0 hits. No suite resolves via `lib.rs`.
- All 10 sampled sessions lane suites `#[path]`-included; `grep crate::` = 0.
  `crates/sessions/tests/share_policy.rs` (non-lane) imports via crate
  (`use opencode_rk_sessions::share_policy::...`) — wired module, expected.
- Stale-header note stands: `share_merge.rs` / `share_merge_lane.rs` headers
  claim "NOT wired into `lib.rs`" but `lib.rs:23` wires `share_merge` —
  harmless, integrator call.

## 7. WEB full suites — explicit flag

Full suites exist, frozen, GREEN (NOT 2-test-only):
- WEB-014 T01-T05: `crates/sessions/tests/chat_nav_lane.rs` (5 tests
  `web014_t01..t05`) — GREEN per `worklog/WEB-014-017-SUITES.md` + WEB-VERIFY4.
- WEB-017 T01-T05: `crates/server/tests/web_artifact.rs` (5 tests
  `artifact_t01..t05`) — GREEN per same.
- Supplemental 2-test HTTP boundary files (`session_history_api.rs`,
  `web_artifact_api.rs`) are additive, not replacements.
- WEB-VERIFY4 total: 107 passed, 0 failed (log `/tmp/opencode/yK-web.log`).

## 8. Verdict

Drift rows: 0 of 13 vs TWINS-WATCH3 (all sha8/wc/diff-lines identical).
Vs EXT-TWINS-DISPOSITION: 1 resolved drift (pair-1 guard port, recorded since
TWINS-WATCH). GREEN: n/a (read-only lane, no cargo runs per task).

## Files written by this lane

- `worklog/TWINS-WATCH4.md`, `worklog/STUB-FMT-8.md` only (owned files).
