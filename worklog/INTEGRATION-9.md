# INTEGRATION-9 — RED-validity matrix + serial mandates + wiring + flip requests (v9)

Rev: `248f519` HEAD. Date: 2026-09-16.
Bounds: READ-ONLY except this file. No ralph.json/controller/product edits. No fmt run. No commit.
Supersedes INTEGRATION-8 where disk disagrees (marked CORR).
Inputs: `worklog/INTEGRATION-8.md`, `worklog/GUARD-TRIAGE-9.md`,
`worklog/TURN-STREAM-GATE.md`, `worklog/RED-VALIDITY-INT.md`,
`worklog/EXT002-T05-DETERMINISM.md`, `worklog/CLEANUP-TRIAGE.md`,
`worklog/RED-VALIDITY-WEB-PROV.md`, `RED-VALIDITY-EXT1/EXT2/AUTO/OPS/TOOL/ACPSDK.md`,
`worklog/WEB-007-012-ACCEPT.md`, `worklog/WSX-SDK-HEAD-GATE.md`,
on-disk lib.rs diffs, `ralph.json` (read-only).

Guard (GUARD-TRIAGE-9, no re-run this lane): validate_repository exit 1,
122 errors, sets byte-identical vs TRIAGE-8; validate_plan exit 1, 133 =
122 + 11 unknown prefixes (ACP/HEAD/RUN/SDK/SYNC/WSX); `git diff --check`
clean (re-verified this lane, exit 0).
`ralph.json`: 258 stories, 176 accepted / 44 in-progress / 38 not-started
(re-verified this lane via python count; unchanged vs v8). NOT edited.

## 1. Stub / ignore scan (this lane)

- Strict grep: `grep -rn --include='*.rs' -e 'todo!' -e 'unimplemented!' -e '#\[ignore\]' crates/`
  → **0 hits**. Log: `/tmp/opencode/uP-stub.log` (0 lines). Classify: CLEAN.
- Loose `ignore` matches remain only known-benign (`eq_ignore_ascii_case`,
  doc "ignore leading/trailing whitespace" in `diff_tool.rs`, etc. per v8 §1).
  Zero real stubs. No action.

## 2. Fmt + status (this lane)

- `cargo fmt --check 2>&1 | grep -c 'Diff in'` → **43** (unchanged vs v6/v7/v8).
- `git status --porcelain` (bare `git` authority; rtk strips leading space):
  **205 M + 174 ?? = 379 lines**.
  CORR vs INTEGRATION-8 (204 M + 159 ?? = 363) = **+1 tracked, +15 untracked**.
  CORR vs GUARD-TRIAGE-9 (205 M + 161 ?? = 366) = +0 tracked, +13 untracked drift.
  CORR vs CLEANUP-TRIAGE (204 M + 159 ?? = 363) = same drift source (worklogs + 1 product file).
- No `cargo fmt` executed (write-shaped op banned this lane).

## 3. Wiring checklist (on-disk truth, re-verified this lane)

| Crate | File | State | Diff vs HEAD | Owner worklog | Gate evidence |
|---|---|---|---|---|---|
| server | `crates/server/src/lib.rs` | WIRED-uncommitted | +20/-0 pure additive (unchanged vs v8) | SERVER-WIRING-FINAL (45/45) | `cargo check` 0; 7 suites 48 passed |
| sessions | `crates/sessions/src/lib.rs` | WIRED-uncommitted | +3/-0 (unchanged) | SESSIONS-WIRING-FINAL | focused 20; full 288 exit 0 |
| foundation | `crates/foundation/src/lib.rs` | WIRED-uncommitted | +5/-2 (unchanged) | FOUNDATION-WIRING-FINAL | check 0; repo_ref 5 + repo_cache_store 5 |
| cli | `crates/cli/src/main.rs` | n/a, no edit needed | 0 | CLI-WIRING | `--tests` 23 passed |
| providers | `crates/providers/src/lib.rs` | n/a, no edit (deliberate) | 0 | PROVIDERS-WIRING | `cargo check` exit 0 |
| security | `crates/security/src/lib.rs` | UNPLANNED-uncommitted | 1-line alpha swap (unchanged vs v8) | none | none — attribute or revert |
| storage | `crates/storage/src/lib.rs` | UNPLANNED-uncommitted | fmt-only reflow (unchanged vs v8) | none | none — attribute or revert |
| tools | `crates/tools/src/lib.rs` | UNPLANNED-uncommitted | 1-line alpha swap (unchanged vs v8) | none | none — attribute or revert |

