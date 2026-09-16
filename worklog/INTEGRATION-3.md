# INTEGRATION-3 — final fragment checklist + dedupe + flip requests (v3)

Rev: `248f519` HEAD. Date: 2026-09-16. Owner: integration lane.
Bounds: read-only except this file. No `ralph.json`/controller/lib.rs/product edits.
Supersedes: `worklog/INTEGRATION-2.md` where disk disagrees (corrections marked CORR).
Inputs: `worklog/INTEGRATION-2.md`, `worklog/GUARD-TRIAGE-3.md`,
`worklog/PROV-017-024.md` (fix section), `worklog/SDK-HEAD-STATUS.md`,
`worklog/WEB-007-012-ACCEPT.md`, `worklog/WEB-013-PROV-016.md`,
`worklog/WEB-016-DECISION.md`, `worklog/UI019-TOOL016-020-SYNC-RUN.md`,
`worklog/ACP-001.md`, `worklog/ACP-002.md`, `worklog/SDK-001.md`,
`worklog/WSX-002.md`, `worklog/HEAD-002.md`, `worklog/EXT-DEDUP.md`,
`worklog/EXT-009-DEDUP.md`, `worklog/EXT-TYPE-UNIFY.md`, `worklog/REL-001/002/003.md`.
No `TURN-STREAM-GATE.md` exists in `worklog/` (glob `TURN*` empty); turn_stream
evidence taken from INTEGRATION-2 §6 + `crates/server/src/lib.rs:53` on disk.

Guard (this lane, `timeout 110 python3 tools/validate_repository.py`):
exit=1, backlog-exhaustion class (tail: FEATURES UI-015..018 stale
`not-started` vs accepted). GUARD-TRIAGE-3 (122 errors repo / 133 plan,
11 extra = SYNC/RUN/ACP/WSX/SDK/HEAD) holds; no new guard drift claimed.

## 1. Final lib.rs fragment checklist (on-disk truth, integrator assembles)

### 1a. foundation — +0 wired (CORR vs INTEGRATION-2 §2a)
- On-disk `crates/foundation/src/lib.rs:3-29`: reorder-only diff vs HEAD
  (`ops_lock`/`ops_runtime` alpha). NO `repo_cache_store`/`repo_ref` mods.
- Files `repo_cache_store.rs`, `repo_ref.rs` exist on disk but TRACKED-MODIFIED
  and UNWIRED. `repo_ref_ext.rs` pre-existing, wired.
- Integrator decision: wire `pub mod repo_cache_store;` + `pub mod repo_ref;`
  only with passing lane gates; else leave unwired. Do NOT treat as done.

### 1b. server — acp/voice/sync all UNWIRED (CORR vs INTEGRATION-2 §2b)
- On-disk `crates/server/src/lib.rs:3-27`: 25 mods only (`app_client`..
  `web_suffix`). Zero `acp_*`/`sync_log`/`voice_capture`/`control_plane_*`/
  `event_stream`/`protocol_api`/`remote_sync`/`sdk_client` mods. `git diff`
  vs HEAD for this file is EMPTY (unmodified); INTEGRATION-2's wired
  worktree (:3-36) is not present.
- Disk-present, unwired, `#[path]`-testable lane files:
  `acp_bridge.rs`, `acp_files.rs`, `remote_sync.rs`, `sdk_client.rs`,
  `sdk_spawns.rs` (NEW since SDK-HEAD-STATUS — was MISSING),
  `sync_log.rs`, `chat_composer.rs`, `transcript_lane.rs`, `turn_parts.rs`,
  `web_attachments.rs`, `web_tool_chooser.rs`, `web_artifact.rs`,
  `web_entry_probe.rs`.
- Voice: `web_capabilities()` (:163-166) still reports `"voice":{"available":
  false}` — read-only posture per WEB-016-DECISION holds, no `voice_capture`
  module on disk (file absent from `ls`).
- Turn routes wired: `/turns` + `/turns/stream` (:107-108),
  `TURN_PERMITS = Semaphore::const_new(2)` (:53). Serial-only caveat §4.
- Integrator adds (each gated): `pub mod acp_bridge;`, `pub mod acp_files;`,
  `pub mod sync_log;`, `pub mod remote_sync;`, `pub mod sdk_client;`,
  `pub mod sdk_spawns;`.

### 1c. sessions — +0 wired, 3 files present-unwired (CORR vs INTEGRATION-2 §2c)
- On-disk `lib.rs:4-46`: `mcp_status_panel` wired (:11); NO `part_events`/
  `runner`/`tui_info_panel` mods (diff vs HEAD = fmt-only + `pub use`
  reorder, §evidence below). Files exist UNTRACKED (`part_events.rs`,
  `runner.rs`, `tui_info_panel.rs`).
- Canonical `share_*.rs` wired (:17-28 excl. `share_store.rs` — wired as
  `share_store_lane`? NO: `share_store.rs` file exists, lib.rs has NO
  `share_store` mod; only `share`, `share_audit`, `share_count`,
  `share_expiry`, `share_invite`, `share_list`, `share_merge`,
  `share_policy`, `share_queue`, `share_revoke`, `share_scope`,
  `share_token`). Lane twins (§2) all unwired — correct.
