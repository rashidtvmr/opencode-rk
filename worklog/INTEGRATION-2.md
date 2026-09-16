# INTEGRATION-2 — final fragment list + dedupe + flip requests

Rev: `248f519` HEAD. Date: 2026-09-16. Owner: integration lane.
Bounds: read-only except this file. No `ralph.json`/controller/lib.rs edits.
Supersedes: `worklog/INTEGRATION-PROPOSAL.md` (2026-09-15, pre-yesterday-dedupe).
Inputs: `worklog/GUARD-TRIAGE-2.md`, `worklog/EXT-DEDUP.md`,
`worklog/EXT-009-DEDUP.md`, `worklog/EXT-TYPE-UNIFY.md`,
`worklog/WEB-007-012-ACCEPT.md`, `worklog/WEB-016-DECISION.md`,
`worklog/ACP-001.md`, `worklog/ACP-002.md`, `worklog/SDK-001.md`,
`worklog/WSX-002.md`, `worklog/UI019-TOOL016-020-SYNC-RUN.md`,
`worklog/WEB-013-PROV-016.md`, `worklog/REL-001/002/003.md`,
`worklog/WEB-006.md`, `worklog/WEB-009.md`, `worklog/WEB-LIVE.md`.

## 1. Guard result (unchanged class)

- `tools/validate_repository.py` exit=1, 122 backlog-exhaustion errors
  (GUARD-TRIAGE-2: log `/tmp/opencode/validate2.log`).
- `tools/validate_plan.py` exit=1, 133 errors (11 extra = unknown prefixes
  SYNC/RUN/ACP/WSX/SDK/HEAD; log `/tmp/opencode/plan2.log`).
- `git diff --check` clean. `git status`: 183 modified + 91 untracked
  (`/tmp/opencode/status2.txt`).
- Triage holds: stale-FEATURES/accepted + unknown-prefix lines are
  pre-existing controller sync work; lane-output task/worklog deltas
  (OPS/REL/SHARE/EXT/INT/WEB/PROV/ACP/SYNC/RUN/WSX/SDK/TOOL/UI) await
  verifier accept, not regressions.

## 2. Final lib.rs fragment list (integrator assembles, lane does not edit)

### 2a. foundation — +2 mods (reorder only + 2 adds)
Current worktree `crates/foundation/src/lib.rs:1-31`:
`auto_budget, config, config_overlay, delegation, effect, inbuilt_optout,
install_meta, lifecycle, ops_budget, ops_guard, ops_health, ops_lock,
ops_metrics, ops_ping, ops_replay, ops_repo_ref, ops_runtime, ops_scope,
ops_seq, ops_state, rel_check, rel_gate, repo_cache_store [NEW],
repo_ref [NEW], repo_ref_ext, resource_ledger, runtime_report,
stats_collect, stats_report`.
Diff vs HEAD: pure reorder (`ops_lock`/`ops_runtime` alpha) + adds
`repo_cache_store`, `repo_ref` (both files exist in tree,
`repo_cache_store.rs`, `repo_ref.rs`). `repo_ref_ext.rs` pre-existing.
Keep as-is.

### 2b. server — acp/voice/sync + WEB-live set (all wired in worktree lib.rs:3-36)
Keep wired (files exist on disk):
- `acp_bridge` (`acp_bridge.rs` tracked-modified, ACP-001 lane;
  test `tests/acp_bridge.rs` hash `a783d15f…3f124f`).
- `acp_files` (`acp_files.rs` UNTRACKED, ACP-002 lane, impl `df9c73db…`;
  test `tests/acp_files.rs` frozen `6370fc0c…1ec74d57`, 5/5 GREEN via
  `#[path]`; needs `pub mod acp_files;` — already present worktree:3-4).
- `sync_log` (`sync_log.rs` UNTRACKED, SYNC lane; test `tests/sync_log.rs`
  frozen `9384ffdc…`, 5/5 GREEN). Keep `pub mod sync_log;` (:27).