- 6 lib.rs files differ (same set as v8 §3). 3 NEW-vs-v7 diffs still
  reorder/fmt-only, still unattributed: single-writer unverified.
  Integrator must attribute or revert before commit.
- Providers 9 unwired left to integrator on acceptance (YAGNI, no consumer).
- No `crates/*/tests/zz_*` in tree (probe absent, unchanged vs v8).

## 4. RED-validity matrix (all slices; "probe" = recorded stub→fail→restore→GREEN)

In-repo receipt lives in `worklog/`; per-test logs in `/tmp/opencode` (ephemeral).
No RED receipts live in `crates/` (by design — frozen tests untouched).

| Slice | RED receipt | Location | Verdict |
|---|---|---|---|
| WEB-013/014/015/016/017 + PROV-015/016 | behavior-stub per suite (full-fail 4; partial WEB-014 2/5, WEB-016 4/5, WEB-017 4/5 — valid suite-bite, negative-path survival correct) + 5/5 GREEN, sha pre==post | `worklog/RED-VALIDITY-WEB-PROV.md` + `/tmp/opencode/s3-*-red/green.log`, `bak-<id>.rs` | **RED-VALID (strong, behavior)** |
| EXT-009/010/011/012 | behavior-stub per suite (3× full-fail; EXT-010 4/5, T04 negative-path survival) + 5/5 GREEN, cmp identical | `worklog/RED-VALIDITY-EXT2.md` + `/tmp/opencode/rC-*-red/green.log` | **RED-VALID (strong, behavior)** |
| EXT-001/002/004/005/006/008 | behavior-stub per file (1–2 fail each: T03 boundary arms) + 5/5 GREEN, sha pre==post | `worklog/RED-VALIDITY-EXT1.md` + `/tmp/opencode/rB-*-red.log` | **RED-VALID (strong, behavior)** |
| AUTO-004/006 | behavior-stub per file (4/1 each: owner-cancel / milestone-marker arms) + 5/5 GREEN, agents-full GREEN | `worklog/RED-VALIDITY-AUTO.md` + `/tmp/opencode/rA-*.log` | **RED-VALID (strong, behavior)** |
| AUTO-005 | declarative tool: PASS exit 0 + 3 mutated fixtures exit 2 (mutated-frozen / self-report / wrong-rev) | `tools/check_tdd_pipeline.py`, `/tmp/opencode/auto005/report-*.json` | **RED-VALID (declarative, no Rust RED per instructions)** |
| ACP-001/002, WSX-001/002, SDK-001/002, HEAD-001/002, RUN-001 | behavior-stub per file (partial-fail suite-bite; happy-path arms fail, negative paths survive) + full GREEN restore, cmp identical | `worklog/RED-VALIDITY-ACPSDK.md` + `/tmp/opencode/rH-*-red/green.log` | **RED-VALID (strong, behavior). SUPERSEDES v8 §4 "GREEN-only, no RED" row for these 9 — that row is now STALE.** |
| TOOL-016..020 + UI-019 + SYNC-001/002 | behavior-stub per file (partial-fail suite-bite; SYNC-001 full-fail 0/5) + 5/5 GREEN, sha pre==post | `worklog/RED-VALIDITY-TOOL.md` + `/tmp/opencode/rG-*-red/green.log` | **RED-VALID (strong, behavior). SUPERSEDES v8 §4 "RED gap" row for these 8 — now proven.** |
| OPS-001..009 | whole-file `__RedStub__` stub → rc=101 compile-RED (E0432/E0433), restore sha-identical + 5/5 GREEN each | `worklog/RED-VALIDITY-OPS.md` + `/tmp/opencode/rE-*-red.log` | **WEAK-RED (wiring-only). Proves test→impl linkage on CURRENT dirty content, NOT behavior bite, NOT pristine-commit validity. Frozen tests pre-modified by another lane. Verdict: re-do behavior-stub RED per file once tree quiesces; do NOT accept as behavior proof.** |
| INT-001/002/003/005/006/007/009/010 | retry NOT run: tree still dirty (src fmt-hunks + all frozen tests reformatted — re-verified this lane: `int_registry_lane/methods/refresh/location_ctx/mcp_transport/share_descriptor` src diffs are fmt-only reflows; `crates/providers/tests/int_*.rs` carry uncommitted rewraps). Prior lane-claimed RED logs exist but unwitnessed + frozen tests since modified. | `worklog/RED-VALIDITY-INT.md` (BLOCKED) + prior `/tmp/opencode/int00*_red.log` | **BLOCKED — NO VALID RED. Retry serial temp-stub-restore per file ONLY once owned paths clean; logs `/tmp/opencode/rD-<id>-red.log`; byte-identical restore; GREEN 5/5 each. Frozen tests/lib.rs/ralph.json never touched.** |
| WEB-007..012 | none — GREEN-only 5/5 per suite, write-paths deliberately disabled | `worklog/WEB-007-012-ACCEPT.md` | GREEN-only HOLD (unchanged) |
| session_turn_stream_api | none — 3x serial GREEN (harness-race diagnosis, not behavior RED) | `worklog/TURN-STREAM-GATE.md` + `/tmp/opencode/stream_serial{1,2,3}/serial.log` | serial-mandate only (§5) |
| EXT002-T05 | flaky-by-construction: impl spawns zero threads; T05 measures libtest's own transient workers via `/proc/self/task`; serial 30/30 GREEN, parallel flake spectra reproduced | `worklog/EXT002-T05-DETERMINISM.md` + `/tmp/opencode/rI-t05.log` | NO-FIX. Serial mandate only (§5). Deterministic repair needs frozen-test waiver (owner authority). |
| REL-001..003 | none — validator-only slices (T02/T03/T05 exit-2 pattern per INTEGRATION-4 §3) | lane worklogs only | RED gap, verifier re-run pending |
| WEB-006 | none this lane | prior worklogs | unchanged |