- Do NOT wire: `share_*_lane`, `web_008_lane`, `web_013`, `chat_nav_lane`
  (all `#[path]`-only).
- Integrator adds (gated): `pub mod part_events;`, `pub mod runner;`,
  `pub mod tui_info_panel;`.

### 1d. tools — shims NOT shims: full modules, unwired (CORR vs INTEGRATION-2 §2d/§3)
- On-disk `lib.rs:4-65`: NO `ext_*_lane` mods; thin `ext_commands`,
  `ext_compat`, `ext_enable`, `ext_hooks`, `ext_lifecycle`, `ext_perms`,
  `ext_rate`, `ext_secure` + all 10 `plugin_*` + `mcp*` wired.
- Disk `ext_*_lane.rs` files are FULL modules (doc headers show real
  contracts, e.g. `ext_builtins_lane.rs` "Fixed auditable table…",
  `ext_scoped_exec_lane.rs` broker-gated exec), NOT 12-14-line
  `#[path]+pub use` shims. INTEGRATION-2 §3 shim hashes do NOT describe
  current disk. Dedupe rule still stands (one definition; `plugin_*` =
  canonical, lane files stay unwired), but integrator must NOT delete lane
  files as "shims" — they are independent lane implementations awaiting
  test-migration-then-delete.
- `ext_manifest_lane.rs` vs `plugin_manifest.rs`: divergent contracts,
  left alone (EXT-DEDUP §Not-duplicates, unchanged).

### 1e. agents — +0 wired (CORR vs INTEGRATION-2 §2e)
- On-disk `lib.rs:5-7`: `executor, message, turn_state` only.
  `delegation_lane.rs`, `driver_lane.rs` exist on disk (MODIFIED per status)
  but UNWIRED. Fresh files `context.rs`, `session.rs` also present, unwired.
- COMPLIANCE-SWEEP RED-incomplete flag on both lanes still applies;
  wiring harmless but acceptance blocked on RED question, not wiring.

## 2. Dedupe confirmations

- EXT (8 files: `ext_builtins/replay/namespacing/ui_boundary/discovery/
  deferred/lifecycle/scoped_exec _lane`): keep-both on disk, canonical
  `plugin_*` wired, lane files unwired. NO deletion in any lane; integrator
  deletes only after lane tests migrate to canonical `#[path]`.
- Share twins NOT convertible — keep-both recorded: `grep "pub struct Lane"`
  hits only `share_merge_lane.rs` + `share_queue_lane.rs`; `share_policy_lane`,
  `share_policy2_lane`, `share_store_lane`, `share_enterprise_lane` use
  non-`Lane` symbol sets incompatible with canonical `share_policy.rs`/
  `share_store.rs`/etc. Canonical keeps Debug-redact (`payload_len`/
  `value_len`); twins lack it → keep canonical wired. `share_policy2_lane`
  is fallback duplicate of `share_policy_lane` (two lanes claim SHARE-005);
  integrator picks one post-accept. No deletion in lanes.

## 3. ralph.json flip request table (EVIDENCE ONLY — do NOT edit ralph.json)

Read 2026-09-16: 258 stories, 176 accepted / 44 in-progress / 38 not-started.

| Story | ralph now | Request | Evidence path |
|---|---|---|---|
| REL-001 | in-progress | controller review → accepted? validator-only slice | `worklog/REL-001.md`, validator `ab9b350e…`, T01..T05 `0,2,2,0,2` |
| REL-002 | in-progress | same | `worklog/REL-002.md`, `79be6ef1…`, `0,2,2,2,2` |
| REL-003 | in-progress | same | `worklog/REL-003.md`, `3a462032…`, `0,2,2,2,2,2,2`, canary absent |
| ACP-001 | not-started | review → in-progress? codec GREEN, unwired | `worklog/ACP-001.md`, `server/src/acp_bridge.rs` untracked, test `a783d15f…` 5/5 |
| ACP-002 | not-started | review → in-progress? impl GREEN, unwired | `worklog/ACP-002.md`, `server/src/acp_files.rs` untracked, `6370fc0c…` 5/5 |
| SDK-001 | not-started | review → in-progress? 12/12 GREEN, unwired | `worklog/SDK-001.md`, `server/src/sdk_client.rs` untracked |
| SDK-002 | not-started | review → in-progress? NEW file+test on disk | `server/src/sdk_spawns.rs` + `server/tests/sdk_spawns.rs` (untracked; SDK-HEAD-STATUS said MISSING) |
| WSX-002 | not-started | review → in-progress? 5/5 GREEN, unwired | `worklog/WSX-002.md`, `server/src/remote_sync.rs` untracked |
| SYNC slice | not-started (SYNC-001/002) | review → in-progress? files on disk, unwired | `server/src/sync_log.rs` + `sessions/src/part_events.rs`, frozen `9384ffdc…` |
| RUN-001 | not-started | review → in-progress? file on disk, unwired | `sessions/src/runner.rs`, frozen `ba048213…` |
| TOOL-016..019 | not-started | review → in-progress? pre-existing product, new tests | `worklog/UI019-TOOL016-020-SYNC-RUN.md`, `5e3accbe…/fa0375f4…/b605f0c9…/02b72d40…` |
| UI-019 | not-started | review → in-progress? | same worklog, `909ff595…/0e5b1686…` (`tui_info_panel.rs` on disk, unwired) |
| HEAD-002 | not-started | review → in-progress? NEW file+tests on disk | `cli/src/session_export.rs` + `cli/tests/session_export.rs`, `worklog/HEAD-002.md` |
| PROV-017..024 | not-started | HOLD — fix NOT on disk (see §5) | 8 frozen `prov_*.rs` tests untracked; product fix absent |
| WEB-007..012 | not-started | HOLD (models GREEN, write-paths disabled) | `worklog/WEB-007-012-ACCEPT.md`, no `pub mod` wiring |
| WEB-013..017 + PROV-015/016 | not-started | HOLD (GREEN-only, no valid RED) | `worklog/WEB-013-PROV-016.md`, `WEB-016-DECISION.md` |
| OPS/SHARE/EXT/INT/AUTO in-progress set | in-progress | HOLD for verifier accept | per-lane worklogs; GUARD-TRIAGE-3 carried drift |
| FEATURES stale-accepted set | accepted | FEATURES.md sync only (controller authority) | validate log stale-status lines |
| Unknown prefixes SYNC/RUN/ACP/WSX/SDK/HEAD | n/a | controller: allowlist-or-map + reconcile accounting | plan log 11 extra |

