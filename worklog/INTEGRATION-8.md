# INTEGRATION-8 — wiring checklist + serial mandate + flip requests (v8)

Rev: `248f519` HEAD (same as INTEGRATION-5/6/7, GUARD-TRIAGE-7/8). Date: 2026-09-16.
Bounds: read-only except this file. No ralph.json/controller/product edits.
Supersedes INTEGRATION-7 where disk disagrees (marked CORR).
Inputs: `worklog/INTEGRATION-6.md`, `worklog/INTEGRATION-7.md`,
`worklog/GUARD-TRIAGE-7.md`, `worklog/GUARD-TRIAGE-8.md`,
`worklog/TURN-STREAM-GATE.md`, `worklog/RED-VALIDITY-WEB-PROV.md`,
`worklog/WSX-SDK-HEAD-GATE.md`, `worklog/WEB-007-012-ACCEPT.md`,
`worklog/WEB-016-DECISION.md`, `worklog/AUDIT-NONACCEPTED.md`,
on-disk lib.rs diffs, `ralph.json` (read-only).
CLEANUP-TRIAGE.md: ABSENT (glob no match).

Guard (GUARD-TRIAGE-8, no re-run this lane): validate_repository exit 1
(backlog exhaustion, 122 error lines, byte-identical sets vs TRIAGE-7);
validate_plan exit 1 (133 = 122 + 11 unknown prefixes ACP/HEAD/RUN/SDK/SYNC/WSX);
`git diff --check` clean (re-verified this lane, exit 0).
`ralph.json`: 258 stories, 176 accepted / 44 in-progress / 38 not-started
(re-verified this lane via python count on `userStories`; unchanged vs v5/v6/v7).
ralph.json NOT edited.

## 1. Stub / ignore scan (this lane)

- Strict grep: `grep -rn --include='*.rs' -e 'todo!' -e 'unimplemented!' -e '#\[ignore\]' crates/`
  → **0 hits**. Log: `/tmp/opencode/t6-stub.log` (0 lines).