## 5. Serial-test mandates (binding on verifier/CI)

```
# stream binary: env race (TURN-STREAM-GATE §2)
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-server --test session_turn_stream_api -- --test-threads=1
# ext_builtins_lane binary: T05 thread-count race (EXT002-T05-DETERMINISM §2-3)
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-tools --test ext_builtins_lane -- --test-threads=1
```

- Stream root cause: process-global env race (`OPENAI_BASE_URL`/`KEY` via
  unlocked `EnvGuard`, 2 tests same binary). 3x serial GREEN logged. No server
  bug; `TURN_PERMITS` cap does not fix test parallelism.
- T05 root cause: `/proc/self/task` counts libtest's own sibling workers;
  `t0 == t1` samples a racy global. Impl deterministic (zero spawns, 2000-sample
  isolated probe t0=t1=2). Parallel default stays flaky (7/40, 9/20 @threads=2
  samples); serial 30/30 GREEN. Treat parallel `left/right in 1..5` line-175
  failures as harness noise, not regressions.
- Do NOT run either binary `--test-threads=N>1`; do NOT parallelize with heavy
  jobs (8 GiB budget, one validation at a time). Do NOT edit frozen tests.
- `env_lock` proposal TURN-STREAM-GATE §6, T05 options EXT002-T05 §5 (pin
  serial / rewrite assertion to impl-owned state): NOT applied —
  verifier/test-owner authority.

## 6. ralph.json flip request table (EVIDENCE ONLY — do NOT edit)

