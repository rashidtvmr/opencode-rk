# INTEGRATION-4 — wiring checklist status + flip requests (v4)

Rev: `248f519` HEAD, worktree ~314 entries (234 modified + 80 untracked).
Date: 2026-09-16. Owner: integration lane.
Bounds: read-only except this file. No `ralph.json`/controller/lib.rs/product edits.
Supersedes INTEGRATION-3 where disk disagrees (marked CORR).
Inputs: `worklog/INTEGRATION-3.md`, `worklog/GUARD-TRIAGE-4.md`,
`worklog/ACP-WIRING.md`, `worklog/TURN-STREAM-GATE.md`, on-disk
`crates/{server,sessions,foundation,cli}/src/lib.rs|main.rs`,
`git status --porcelain`, `ralph.json` (read-only), guard tail.
Requested SERVER-WIRING.md / SESSIONS-WIRING.md / CLI-WIRING.md: MISSING
(glob finds only `ACP-WIRING.md`). Noted pending below; this file covers all three.

Guard (this lane, `timeout 110 python3 tools/validate_repository.py`):
exit=1, backlog-exhaustion class, tail stale `FEATURES.md: UI-015..018
not-started vs accepted`. Same class as INTEGRATION-3; no new guard drift claimed.
`ralph.json`: 258 stories, 176 accepted / 44 in-progress / 38 not-started (unchanged counts).

## 1. Wiring checklist status (on-disk truth, integrator assembles)

### 1a. sessions — +3 WIRED in worktree (CORR vs INTEGRATION-3 §1c)
- `crates/sessions/src/lib.rs:47-49`: `pub mod part_events;`,
  `pub mod runner;`, `pub mod tui_info_panel;` present as TRACKED modification
  (+72/-30 diff vs HEAD, rest = fmt reflow + `pub use` reorder).
- Files still UNTRACKED (`part_events.rs`, `runner.rs`, `tui_info_panel.rs`);
  wiring lines tracked-modified. Gate each target GREEN before integration commit.
- Do NOT wire: `share_*_lane`, `chat_nav_lane` (`#[path]`-only). Canonical
  `share_*` set unchanged; `share_store.rs` still has NO `share_store` mod line.

### 1b. server — +0 wired, 7 files present-unwired (unchanged vs INTEGRATION-3 §1b)
- `crates/server/src/lib.rs:3-27`: 25 mods only, diff vs HEAD EMPTY (unmodified).
  Zero `acp_*`/`sync_log`/`remote_sync`/`sdk_client`/`sdk_spawns`/
  `workspace_proxy` mods.
- Present-unwired untracked: `acp_bridge.rs`, `acp_files.rs`, `remote_sync.rs`,
  `sdk_client.rs`, `sdk_spawns.rs`, `sync_log.rs`, `workspace_proxy.rs`
  (+ matching `server/tests/*.rs`).
- CORR vs `worklog/ACP-WIRING.md` claim: that worklog reports
  `pub mod acp_bridge;` + `pub mod acp_files;` wired with `cargo check` GREEN,
  but on-disk `lib.rs` contains neither line. Treat ACP-WIRING as
  STALE/SUPERSEDED — wiring NOT on disk. Integrator re-adds (gated):
  `acp_bridge`, `acp_files`, `sync_log`, `remote_sync`, `sdk_client`,
  `sdk_spawns`, `workspace_proxy`.
- Voice posture unchanged: `web_capabilities()` reports voice unavailable;
  `voice_capture.rs` exists on disk, unwired — read-only posture holds.

### 1c. cli — +0 wired, binary is single-file `main.rs` (CORR vs INTEGRATION-3 §5.2)
- `crates/cli/src/`: `main.rs` only (NO `lib.rs`); `run_headless.rs` +
  `session_export.rs` UNTRACKED, zero refs in `main.rs` (grep empty).
- CORR vs INTEGRATION-3: `run_headless.rs` (HEAD-001) and
  `workspace_proxy.rs` (WSX-001) NOW EXIST on disk (were MISSING).
  `worklog/HEAD-001.md` + `worklog/WSX-001.md` exist. Missing-pair closer to
  done: needs CLI `mod` wiring lane (declare mods in `main.rs`) + gates.
- Integrator adds (gated): `mod run_headless;` + `mod session_export;` in
  `main.rs`, or controller-approved layout change. No other CLI edits.

### 1d. foundation — +0 wired (unchanged vs INTEGRATION-3 §1a)
- `lib.rs:3-29`: reorder-only diff (`ops_lock`/`ops_runtime` alpha).
  NO `repo_cache_store`/`repo_ref` mods; both files TRACKED-MODIFIED, unwired.
  Wire only with passing gates; else leave unwired.

### 1e. agents/tools — +0 wired (unchanged)
- `agents/src/lib.rs:5-7`: `executor, message, turn_state` only.
  `tools/src/lib.rs`: no `ext_*_lane` mods (dedupe keep-both holds).

## 2. ralph.json flip request table (EVIDENCE ONLY — do NOT edit ralph.json)