## 4. REL-001..003 checklist

- REL-001 `tools/check_release_accounting.py`: T01 exit 0; T02 exit 2
  `missing∋REL-001`; T03 exit 2 `inference-without-evidence`; T04 exit 0 +
  zero `"accepted":true` bytes, controller hash `ed58eaf0…` unchanged; T05
  exit 2 `duplicate-of-existing-owner`. Lane GREEN — verifier re-run pending.
- REL-002 `tools/check_release_tdd.py`: T01 exit 0 all four; T02–T05 exit 2
  (`no-red`, mutated, worker-only/wrong-rev, pipeline-only); blocked exit 1.
  Lane GREEN — verifier re-run pending.
- REL-003 `tools/check_release_safety.py`: T01 exit 0 all four; T02–T05
  exit 2 (`*`-bypass, fs/proc, oversize/deletion, tdd-only); canary
  `REL003-CANARY-9f8e7d6c5b4a` absent. Lane GREEN — verifier re-run pending.

## 5. Blockers ranked

1. PROV-017 fix PENDING (not applied): `PROV-017-024.md` fix section claims
   `begin_login` embeds raw `LOOPBACK_REDIRECT_URI`; on-disk
   `crates/providers/src/claude_oauth.rs:458-464` STILL emits percent-encoded
   `redirect_uri=http%3A%2F%2F127.0.0.1%3A1455%2Foauth%2Fcallback`, and
   `git status` shows the file UNMODIFIED (only the new `prov_017` test is
   untracked). The 8 PROV suites cannot all be GREEN on this tree — PROV-017
   RED baseline (`BadConsentUrl`) still reproduces. Owner lane must apply
   the one-line fix, then verifier re-runs `prov_017`.
2. WSX-001/HEAD-001 MISSING (no code, no tests): `server/src/workspace_proxy.rs`
   and `cli/src/run_headless.rs` absent (`ls` fails both). WSX-002 exists but
   depends on WSX-001 (`dependencyIds: ['WSX-001']`); HEAD-001 has no lane
   output at all. Needs fresh lease or explicit controller scope-cut.
3. turn_stream serial-only (pre-existing, not a code bug): `session_turn_stream_api`
   2/2 GREEN only with `--test-threads=1` (process-global provider fixture;
   `TURN_PERMITS` cap 2 does not fix test parallelism). Same class:
   `ext_builtins_lane` T05 thread-count assert needs serial. Verifier must run
   these targets serial; do NOT "fix" by editing frozen tests.
4. Four-task close still open: HEAD-001 (missing), HEAD-002 (NEW on disk,
   needs gate), SDK-002 (NEW on disk, needs gate), RUN-001-as-standalone
   (covered only inside combined `UI019-TOOL016-020-SYNC-RUN.md`; no
   standalone worklog). SDK-HEAD-STATUS §"4/8 missing" is now 2/8 missing
   (`sdk_spawns.rs` + `session_export.rs` landed with tests) — status file
   itself is stale and needs a refresh lane.
5. Merge order (unchanged): (i) PROV-017 owner applies fix; (ii) lane gates
   GREEN per lane for all untracked modules; (iii) ONE integration commit for
   tracked fixes + lib.rs wirings from §1, bounded `timeout 120
   CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test -p opencode-rk-sessions
   -p opencode-rk-tools -p opencode-rk-server` with serial for known-flaky
   targets; (iv) untracked modules added only with passing gates; (v)
   controller syncs FEATURES.md/accounting + flips per §3. `*_lane`/twin
   deletion only after test migration to canonical paths.
