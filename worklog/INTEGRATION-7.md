# INTEGRATION-7 — wiring checklist + serial mandate + flip requests (v7)

Rev: `248f519` HEAD (same as INTEGRATION-5/6, GUARD-TRIAGE-7). Date: 2026-09-16.
Bounds: read-only except this file. No ralph.json/controller/product edits.
Supersedes INTEGRATION-6 where disk disagrees (marked CORR).
Inputs: `worklog/INTEGRATION-5.md`, `worklog/INTEGRATION-6.md`,
`worklog/GUARD-TRIAGE-7.md`, `worklog/TURN-STREAM-GATE.md`,
`worklog/SERVER-WIRING-FINAL.md`, `worklog/SESSIONS-WIRING-FINAL.md`,
`worklog/FOUNDATION-WIRING-FINAL.md`, on-disk lib.rs diffs, `ralph.json` (read-only).

Guard (GUARD-TRIAGE-7, no re-run this lane): validate_repository exit 1
(backlog exhaustion, 122 error lines, delta 0); validate_plan exit 1 (133 errors
= 122 + 11 unknown prefixes SYNC/RUN/ACP/WSX/SDK/HEAD); `git diff --check` clean.
`ralph.json`: 258 stories, 176 accepted / 44 in-progress / 38 not-started
(re-verified this lane via python count on `userStories`; unchanged vs v5/v6).
ralph.json NOT edited.

## 1. Stub / ignore scan (this lane)

- Strict grep: `grep -rn --include='*.rs' -e 'todo!' -e 'unimplemented!' -e '#\[ignore\]' crates/`
  → **0 hits**. Log: `/tmp/opencode/s14-stub.log` (0 lines).
- Benign case-insensitive tally (79 lines): `ignored` 28, `eq_ignore_ascii_case` 27,
  `ignore_case` 8, `ignore_whitespace` 5, `ignores` 5, `INSERT OR IGNORE` 3,
  `ignore` 3, `ignore_ws` 2, `ignoring` 1. Zero real stubs. No action.

## 2. Fmt + status (this lane)

- `cargo fmt --check 2>&1 | grep -c 'Diff in'` → **43** (unchanged vs INTEGRATION-6).
- `git status --porcelain`: **204 M + 131 ?? = 335 entries** (`wc -l` 334, off-by-one blank-line artifact).
  CORR vs INTEGRATION-6 (203 M + 127 ?? ~329-330) = **+1 tracked, +4 untracked drift**.
  CORR vs GUARD-TRIAGE-7 (203 M + 127 ?? = 330) same drift.
- No `cargo fmt` executed (write-shaped op banned this lane).

## 3. Wiring checklist (on-disk truth)

| Crate | File | State | Diff vs HEAD | Owner worklog | Gate evidence |
|---|---|---|---|---|---|
| server | `crates/server/src/lib.rs` | WIRED-uncommitted | +20/-0 pure additive (`acp_bridge`, `acp_files`, `chat_composer`, `control_plane_errors`, `control_plane_exposure`, `control_plane_inputs`, `event_stream`, `protocol_api`, `remote_sync`, `sdk_client`, `sdk_spawns`, `sync_log`, `transcript_lane`, `turn_parts`, `voice_capture`, `web_artifact`, `web_attachments`, `web_entry_probe`, `web_tool_chooser`, `workspace_proxy`) | SERVER-WIRING → SERVER-WIRING-FINAL (45/45 wired, 0 missing) | FINAL: `cargo check` exit 0; 7 target suites 48 passed |
| sessions | `crates/sessions/src/lib.rs` | WIRED-uncommitted | +3/-0 (`part_events`, `runner`, `tui_info_panel` after `ui_013`) | SESSIONS-WIRING → SESSIONS-WIRING-FINAL (verified, no new edit) | focused 20 passed; full 288 passed exit 0 |
| foundation | `crates/foundation/src/lib.rs` | WIRED-uncommitted | +5/-2 (3 adds `ops_parser_lane`, `repo_cache_store`, `repo_ref` + `ops_lock`/`ops_runtime` alpha reorder) | FOUNDATION-WIRING-FINAL | `cargo check` 0 errors; `repo_ref` 5 + `repo_cache_store` 5 passed |
| cli | `crates/cli/src/main.rs` | n/a, no edit needed | 0 (binary crate, zero `mod` lines) | CLI-WIRING | `--tests` 23 passed; `#[path]` includes, no wiring required |
| providers | `crates/providers/src/lib.rs` | n/a, no edit (deliberate) | 0 (57 wired; 9 unwired `#[path]`-consumed, zero crate-path consumers) | PROVIDERS-WIRING | `cargo check` exit 0 |

- Single-writer holds: each lib.rs touched by exactly one wiring lane; `git diff --stat`
  confirms only the 3 files above changed (server +20, sessions +3, foundation +5/-2;
  cli/providers 0). Unchanged vs INTEGRATION-6.
- Providers 9 unwired left to integrator on lane acceptance (YAGNI, no consumer).

