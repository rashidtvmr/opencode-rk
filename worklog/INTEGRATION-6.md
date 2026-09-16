# INTEGRATION-6 — wiring checklist + serial mandate + flip requests (v6)

Rev: `248f519` HEAD (same as INTEGRATION-5 / GUARD-TRIAGE-6). Date: 2026-09-16.
Bounds: read-only except this file. No ralph.json/controller/product edits.
Supersedes INTEGRATION-5 where disk disagrees (marked CORR).
Inputs: `worklog/INTEGRATION-5.md`, `worklog/GUARD-TRIAGE-6.md`,
`worklog/TURN-STREAM-GATE.md`, `worklog/SERVER-WIRING.md`,
`worklog/SERVER-WIRING-FINAL.md`, `worklog/SESSIONS-WIRING.md`,
`worklog/SESSIONS-WIRING-FINAL.md`, `worklog/CLI-WIRING.md`,
`worklog/FOUNDATION-WIRING-FINAL.md`, `worklog/PROVIDERS-WIRING.md`,
on-disk lib.rs diffs, `ralph.json` (read-only).

Guard (GUARD-TRIAGE-6, no re-run this lane): validate_repository exit 1
(backlog exhaustion, 122 error lines); validate_plan exit 1 (133 errors);
plan-repo delta 11 = unknown prefixes (SYNC/ACP/WSX/SDK/HEAD/RUN).
`ralph.json`: 258 stories, 176 accepted / 44 in-progress / 38 not-started
(unchanged vs INTEGRATION-5, re-verified this lane via python count).
ralph.json NOT edited (out of scope).

## 1. Stub / ignore scan (this lane)

- Bare grep (no rtk filter, avoids pass-through doubt):
  `grep -rn --include='*.rs' -e 'todo!' -e 'unimplemented!' -e '#\[ignore\]' crates/`
  → 0 hits. Log: `/tmp/opencode/w11-stub.log` (0 lines).
- Case-insensitive `todo|unimplemented|ignore` → 60 benign hits only:
  `eq_ignore_ascii_case`, `INSERT OR IGNORE`, "unknown keys ignored" comments,
  `ignore_whitespace/case` fields. Zero real stubs. No action.

## 2. Fmt + status (this lane)

- `cargo fmt --check 2>&1 | grep -c 'Diff in'` → **43**.
- `git status --porcelain`: **203 M + 127 ?? = ~329-330 lines**
  (CORR vs GUARD-TRIAGE-6 321 = 203 M + 118 ??: modified same, +9 untracked drift).
- No `cargo fmt` executed (write-shaped op banned this lane).

## 3. Wiring checklist (on-disk truth)

| Crate | File | State | Diff vs HEAD | Owner worklog | Gate evidence |
|---|---|---|---|---|---|
| server | `crates/server/src/lib.rs` | WIRED-uncommitted | +20/-0 pure additive (`acp_bridge`, `acp_files`, `chat_composer`, `control_plane_errors`, `control_plane_exposure`, `control_plane_inputs`, `event_stream`, `protocol_api`, `remote_sync`, `sdk_client`, `sdk_spawns`, `sync_log`, `transcript_lane`, `turn_parts`, `voice_capture`, `web_artifact`, `web_attachments`, `web_entry_probe`, `web_tool_chooser`, `workspace_proxy`) | SERVER-WIRING (append) → SERVER-WIRING-FINAL (alpha-sorted, 45/45 wired, 0 missing) | FINAL: `cargo check` exit 0; 7 target suites 48 passed |
| sessions | `crates/sessions/src/lib.rs` | WIRED-uncommitted | +3/-0 (`part_events`, `runner`, `tui_info_panel` after `ui_013`) | SESSIONS-WIRING → SESSIONS-WIRING-FINAL (no new edit, verified) | FINAL re-verify wave: focused 20 passed; full 288 passed exit 0 |
| foundation | `crates/foundation/src/lib.rs` | WIRED-uncommitted | +5/-2 (3 adds `ops_parser_lane`, `repo_cache_store`, `repo_ref` + `ops_lock`/`ops_runtime` alpha reorder) | FOUNDATION-WIRING-FINAL | `cargo check` 0 errors; `repo_ref` 5 + `repo_cache_store` 5 passed |
| cli | `crates/cli/src/main.rs` | n/a, no edit needed | 0 (binary crate, zero `mod` lines) | CLI-WIRING | `--tests` 23 passed; `#[path]` includes, no wiring required |
| providers | `crates/providers/src/lib.rs` | n/a, no edit (deliberate) | 0 (57 wired; 9 unwired `connection`, `int_connect`, `int_handler_lane`, `int_location_lane`, `int_mcp_lane`, `int_methods`, `int_refresh`, `int_registry_lane`, `int_share_sync_lane` consumed via `#[path]`, zero crate-path consumers) | PROVIDERS-WIRING | `cargo check` exit 0 |

- Single-writer holds: each lib.rs touched by exactly one wiring lane.
  Unchanged vs INTEGRATION-5 except server FINAL alpha-sort (still additive).
- CLI verdict stands (INTEGRATION-5 §1c): binary + `#[path]` → no mod wiring.
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
- `env_lock` proposal location: **TURN-STREAM-GATE §6** — suggested diff for
  `crates/server/tests/session_turn_stream_api.rs` (add `Mutex`/`OnceLock`
  import + `fn env_lock()` + per-test guard, mirrors
  `session_turn_activity_api.rs:21-24,179-181,253-255`). NOT applied —
  verifier/test-owner authority. Per-binary lock only; full fix needs
  distinct keys / injection / workspace lock (noted in gate §6).

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
| Unknown prefixes SYNC/RUN/ACP/WSX/SDK/HEAD | n/a | controller: allowlist-or-map + reconcile accounting | plan validator 11 extra (GUARD-TRIAGE-6) |
| FEATURES stale-accepted set | accepted | FEATURES.md sync only (controller authority) | guard log 122 lines |

## 6. Ranked blockers top 5 (owner lanes)

1. Wired-but-uncommitted + ungated (owner: wiring lanes + verifier).
   Server +20, sessions +3, foundation +3 mods in worktree, zero committed.
   Action: serial lane-gate GREEN per suite (§4 mandate for
   `session_turn_stream_api`), then ONE integration commit. CLI/providers need none.
2. Guard FAIL backlog exhaustion (owner: controller). 122 errors exit 1,
   delta 0 vs TRIAGE-6. FEATURES.md sync only — no lane edits ralph.json.
3. Unknown-prefix accounting (owner: controller). Plan validator 133 errors,
   11 extra. Allowlist-or-map + reconcile.
4. PROV-017 fix gate pending (owner: providers lane + verifier).
   One-line worktree fix per INTEGRATION-5 §2; re-run `prov_017` (+018..024) first.
5. Fmt drift 43 + untracked drift +9 (owner: owning lanes + integrator).
   `cargo fmt --check` 43 diffs; status 321→~330. Action: owning lanes gate,
   integrator formats at commit time only (no lane-run `cargo fmt` per budget rules).

Merge order: (i) lane gates GREEN per wired suite (serial mandate §4);
(ii) refresh stale status files; (iii) ONE integration commit (tracked fixes +
lib.rs wirings, fmt at commit); (iv) untracked modules only with passing gates;
(v) controller syncs FEATURES.md/accounting + flips per §5.