- Non-benign filter (exclude `eq_ignore_ascii_case`/`ignore_case`/`ignore_whitespace`/
  `ignored`/`ignores`/`ignore_ws`/`ignoring`/`INSERT OR IGNORE`): only
  `crates/tools/src/diff_tool.rs` doc comments ("ignore leading/trailing
  whitespace", "ignore character case" — struct-field docs, not stubs).
  Zero real stubs. No action.

## 2. Fmt + status (this lane)

- `cargo fmt --check 2>&1 | grep -c 'Diff in'` → **43** (unchanged vs INTEGRATION-6/7).
- `git status --porcelain` (bare `git` is authority; rtk-wrapped counts disagree):
  **204 M + 159 ?? = 363 lines** (`/tmp/opencode/t6-status.log`).
  CORR vs INTEGRATION-7 (204 M + 131 ?? = 335) = **+0 tracked, +28 untracked drift**.
  CORR vs GUARD-TRIAGE-8 (204 M + 133 ?? = 337) = +26 untracked since s13 snapshot.
- No `cargo fmt` executed (write-shaped op banned this lane).

## 3. Wiring checklist (on-disk truth)

| Crate | File | State | Diff vs HEAD | Owner worklog | Gate evidence |
|---|---|---|---|---|---|
| server | `crates/server/src/lib.rs` | WIRED-uncommitted | +20/-0 pure additive (same 20 mods as v6/v7) | SERVER-WIRING → SERVER-WIRING-FINAL (45/45 wired, 0 missing) | FINAL: `cargo check` exit 0; 7 target suites 48 passed |
| sessions | `crates/sessions/src/lib.rs` | WIRED-uncommitted | +3/-0 (`part_events`, `runner`, `tui_info_panel`) | SESSIONS-WIRING → SESSIONS-WIRING-FINAL | focused 20 passed; full 288 passed exit 0 |
| foundation | `crates/foundation/src/lib.rs` | WIRED-uncommitted | +5/-2 (`ops_parser_lane`, `repo_cache_store`, `repo_ref` + alpha reorder) | FOUNDATION-WIRING-FINAL | `cargo check` 0 errors; `repo_ref` 5 + `repo_cache_store` 5 passed |
| cli | `crates/cli/src/main.rs` | n/a, no edit needed | 0 | CLI-WIRING | `--tests` 23 passed; `#[path]` includes |
| providers | `crates/providers/src/lib.rs` | n/a, no edit (deliberate) | 0 | PROVIDERS-WIRING | `cargo check` exit 0 |
| security | `crates/security/src/lib.rs` | UNPLANNED-uncommitted (NEW vs v7) | alpha reorder (`clarity_guard`/`cmd_patterns` swap, ~1-line) | none | none this lane |
| storage | `crates/storage/src/lib.rs` | UNPLANNED-uncommitted (NEW vs v7) | fmt-only reflow (import wrap + 3 line-breaks, no mod add/remove) | none | none this lane |
| tools | `crates/tools/src/lib.rs` | UNPLANNED-uncommitted (NEW vs v7) | 1-line alpha swap (`tool_allow`/`tool_quota`/`tool_sandbox` order) | none | none this lane |

- INTEGRATION-7 §3 claimed "only the 3 files above changed"; that no longer holds —
  6 lib.rs files now differ (CORR). The 3 NEW diffs are reorder/fmt-only (zero mod
  adds/removes), but single-writer is now unverified: no owning lane claims
  security/storage/tools lib.rs. Integrator must attribute or revert before commit.
- Providers 9 unwired left to integrator on lane acceptance (YAGNI, no consumer).
- `crates/sessions/tests/zz_probe_debug_redact.rs` (TRIAGE-8 §2 new path) is GONE
  from disk this lane (no `crates/*/tests/zz_*` match) — probe removed or renamed
  by owning lane; no validator input touched either way.

## 4. RED-validity matrix (which slices have probes)

"Probe" = recorded RED (stub→fail→restore→GREEN) evidence. In-repo = receipt
lives in `worklog/` (hashes + logs); all per-test logs sit in `/tmp/opencode`
(ephemeral). No RED receipts live in `crates/` (by design — frozen tests untouched).

| Slice | RED receipt | Location | Verdict |
|---|---|---|---|
| WEB-013/014/015/016/017 + PROV-015/016 | behavior-stub RED per suite (full-fail 4 suites; partial-fail WEB-014 2/5, WEB-016 4/5, WEB-017 4/5 — valid suite-bite, negative-path tests correctly survive) + 5/5 GREEN restore, sha pre==post | `worklog/RED-VALIDITY-WEB-PROV.md` + `/tmp/opencode/s3-*-red.log`, `s3-*-green.log`, `bak-<id>.rs`, `red_run.py` | RED-VALID (closes AUDIT gap for these 7) |
| WEB-007..012 | none — GREEN-only 5/5 per suite, write-paths deliberately disabled | `worklog/WEB-007-012-ACCEPT.md` | GREEN-only HOLD (unchanged) |
| ACP-001/002, WSX-001/002, SDK-001/002, HEAD-001/002 | none — "No RED/GREEN cycle needed", GREEN spot 24 passed (grand 59 across 8 suites) | `worklog/WSX-SDK-HEAD-GATE.md` + `/tmp/opencode/w16-spot.log` | GREEN-only, no RED history |
| session_turn_stream_api | none — 3x serial GREEN (harness-race diagnosis, not behavior RED) | `worklog/TURN-STREAM-GATE.md` + `/tmp/opencode/stream_serial{1,2,3}/serial.log` | serial-mandate only (§5) |
| REL-001..003 | none — validator-only slices (T02/T03/T05 exit-2 pattern per INTEGRATION-4 §3) | lane worklogs only | RED gap, verifier re-run pending |
| AUTO-004/006, SYNC/RUN/TOOL-016..020/UI-019 | none found — GREEN-only `#[path]`/lane-variant | bundle worklogs (`UI019-TOOL016-020-SYNC-RUN.md`) | RED gap |
| WEB-006 | none this lane | prior worklogs | unchanged |

## 5. Serial-test mandate (TURN-STREAM-GATE, binding on verifier/CI)

```
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-server --test session_turn_stream_api -- --test-threads=1
```

- Root cause: process-global env race (`OPENAI_BASE_URL`/`KEY` via unlocked
  `EnvGuard`, 2 tests same binary). 3x serial GREEN logged. Harness race,
  no server bug; `TURN_PERMITS` cap 2 does not fix test parallelism.
- Do NOT run `--test-threads=N>1`; do NOT parallelize with heavy jobs
  (8 GiB budget, one validation at a time). Do NOT edit frozen tests.
- `env_lock` proposal: TURN-STREAM-GATE §6 (mirrors
  `session_turn_activity_api.rs:21-24,179-181,253-255`). NOT applied —
  verifier/test-owner authority. Per-binary lock only; full fix needs
  distinct keys / injection / workspace lock.

## 6. ralph.json flip request table (EVIDENCE ONLY — do NOT edit)

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
| WEB-013..017 + PROV-015/016 | not-started | HOLD → RED-validated (this lane §4): controller decides flip to in-progress | `worklog/RED-VALIDITY-WEB-PROV.md` (UPGRADED vs v7 HOLD: RED now proven) |
| Unknown prefixes SYNC/RUN/ACP/WSX/SDK/HEAD | n/a | controller: allowlist-or-map + reconcile accounting | plan validator 11 extra (GUARD-TRIAGE-8 §1) |
| FEATURES stale-accepted set | accepted | FEATURES.md sync only (controller authority) | guard log 122 lines (69 stale-mirror + 53 ledger/gap) |

## 7. Ranked blockers top 5 (owner lanes)

1. Wired-but-uncommitted + ungated + unattributed lib.rs drift (owner: wiring lanes + verifier + integrator).
   Server +20, sessions +3, foundation +3 mods in worktree, zero committed.
   NEW: security/storage/tools lib.rs reorder/fmt-only diffs with no owning lane —
   attribute or revert before the integration commit.
   Action: serial lane-gate GREEN per suite (§5 mandate for
   `session_turn_stream_api`), then ONE integration commit. CLI/providers need none.
2. Guard FAIL backlog exhaustion (owner: controller). 122 errors exit 1,
   byte-identical sets vs TRIAGE-7. 69 FEATURES sync + 53 ledger/gap; no lane edits ralph.json.
3. Unknown-prefix accounting (owner: controller). Plan validator 133 errors,
   11 extra. Allowlist-or-map + reconcile; PROV-015 unclassified gap.
   (22 task-card paths reference non-existent `ext/share/delegation/autonomy/ops/integration` crates — lane-variant exists; see AUDIT-NONACCEPTED §crate-missing rollup.)
4. PROV-017 fix gate pending (owner: providers lane + verifier).
   One-line worktree fix per INTEGRATION-5 §2; re-run `prov_017` (+018..024) first.
5. Fmt drift 43 + untracked drift +28 (owner: owning lanes + integrator).
   `cargo fmt --check` 43 diffs (unchanged); status 335→363, all untracked.
   Action: owning lanes gate, integrator formats at commit time only.

Merge order: (i) attribute/revert §3 NEW lib.rs drift; (ii) lane gates GREEN per
wired suite (serial mandate §5); (iii) refresh stale status files; (iv) ONE
integration commit (tracked fixes + lib.rs wirings, fmt at commit);
(v) untracked modules only with passing gates; (vi) controller syncs
FEATURES.md/accounting + flips per §6.