- `voice_capture` (`voice_capture.rs` tracked, WEB-016 decision: read-only
  state machine, capability `available:false` unchanged lib.rs:164-167;
  test `tests/voice_capture.rs` 5/5). Keep `pub mod voice_capture;` (:28).
- WEB-live set (WEB-LIVE evidence, all wired): `control_plane_errors`,
  `control_plane_exposure`, `control_plane_inputs`, `event_stream`,
  `protocol_api` (:11-13,19,21) + routes `/events`, `/control/move_session`
  (:69-70) + `ConnectInfo<SocketAddr>` peer (:39,56).
- NOT wired (correct, `#[path]`-only lanes): `transcript_lane`,
  `turn_parts`, `chat_composer`, `web_attachments`, `web_tool_chooser`
  (WEB-007-012-ACCEPT §wiring boundary). Do NOT add.
- Untracked extras needing integrator decision (NOT in lib.rs, `#[path]`
  tests): `remote_sync.rs` (WSX-002, 5/5), `sdk_client.rs` (SDK-001, 12/12).
  Wire only after lane gate: `pub mod remote_sync;`, `pub mod sdk_client;`.

### 2c. sessions — +3 keep (part_events/runner/tui_info_panel)
Worktree `lib.rs:4-49` keeps: `pub mod part_events;` (:14), `pub mod runner;`
(:18), `pub mod tui_info_panel;` (:34) — all UNTRACKED new files, frozen
tests 5/5 each (`part_events cad1d938…`, `runner ba048213…`,
`tui_info_panel 0e5b1686…`). Keep.
Also wired pre-existing: `mcp_status_panel` (:11, UI-019 evidence 5/5).
Do NOT wire: `share_*_lane`, `web_008_lane`, `web_013`, `chat_nav_lane`
(all `#[path]`-only; §3).

### 2d. tools — shims, no new mods
Worktree `lib.rs` diff vs HEAD is one-line mod reorder
(`tool_quota`/`tool_sandbox` alpha) — no new mods needed.
Keep wired: `plugin_builtins, plugin_deferred, plugin_discover,
plugin_hook_boundary, plugin_lifecycle, plugin_manifest, plugin_namespace,
plugin_scoped_exec, plugin_transform, plugin_ui_boundary` + thin
`ext_commands, ext_compat, ext_enable, ext_hooks, ext_lifecycle, ext_perms,
ext_rate, ext_secure` + `mcp/mcp_bulk_actions/mcp_catalog_search/
mcp_lifecycle/mcp_payload_filter` (TOOL-016..019 lanes, tests 5/5 each).
Do NOT add any `ext_*_lane` mod: all 9 lane files are now 12-14-line
`#[path]+pub use` shims compiled via frozen tests only (§3).

### 2e. agents — +2 keep
`pub mod delegation_lane; pub mod driver_lane;` (worktree lib.rs:5-6).
COMPLIANCE-SWEEP notes both lanes GREEN-only (`#[path]` bypass, no valid
RED) — verifier must treat RED incomplete per TDD §3. Wiring harmless;
acceptance blocked on RED question, not on wiring.

## 3. Dedupe pick confirmations (yesterday decisions, byte evidence today)

Rule applied: one definition; `ext_*_lane` = shim, `plugin_*` = canonical.
lib.rs never wires shims. Frozen tests untouched (`#[path]` still compiles).

### tools/plugin_* (CONFIRMED — shims in place, hashes today)
- `ext_replay_lane.rs` is 14-line shim (header names canonical
  `plugin_transform`); canonical sha `f831dca8…aa08a54` unchanged from
  EXT-009-DEDUP pre-edit evidence. Shim sha `87286931…104535c52f` matches
  EXT-009-DEDUP post-edit hash exactly.
- `ext_namespacing_lane.rs` shim; canonical `plugin_namespace`
  `5eaa922a…150b1587` matches EXT-009-DEDUP pre-edit `5eaa922a…`; shim
  `bdf7870a…d37644d34a` matches post-edit hash exactly.