## 4. Serial-test mandate (TURN-STREAM-GATE, binding on verifier/CI)

```
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-server --test session_turn_stream_api -- --test-threads=1
```

- Root cause: process-global env race (`OPENAI_BASE_URL`/`KEY` via unlocked
  `EnvGuard`, 2 tests same binary). 3x serial GREEN logged. Harness race,
  no server bug; `TURN_PERMITS` cap 2 does not fix test parallelism.
- Do NOT run `--test-threads=N>1`; do NOT parallelize with heavy jobs
  (8 GiB budget, one validation at a time). Do NOT edit frozen tests.
- `env_lock` proposal location: TURN-STREAM-GATE §6 (mirrors
  `session_turn_activity_api.rs:21-24,179-181,253-255`). NOT applied —
  verifier/test-owner authority. Per-binary lock only; full fix needs
  distinct keys / injection / workspace lock.

## 5. ralph.json flip request table (EVIDENCE ONLY — do NOT edit)

| Story | ralph now | Request | Evidence path |
|---|---|---|---|
| REL-001..003 | in-progress | controller review → accepted? validator-only slices, lane GREEN, verifier re-run pending | `worklog/REL-001/002/003.md`, INTEGRATION-5 §3 |
| ACP-001/002 | not-started | review → in-progress? impl GREEN, WIRED-uncommitted §3 | `worklog/ACP-001.md`, `ACP-002.md`, SERVER-WIRING-FINAL |
| SDK-001/002 | not-started | review → in-progress? lane GREEN, WIRED-uncommitted §3 | `worklog/SDK-001.md`, `SDK-002.md` |
| WSX-001 | not-started | review → in-progress? file on disk, WIRED-uncommitted §3 | `worklog/WSX-001.md` |
| WSX-002 | not-started | review → in-progress? 5/5 GREEN, WIRED-uncommitted §3 | `worklog/WSX-002.md` |
| HEAD-001/002 | not-started | review → in-progress? `#[path]`-tested, NO wiring needed §3 | `worklog/HEAD-001.md`, `HEAD-002.md`, CLI-WIRING |
| SYNC-001/002 | not-started | review → in-progress? files on disk, WIRED-uncommitted §3 | `sync_log.rs`, `part_events.rs` |
| RUN-001 | not-started | review → in-progress? WIRED-uncommitted §3, gate pending | `worklog/UI019-TOOL016-020-SYNC-RUN.md` |
| UI-019 / TOOL-016..020 | not-started | review → in-progress? pre-existing product, new tests | same worklog |
| PROV-017..024 | not-started | HOLD → READY for gate: one-line fix in worktree (uncommitted) | `claude_oauth.rs:462` raw `{LOOPBACK_REDIRECT_URI}` (per INTEGRATION-5 §2; re-verify before flip) |
| WEB-007..012 | not-started | HOLD (models GREEN, write-paths disabled) | `worklog/WEB-007-012-ACCEPT.md` |
| WEB-013..017 + PROV-015/016 | not-started | HOLD (GREEN-only, no valid RED) | `worklog/WEB-013-PROV-016.md`, `WEB-016-DECISION.md` |
| Unknown prefixes SYNC/RUN/ACP/WSX/SDK/HEAD | n/a | controller: allowlist-or-map + reconcile accounting | plan validator 11 extra (GUARD-TRIAGE-7 §1) |
| FEATURES stale-accepted set | accepted | FEATURES.md sync only (controller authority) | guard log 122 lines (69 stale-mirror + 53 ledger/gap) |

## 6. Ranked blockers top 5 (owner lanes)

1. Wired-but-uncommitted + ungated (owner: wiring lanes + verifier).
   Server +20, sessions +3, foundation +3 mods in worktree, zero committed.
   Action: serial lane-gate GREEN per suite (§4 mandate for
   `session_turn_stream_api`), then ONE integration commit. CLI/providers need none.
2. Guard FAIL backlog exhaustion (owner: controller). 122 errors exit 1,
   delta 0 vs TRIAGE-7. 69 FEATURES sync + 53 ledger/gap; no lane edits ralph.json.
3. Unknown-prefix accounting (owner: controller). Plan validator 133 errors,
   11 extra. Allowlist-or-map + reconcile; PROV-015 unclassified gap.
4. PROV-017 fix gate pending (owner: providers lane + verifier).
   One-line worktree fix per INTEGRATION-5 §2; re-run `prov_017` (+018..024) first.
5. Fmt drift 43 + untracked drift +4 (owner: owning lanes + integrator).
   `cargo fmt --check` 43 diffs (unchanged); status 330→335.
   Action: owning lanes gate, integrator formats at commit time only.

Merge order: (i) lane gates GREEN per wired suite (serial mandate §4);
(ii) refresh stale status files; (iii) ONE integration commit (tracked fixes +
lib.rs wirings, fmt at commit); (iv) untracked modules only with passing gates;
(v) controller syncs FEATURES.md/accounting + flips per §5.
