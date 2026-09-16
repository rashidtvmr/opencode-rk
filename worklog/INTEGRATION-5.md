# INTEGRATION-5 — single-writer wiring results + flip requests (v5)

Rev: `248f519` HEAD, worktree 321 entries (203 modified + 118 untracked).
Date: 2026-09-16. Owner: integration lane.
Bounds: read-only except this file. No `ralph.json`/controller/product edits.
Supersedes INTEGRATION-4 where disk disagrees (marked CORR).
Inputs: `worklog/INTEGRATION-4.md`, `worklog/GUARD-TRIAGE-5.md`,
`worklog/SERVER-WIRING.md`, `worklog/SESSIONS-WIRING.md`,
`worklog/CLI-WIRING.md`, `worklog/FOUNDATION-WIRING-FINAL.md`,
`worklog/ACP-WIRING.md` (STALE, superseded), `worklog/TURN-STREAM-GATE.md`,
on-disk `crates/{server,sessions,foundation}/src/lib.rs`,
`crates/cli/src/main.rs`, guard tail, `ralph.json` (read-only).
Requested AUTO-GATED-COVERAGE.md: ABSENT (glob empty). Noted pending; no claim made.

Guard (this lane, `timeout 110 python3 tools/validate_repository.py`):
exit=1, backlog-exhaustion class, 122 `  - ` lines (unchanged vs TRIAGE-5).
`python3 tools/validate_plan.py`: 133 errors (11 extra = unknown prefixes).
`ralph.json`: 258 stories, 176 accepted / 44 in-progress / 38 not-started (unchanged).

## 1. Single-writer wiring results (on-disk truth, one worklog per crate)

### 1a. server — SERVER-WIRING owns lib.rs, +20 mods present-uncommitted
- `crates/server/src/lib.rs:3-47`: 45 mods. Diff vs HEAD +20/-0 (pure additive).
  Matches SERVER-WIRING claim exactly: `acp_bridge`, `acp_files`,
  `chat_composer`, `control_plane_errors`, `control_plane_exposure`,
  `control_plane_inputs`, `event_stream`, `protocol_api`, `remote_sync`,
  `sdk_client`, `sdk_spawns`, `sync_log`, `transcript_lane`, `turn_parts`,
  `voice_capture`, `web_artifact`, `web_attachments`, `web_entry_probe`,
  `web_tool_chooser`, `workspace_proxy`.
- CORR vs INTEGRATION-4 §1b (which reported +0 wired, lib.rs untouched):
  SERVER-WIRING lane has since applied the wiring in worktree. Still
  UNCOMMITTED — gate each suite GREEN before integration commit.
- CORR vs ACP-WIRING.md: that file's 2-mod claim (`acp_bridge`+`acp_files`)
  is SUBSUMED by SERVER-WIRING's 20-mod set. Treat ACP-WIRING as STALE.
- No competing writer: `git diff` for this file shows only `pub mod` lines.

### 1b. sessions — SESSIONS-WIRING owns lib.rs, +3 mods present-uncommitted
- `crates/sessions/src/lib.rs:47-49`: `part_events`, `runner`,
  `tui_info_panel` present. Diff vs HEAD +3/-0 (pure additive).
  Matches SESSIONS-WIRING claim exactly (append after `ui_013`, no reorder).
- Unchanged vs INTEGRATION-4 §1a except still uncommitted. Gate `part_events`/
  `runner`/`tui_info_panel` (+ `mcp_status_panel` regression) before commit.
- Do NOT wire: `share_*_lane`, `chat_nav_lane` (`#[path]`-only). Canonical
  `share_*` set unchanged.
- Lane evidence: `cargo check -p opencode-rk-sessions` exit 0;
  `runner`/`part_events`/`tui_info_panel`/`mcp_status_panel` 5 passed each.

### 1c. cli — CLI-WIRING verdict: n/a, no edit needed (CORR vs INTEGRATION-4 §1c)
- `crates/cli/src/main.rs`: binary crate, zero `mod` lines (grep empty).
  Tests include via `#[path]` (`tests/run_headless.rs:6-7`,
  `tests/session_export.rs:6-7`). No `mod` wiring required.
- Lane evidence: `cargo test -p opencode-rk-cli --tests` 23 passed, 0 failed.
- INTEGRATION-4 §1c instruction to add `mod run_headless;`+`mod session_export;`
  is SUPERSEDED — wiring not needed for binary crates with `#[path]` tests.

### 1d. foundation — FOUNDATION-WIRING-FINAL owns lib.rs, +3 mods present-uncommitted
- `crates/foundation/src/lib.rs:16,26-27`: `ops_parser_lane`,
  `repo_cache_store`, `repo_ref` present. Diff vs HEAD +5/-2 (3 adds +
  pre-existing `ops_lock`/`ops_runtime` alpha reorder).
- Matches FOUNDATION-WIRING-FINAL claim exactly. Lane evidence:
  `cargo check` 0 errors; `repo_ref` 5 passed; `repo_cache_store` 5 passed.
- CORR vs INTEGRATION-4 §1d (+0 wired): wiring now applied in worktree,
  uncommitted. Gate before commit or leave unwired explicitly.
- NOTE: requested `SERVER/SESSIONS/FOUNDATION-WIRING-FINAL.md` path not found
  as single file; per-crate files present instead (`SERVER-WIRING.md`,
  `SESSIONS-WIRING.md`, `CLI-WIRING.md`, `FOUNDATION-WIRING-FINAL.md`).
  Coverage equivalent; single-writer holds per crate.