- `ext_ui_boundary_lane.rs` shim `5aa5145d…f6b5cbdfc` matches post-edit;
  canonical `plugin_ui_boundary` `90883e97…d60631b9` (drift: 1-line fmt
  hunk in worktree, behavior per EXT-009-DEDUP `129/129` non-comment equal).
- `ext_discovery_lane.rs` shim `0990ba2d…30b9ba3eda` matches post-edit;
  canonical `plugin_discover` `d7d7c5c0…28ddf3ee` (comment-only drift,
  `109/109` stripped equal per EXT-009-DEDUP).
- `ext_builtins_lane.rs` shim `fcabce3f…bd08b11f`; `ext_deferred_lane.rs`
  shim `5826dfca…dcb6928e`; `ext_lifecycle_lane.rs` shim
  `7d79f1ba…4e1749d0e`; `ext_scoped_exec_lane.rs` shim
  `cd1a28d4…cb9bdce99` — all 12-line `#[path]+pub use` form per EXT-DEDUP.
- `plugin_lifecycle.rs` canonical `4c7400f2…a6a791` byte-stable (matches
  EXT-DEDUP + EXT-TYPE-UNIFY base).
- `plugin_builtins.rs` `3fbe08fb…3e305ee1d` = type-unified form
  (EXT-TYPE-UNIFY: -165 net lines, `crate::plugin_lifecycle` re-export in
  lib build, `#[path]` include in test build; frozen tests untouched).
- `ext_manifest_lane.rs` `e54e9647…d758d4b8d` left alone: divergent contract
  vs `plugin_manifest.rs` (`c8c6682c…`, name/version/permissions vs
  EXT-005 name/contract_version/capabilities) per EXT-DEDUP §Not-duplicates.
- `ext_hooks.rs` vs `plugin_hook_boundary.rs` (`fcb2e08a…`): divergent
  contracts, left alone per EXT-DEDUP.
- Pick: keep `plugin_*` wired; integrator deletes the 9 shims only after
  lane tests migrate to canonical `#[path]`. No deletion in this lane.

### sessions/share_* (CONFIRMED — Lane* twins stay unwired)
- All six lane files keep distinct `Lane*` types + `LANE_MAX_*` consts, none
  wired in lib.rs (`grep pub mod share_*_lane lib.rs` = zero):
  `share_merge_lane.rs`, `share_queue_lane.rs`, `share_policy_lane.rs`,
  `share_policy2_lane.rs` (fallback duplicate of policy_lane — two lanes
  claim SHARE-005; both `#[path]`-only, integrator picks one post-accept),
  `share_store_lane.rs`, `share_enterprise_lane.rs`.
- Canonical `share_merge.rs`/`share_queue.rs` carry tracked Debug-redact
  work (`payload_len`/`value_len`); lane twins lack it → keep canonical.
- Pick: keep canonical `share_*.rs` wired; drop `*_lane` twins after lane
  tests rewrite to owned names. No deletion in this lane.

## 4. ralph.json flip request table (EVIDENCE ONLY — do NOT edit ralph.json)

Current statuses read 2026-09-16 (258 stories, 176 accepted).
Requested flips are controller+verifier-gated.