| Story | ralph now | Request | Evidence path |
|---|---|---|---|
| REL-001..003 | in-progress | controller review → accepted? validator-only slices, lane GREEN, verifier re-run pending | `worklog/REL-001/002/003.md`, INTEGRATION-3 §4 |
| ACP-001/002 | not-started | review → in-progress? impl GREEN per SDK-HEAD-STATUS (27 passed/4 suites), UNWIRED (§1b) | `worklog/ACP-001.md`, `ACP-002.md`, `ACP-WIRING.md` STALE |
| SDK-001/002 | not-started | review → in-progress? 12/12 + spawns GREEN, UNWIRED | `worklog/SDK-001.md`, `SDK-002.md` |
| WSX-001 | not-started | review → in-progress? NEW `workspace_proxy.rs` + test on disk, UNWIRED | `worklog/WSX-001.md` |
| WSX-002 | not-started | review → in-progress? 5/5 GREEN, UNWIRED | `worklog/WSX-002.md` |
| HEAD-001 | not-started | review → in-progress? NEW `run_headless.rs` + test on disk, UNWIRED | `worklog/HEAD-001.md` |
| HEAD-002 | not-started | review → in-progress? `session_export.rs` + test on disk, UNWIRED | `worklog/HEAD-002.md` |
| SYNC-001/002 | not-started | review → in-progress? files on disk, UNWIRED + needs validator prefix registration | `sync_log.rs`, `part_events.rs` |
| RUN-001 | not-started | review → in-progress? `runner.rs` WIRED §1a, gate pending | `worklog/UI019-TOOL016-020-SYNC-RUN.md` |
| UI-019 / TOOL-016..020 | not-started | review → in-progress? pre-existing product, new tests | same worklog |
| PROV-017..024 | not-started | HOLD → READY for gate: one-line fix NOW APPLIED in worktree (§5.1) | `prov_017..024` tests untracked |
| WEB-007..012 | not-started | HOLD (models GREEN, write-paths disabled) | `worklog/WEB-007-012-ACCEPT.md` |
| WEB-013..017 + PROV-015/016 | not-started | HOLD (GREEN-only, no valid RED) | `worklog/WEB-013-PROV-016.md`, `WEB-016-DECISION.md` |
| Unknown prefixes SYNC/RUN/ACP/WSX/SDK/HEAD | n/a | controller: allowlist-or-map + reconcile accounting | plan validator 11 extra |
| FEATURES stale-accepted set (UI-015..018 tail etc.) | accepted | FEATURES.md sync only (controller authority) | guard log |

## 3. REL checklist
Per INTEGRATION-3 §4 (lane GREEN, verifier re-run pending): REL-001 T01 exit 0,
T02/T03/T05 exit 2, T04 exit 0 zero-accepted-bytes; REL-002 T01 exit 0,
T02–T05 exit 2; REL-003 T01 exit 0, T02–T05 exit 2, canary absent.
No re-run in this lane (read-only). No change claimed.

## 4. Serial-test mandate (TURN-STREAM-GATE, binding on verifier/CI)
```
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-server --test session_turn_stream_api -- --test-threads=1
```
- Root cause: process-global env race (`OPENAI_BASE_URL`/`KEY` via unlocked
  `EnvGuard`, two tests, same binary). 3x serial GREEN logged. Harness race,
  no server bug; `TURN_PERMITS` cap 2 does not fix test parallelism.
- Do NOT run with `--test-threads=N>1`; do NOT parallelize with heavy jobs
  (8 GiB budget, one validation at a time). Do NOT "fix" by editing frozen tests.
- Proposed `env_lock` harness diff (mirrors `session_turn_activity_api.rs`)
  NOT applied — verifier/test-owner authority.

## 5. Top blockers ranked (owner lanes)
1. Server wiring gap + stale ACP-WIRING claim (owner: server-wiring lane).
   7 files present-unwired, `lib.rs` untouched; ACP-WIRING.md asserts wiring
   absent from disk. Action: gated wiring lane re-adds 7 mods, re-runs
   `acp_bridge`/`acp_files`/`sdk_client`/`sdk_spawns`/`remote_sync`/
   `sync_log`/`workspace_proxy` suites, refreshes or supersedes ACP-WIRING.md.
2. CLI wiring gap (owner: cli lane). `run_headless.rs` + `session_export.rs`
   untracked, `main.rs` single-file binary has no mod lines. Action: wiring
   lane declares mods in `main.rs`, gates `run_headless` + `session_export`
   suites. Unblocks HEAD-001/002 flips.
3. PROV-017 fix applied, gate pending (owner: providers lane + verifier).
   CORR vs INTEGRATION-3 blocker #1: worktree `claude_oauth.rs:462` NOW emits
   raw `{LOOPBACK_REDIRECT_URI}` (one-line diff vs HEAD confirmed). Action:
   lane-gate re-run `prov_017` (+ 018..024), then flip requests unblock.
4. Sessions wired-but-ungated (owner: sessions lane + verifier). 3 mods wired
   in worktree (§1a) on top of untracked files. Action: gate `part_events`/
   `runner`/`tui_info_panel` (+ `mcp_status_panel`) before integration commit.
5. Foundation `repo_cache_store`/`repo_ref` modified-unwired (owner:
   foundation lane). Action: gate, then wire — or leave unwired explicitly.
6. Controller sync backlog (owner: controller/verifier). FEATURES.md stale set,
   unknown-prefix allowlist, all §2 flips. No lane edits `ralph.json`.

Merge order: (i) lane gates GREEN per lane for all unwired modules (serial
mandate §4 for `session_turn_stream_api`); (ii) refresh stale status files
(ACP-WIRING, SDK-HEAD-STATUS "4/8 missing" now 0/8 missing); (iii) ONE
integration commit for tracked fixes + lib.rs/main.rs wirings, bounded
`timeout 120 CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test` per crate;
(iv) untracked modules added only with passing gates; (v) controller syncs
FEATURES.md/accounting + flips per §2. `*_lane`/twin deletion only after test
migration to canonical paths.