### 1e. single-writer check
- Each lib.rs touched by exactly one wiring worklog; diffs are additive
  `pub mod` lines only (foundation carries incidental reorder).
  No cross-lane edit conflict observed in these four files.

## 2. ralph.json flip request table (EVIDENCE ONLY — do NOT edit ralph.json)

| Story | ralph now | Request | Evidence path |
|---|---|---|---|
| REL-001..003 | in-progress | controller review → accepted? validator-only slices, lane GREEN, verifier re-run pending | `worklog/REL-001/002/003.md`, INTEGRATION-4 §3 |
| ACP-001/002 | not-started | review → in-progress? impl GREEN per lane, WIRED-uncommitted §1a | `worklog/ACP-001.md`, `ACP-002.md`, `SERVER-WIRING.md` |
| SDK-001/002 | not-started | review → in-progress? lane GREEN, WIRED-uncommitted §1a | `worklog/SDK-001.md`, `SDK-002.md` |
| WSX-001 | not-started | review → in-progress? file on disk, WIRED-uncommitted §1a | `worklog/WSX-001.md` |
| WSX-002 | not-started | review → in-progress? 5/5 GREEN, WIRED-uncommitted §1a | `worklog/WSX-002.md` |
| HEAD-001/002 | not-started | review → in-progress? `#[path]`-tested, NO wiring needed §1c | `worklog/HEAD-001.md`, `HEAD-002.md`, `CLI-WIRING.md` |
| SYNC-001/002 | not-started | review → in-progress? files on disk, WIRED-uncommitted §1a/§1b | `sync_log.rs`, `part_events.rs` |
| RUN-001 | not-started | review → in-progress? WIRED-uncommitted §1b, gate pending | `worklog/UI019-TOOL016-020-SYNC-RUN.md` |
| UI-019 / TOOL-016..020 | not-started | review → in-progress? pre-existing product, new tests | same worklog |
| PROV-017..024 | not-started | HOLD → READY for gate: one-line fix in worktree (uncommitted) | `claude_oauth.rs:462` raw `{LOOPBACK_REDIRECT_URI}`, diff +1/-1 |
| WEB-007..012 | not-started | HOLD (models GREEN, write-paths disabled) | `worklog/WEB-007-012-ACCEPT.md` |
| WEB-013..017 + PROV-015/016 | not-started | HOLD (GREEN-only, no valid RED) | `worklog/WEB-013-PROV-016.md`, `WEB-016-DECISION.md` |
| Unknown prefixes SYNC/RUN/ACP/WSX/SDK/HEAD | n/a | controller: allowlist-or-map + reconcile accounting | plan validator 11 extra |
| FEATURES stale-accepted set | accepted | FEATURES.md sync only (controller authority) | guard log 122 lines |

## 3. REL status (no re-run in this lane, read-only)
Per INTEGRATION-4 §3 (lane GREEN, verifier re-run pending): REL-001 T01 exit 0,
T02/T03/T05 exit 2, T04 exit 0 zero-accepted-bytes; REL-002 T01 exit 0,
T02–T05 exit 2; REL-003 T01 exit 0, T02–T05 exit 2, canary absent.
No change claimed.

## 4. Serial-test mandate (TURN-STREAM-GATE, binding on verifier/CI)
```
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-server --test session_turn_stream_api -- --test-threads=1
```
- Root cause: process-global env race (`OPENAI_BASE_URL`/`KEY` via unlocked
  `EnvGuard`, two tests, same binary). 3x serial GREEN logged. Harness race,
  no server bug; `TURN_PERMITS` cap 2 does not fix test parallelism.
- Do NOT run with `--test-threads=N>1`; do NOT parallelize with heavy jobs
  (8 GiB budget, one validation at a time). Do NOT "fix" by editing frozen tests.
- Proposed `env_lock` harness diff NOT applied — verifier/test-owner authority.

## 5. Ranked blockers (owner lanes)
1. Wired-but-uncommitted + ungated (owner: wiring lanes + verifier).
   Server +20, sessions +3, foundation +3 mods in worktree, zero committed.
   Action: serial lane-gate GREEN per suite (§4 mandate for
   `session_turn_stream_api`), then ONE integration commit for tracked fixes +
   lib.rs wirings. CLI needs no commit (no edit).
2. Guard FAIL backlog exhaustion (owner: controller). 122 errors, exit 1,
   delta 0 vs TRIAGE-5. FEATURES.md sync only — no lane edits `ralph.json`.
3. Unknown-prefix accounting (owner: controller). Plan validator 133 errors,
   11 extra (SYNC/RUN/ACP/WSX/SDK/HEAD). Allowlist-or-map + reconcile.
4. PROV-017 fix applied, gate pending (owner: providers lane + verifier).
   `claude_oauth.rs` diff +1/-1 confirmed in worktree, file MODIFIED.
   Action: lane-gate re-run `prov_017` (+ 018..024), then flips unblock.
5. Stale status files (owner: owning lanes). ACP-WIRING.md superseded by
   SERVER-WIRING.md; SDK-HEAD-STATUS "missing" counts outdated;
   AUTO-GATED-COVERAGE.md absent. Refresh or mark superseded.

Merge order: (i) lane gates GREEN per lane for all wired modules (serial
mandate §4 for `session_turn_stream_api`); (ii) refresh stale status files;
(iii) ONE integration commit for tracked fixes + lib.rs wirings, bounded
`timeout 120 CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 cargo test` per crate;
(iv) untracked modules added only with passing gates; (v) controller syncs
FEATURES.md/accounting + flips per §2. `*_lane`/twin deletion only after test
migration to canonical paths.