| Story | ralph now | Request | Evidence path |
|---|---|---|---|
| REL-001 | in-progress | controller review → accepted? validator-only slice GREEN | `worklog/REL-001.md`, validator hash `ab9b350e…`, T01..T05 exits `0,2,2,0,2`, reports `/tmp/opencode/rel001-t01..t05.json` (+rerun `/tmp/opencode/rerun-rel001-t01..t05.json`), controller hash `ed58eaf0…` unchanged, T04 zero `"accepted":true` bytes |
| REL-002 | in-progress | same | `worklog/REL-002.md`, hash `79be6ef1…`, exits `0,2,2,2,2`, `/tmp/opencode/rel002-t01..t05.json` + `rel002-pass.json` |
| REL-003 | in-progress | same | `worklog/REL-003.md`, hash `3a462032…`, exits `0,2,2,2,2,2,2`, `/tmp/opencode/rel003-*.json`, canary `REL003-CANARY-9f8e7d6c5b4a` grep-count 0 |
| ACP-001 | not-started | review → in-progress? codec GREEN, NOT ACCEPTED | `worklog/ACP-001.md`, `crates/server/src/acp_bridge.rs` (now tracked-modified, wired `pub mod acp_bridge;`), test hash `a783d15f…3f124f` 5/5 |
| ACP-002 | not-started | review → in-progress? impl GREEN, wiring present in worktree | `worklog/ACP-002.md`, `crates/server/src/acp_files.rs` (untracked, impl `df9c73db…`), test frozen `6370fc0c…1ec74d57` 5/5, `pub mod acp_files;` present |
| SDK-001 | not-started | review → in-progress? 12/12 GREEN via `#[path]`, unwired | `worklog/SDK-001.md`, `crates/server/src/sdk_client.rs` untracked, suite 12 passed |
| WSX-002 | not-started | review → in-progress? 5/5 GREEN via `#[path]`, unwired | `worklog/WSX-002.md`, `crates/server/src/remote_sync.rs` untracked, logs `/tmp/opencode/wsx002_{red,green,check}.log` |
| SYNC-001/002 (sync_log) | not-started | review → in-progress? 5/5 GREEN, wired | `worklog/UI019-TOOL016-020-SYNC-RUN.md`, test frozen `9384ffdc…`, `pub mod sync_log;` wired |
| RUN-001 (runner) | not-started | review → in-progress? 5/5 GREEN, wired | same worklog, test frozen `ba048213…`, `pub mod runner;` wired |
| TOOL-016..019 | not-started | review → in-progress? pre-existing product, new frozen tests 5/5 each | same worklog, frozen `5e3accbe…/fa0375f4…/b605f0c9…/02b72d40…` |
| UI-019 (mcp_status_panel + tui_info_panel) | not-started | review → in-progress? 5/5+5/5 GREEN | same worklog, frozen `909ff595…/0e5b1686…` |
| WEB-007..012 | not-started | HOLD at not-started (models GREEN, write-paths disabled) | `worklog/WEB-007-012-ACCEPT.md`: six suites 5/5, hashes `f46f26de…/f63116f7…/b5874654…/0ef115dd…/328c4d65…/f69d4a34…`, no `pub mod` wiring — verifier must still check browser suite + producers (§5) |
| WEB-013..017 + PROV-015/016 | not-started | HOLD (boundary evidence only; PROV tests GREEN-only, no valid RED) | `worklog/WEB-013-PROV-016.md` (tests `ac548d6e…/dd08f305…` 10/10 GREEN-only), `worklog/WEB-016-DECISION.md` (voice read-only wiring, `available:false`) |
| PROV-017..024 | not-started | HOLD (untracked tests exist, no owned src wired) | `crates/providers/tests/prov_*.rs` + `auth_profile.rs`/`codex_oauth.rs` untracked; INTEGRATION-PROPOSAL §4 |
| OPS-001..009, SHARE-001..005, EXT-001/002/004..006/008..012, INT-001..003/005..007/009/010, AUTO-004..006 | in-progress | HOLD for verifier accept (lane outputs present, gates pending) | per-lane worklogs; COMPLIANCE-SWEEP flags AUTO-004/006 RED-incomplete (`#[path]` bypass), AUTO-005 no RED/GREEN (declarative) |
| Accepted-but-stale FEATURES.md set (ROUTE-012/REL-004/UI-014..018 + ROUTE-001..011/UI-001..013/etc.) | accepted | FEATURES.md sync only (controller/integration authority) | `/tmp/opencode/validate2.log` stale-status lines |
| PROV-015 | not-started | controller: classify (`non-accepted story has no validator classification` = validator gap) | GUARD-TRIAGE-2 §triage |
| Unknown prefixes SYNC/RUN/ACP/WSX/SDK/HEAD | n/a | controller: allowlist-or-map to crates + reconcile FEATURES.md/WEB-006/007/008 accounting + ROUTE-009/010 ownership gaps | `/tmp/opencode/plan2.log` 11 extra |

## 5. REL-001..003 verifier checklist status