| Story | ralph now | Request | Evidence path |
|---|---|---|---|
| REL-001..003 | in-progress | controller review → accepted? validator-only, lane GREEN, verifier re-run pending | `worklog/REL-001/002/003.md`, INTEGRATION-5 §3 |
| ACP-001/002 | not-started | review → in-progress? impl GREEN, WIRED-uncommitted §3, RED-VALID §4 | `worklog/ACP-001.md`, `ACP-002.md`, RED-VALIDITY-ACPSDK |
| SDK-001/002 | not-started | review → in-progress? lane GREEN, WIRED-uncommitted §3, RED-VALID §4 | `worklog/SDK-001.md`, `SDK-002.md`, RED-VALIDITY-ACPSDK |
| WSX-001 | not-started | review → in-progress? file on disk, WIRED-uncommitted §3, RED-VALID §4 | `worklog/WSX-001.md`, RED-VALIDITY-ACPSDK |
| WSX-002 | not-started | review → in-progress? 5/5 GREEN, WIRED-uncommitted §3, RED-VALID §4 | `worklog/WSX-002.md`, RED-VALIDITY-ACPSDK |
| HEAD-001/002 | not-started | review → in-progress? tested, NO wiring needed §3, RED-VALID §4 | `worklog/HEAD-001.md`, `HEAD-002.md`, RED-VALIDITY-ACPSDK |
| SYNC-001/002 | not-started | review → in-progress? files on disk, WIRED-uncommitted §3, RED-VALID §4 | `sync_log.rs`, `part_events.rs`, RED-VALIDITY-TOOL |
| RUN-001 | not-started | review → in-progress? WIRED-uncommitted §3, RED-VALID §4, gate pending | `worklog/UI019-TOOL016-020-SYNC-RUN.md`, RED-VALIDITY-ACPSDK |
| UI-019 / TOOL-016..020 | not-started | review → in-progress? pre-existing product, new tests, RED-VALID §4 | same worklog + RED-VALIDITY-TOOL |
| EXT-009..012 | not-started | review → in-progress? RED-VALID §4 | RED-VALIDITY-EXT2 |
| EXT-001/002/004/005/006/008 | not-started | review → in-progress? RED-VALID §4 | RED-VALIDITY-EXT1 |
| AUTO-004/006 | ? | review → ? RED-VALID §4 (check ralph status before flip) | RED-VALIDITY-AUTO |
| PROV-017..024 | not-started | HOLD → READY for gate: one-line fix in worktree (uncommitted) | `claude_oauth.rs:462` raw `{LOOPBACK_REDIRECT_URI}` (per INTEGRATION-5 §2; re-verify before flip) |
| WEB-007..012 | not-started | HOLD (models GREEN, write-paths disabled) | `worklog/WEB-007-012-ACCEPT.md` |
| WEB-013..017 + PROV-015/016 | not-started | HOLD → RED-validated (§4): controller decides flip to in-progress | `worklog/RED-VALIDITY-WEB-PROV.md` |
| OPS-001..009 | ? | HOLD — weak-RED only; behavior RED re-do required before flip | `worklog/RED-VALIDITY-OPS.md`, this file §4 |
| INT-001..010 | ? | HOLD — BLOCKED, no valid RED; retry per §4 before flip | `worklog/RED-VALIDITY-INT.md`, this file §4 |
| Unknown prefixes SYNC/RUN/ACP/WSX/SDK/HEAD | n/a | controller: allowlist-or-map + reconcile accounting | plan validator 11 extra (GUARD-TRIAGE-9 §1) |
| FEATURES stale-accepted set | accepted | FEATURES.md sync only (controller authority) | guard log 122 lines (69 stale-mirror + 53 ledger/gap) |

## 7. Ranked blockers top 5 (owner lanes)

1. Wired-but-uncommitted + ungated + unattributed lib.rs drift (owner: wiring lanes + verifier + integrator).
   Server +20, sessions +3, foundation +3 mods in worktree, zero committed.
   security/storage/tools lib.rs reorder/fmt-only diffs still no owning lane —
   attribute or revert before the integration commit.
   Action: serial lane-gate GREEN per suite (§5 mandates for
   `session_turn_stream_api` + `ext_builtins_lane`), then ONE integration commit.
2. Guard FAIL backlog exhaustion (owner: controller). 122 errors exit 1,
   byte-identical sets vs TRIAGE-8. 69 FEATURES sync + 53 ledger/gap; no lane edits ralph.json.
3. INT BLOCKED + OPS weak-RED + frozen-test dirt (owner: INT/OPS lanes + verifier).
   INT retry gated on quiesced owned paths (src fmt-hunks + reformatted frozen
   tests re-verified dirty this lane). OPS needs behavior-stub re-do; compile-RED
   is wiring proof only. Neither slice flippable now.
4. Unknown-prefix accounting (owner: controller). Plan validator 133 errors,
   11 extra. Allowlist-or-map + reconcile; PROV-015 now RED-validated but still
   unclassified in ledger (plan gap persists).
   (Task-card paths referencing non-existent `ext/share/delegation/autonomy/ops/integration` crates — lane-variant exists; see AUDIT-NONACCEPTED §crate-missing rollup.)
5. PROV-017 fix gate + fmt/untracked drift (owner: providers lane + verifier + integrator).
   One-line worktree fix per INTEGRATION-5 §2; re-run `prov_017` (+018..024) first.
   `cargo fmt --check` 43 diffs (unchanged); status 366→379, all drift untracked
   (+13 since TRIAGE-9) plus +1 tracked (`plugin_lifecycle.rs` per TRIAGE-9 §2).

Merge order: (i) attribute/revert §3 unattributed lib.rs drift; (ii) quiesce
INT/OPS owned paths → INT retry + OPS behavior re-do (§4); (iii) lane gates
GREEN per wired suite (serial mandates §5); (iv) refresh stale status files;
(v) ONE integration commit (tracked fixes + lib.rs wirings, fmt at commit);
(vi) untracked modules only with passing gates; (vii) controller syncs
FEATURES.md/accounting + flips per §6.