- REL-001 (`tools/check_release_accounting.py --snapshot <dir> --out <report>`):
  T01 complete→exit 0; T02 dropped edge→exit 2 `missing∋REL-001`; T03 bare
  numbers→exit 2 `inference-without-evidence`; T04 not-accepted→exit 0 + zero
  `"accepted":true` bytes + controller hash `ed58eaf0…` unchanged; T05
  pins-only→exit 2 `duplicate-of-existing-owner`; determinism byte-identical;
  tool errors→exit 1 no report. Caps 256-file/8MiB in, 64KiB report.
  Status: lane GREEN (rerun 2026-09-15 clean) — verifier re-run pending,
  then controller flip.
- REL-002 (`tools/check_release_tdd.py --revision --manifest --receipts --gates --out`):
  T01→exit 0 all four (`red_proof,frozen_intact,green_on_frozen,
  verifier_rerun`); T02 no-red→exit 2; T03 mutated→exit 2; T04
  worker-only/wrong-rev→exit 2
  (`self-report-not-evidence`/`verifier-revision-mismatch`); T05
  pipeline-only→exit 2 `duplicate-of-existing-owner:AUTO-005`; blocked→exit 1
  `blocked-pending-authority`. Status: lane GREEN — verifier re-run pending.
- REL-003 (`tools/check_release_safety.py --revision --gates --caps --quotas --out`):
  T01→exit 0 all four
  (`mandatory_intact,denied_no_side_effects,bytes_bounded,quotas_enforced`);
  T02 `*`-bypass→exit 2; T03a/b fs/proc→exit 2; T04a oversize / T04b
  deletion→exit 2; T05 tdd-only→exit 2 `unverified-release-state`; canary
  `REL003-CANARY-9f8e7d6c5b4a` absent; secret-bearing report refused exit 1.
  Status: lane GREEN — verifier re-run pending.

## 6. Remaining blockers

1. `acp_files` production trait wiring: module is pure broker-trait boundary
   (no FS I/O inside); real `FileReader`/`FileWriter`→FS adapter unwired
   (ACP-002 §Remaining unknowns). `pub mod` present; adapter is integrator
   work, then verifier accept.
2. `turn_stream` flaky-under-parallel (pre-existing, not a code bug):
   `session_turn_stream_api` 2/2 GREEN only serial (`--test-threads=1`,
   process-global provider fixture; parallel run fails) — WEB-006:36-37,
   WEB-009:7. Same class: `ext_builtins_lane` T05 thread-count assert needs
   serial (EXT-DEDUP §Remaining drift). Verifier must run these targets
   serial; do not "fix" by editing frozen tests.
3. Four task cards with no worklog and no lane output (missing work, not
   flips): HEAD-001, HEAD-002, SDK-002, RUN-001-as-standalone (RUN covered
   only inside combined `UI019-TOOL016-020-SYNC-RUN.md`; 144/216 task cards
   have no worklog at all — full missing list derivable via
   `tasks/`∖`worklog/`; these four are the in-scope-track ones called out
   in the lease). No code, no tests, no evidence — needs fresh lease or
   explicit controller scope-cut before release.
4. WEB-007..012 write-path producers absent (deliberate, honesty-held):
   WEB-009 tool-call producer + citation contract; WEB-010 HTTP chip/queue
   contracts; WEB-011 unified blob store + provider adapter; WEB-012
   execution-owned approvals; all-six browser `vitest`/`tsc` suite
   unexecuted (WEB-007-012-ACCEPT §verifier-must-check). Stories stay
   NOT ACCEPTED until producers land.
5. Merge order (unchanged from INTEGRATION-PROPOSAL §6): (i) integrator
   deletes `*_lane` shims only after test migration; (ii) one integration
   commit for tracked fixes + lib.rs wirings, bounded
   `timeout 120 CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p
   opencode-rk-sessions -p opencode-rk-tools -p opencode-rk-server` serial
   for the two known-flaky targets; (iii) untracked modules added only with
   passing lane gates; (iv) controller syncs FEATURES.md/accounting + flips.
