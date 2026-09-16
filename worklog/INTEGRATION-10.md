# INTEGRATION-10 — RED-validity matrix + guard-port/codex-bounds + wiring + flip requests (v10)

Rev: `248f519` HEAD. Date: 2026-09-16.
Bounds: READ-ONLY except this file. No ralph.json/controller/product edits. No fmt run. No commit.
Supersedes INTEGRATION-9 where disk disagrees (marked CORR).
Inputs: `worklog/INTEGRATION-9.md`, `worklog/GUARD-TRIAGE-10.md`,
`worklog/TURN-STREAM-GATE.md`, `worklog/RED-VALIDITY-INT.md` (+ uA retry §),
`worklog/RED-VALIDITY-OPS.md`, `worklog/RED-VALIDITY-OPS2.md`,
`worklog/RED-VALIDITY-WEB-PROV.md`, `worklog/RED-VALIDITY-WEB013-015.md`,
`worklog/RED-VALIDITY-PROV015-016.md`, `RED-VALIDITY-EXT1/EXT2/AUTO/TOOL/ACPSDK.md`,
`worklog/WEB-007-012-ACCEPT.md`, `worklog/EXT002-T05-DETERMINISM.md`,
`worklog/EXT-TWINS-DISPOSITION.md`, `worklog/SHARE-TWINS-DISPOSITION.md`,
`worklog/AUTO-TOKIO-BROKER.md`, on-disk lib.rs diffs, `ralph.json` (read-only).
RED-VALIDITY-INT-FINAL: ABSENT (glob no match). RED-VALIDITY-OPS-FINAL: ABSENT (glob no match).
This-wave FINAL verdicts therefore taken from OPS2 (behavior RED) + INT uA-retry section.

Guard (GUARD-TRIAGE-10, no re-run this lane): validate_repository exit 1,
122 errors; validate_plan exit 1, 133 = 122 + 11 unknown prefixes
(ACP/HEAD/RUN/SDK/SYNC/WSX); `git diff --check` clean (re-verified this lane, exit 0).
`ralph.json`: 258 stories, 176 accepted / 44 in-progress / 38 not-started
(re-verified this lane via python count; unchanged vs v8/v9). NOT edited.

## 1. Stub / ignore scan (this lane)

- Strict grep: `grep -rn --include='*.rs' -e 'todo!' -e 'unimplemented!' -e '#\[ignore\]' crates/`
  → **0 hits**. Log: `/tmp/opencode/v16-stub.log` (0 lines). Classify: CLEAN.
- Loose `ignore` matches remain only known-benign (`eq_ignore_ascii_case`,
  doc "ignore leading/trailing whitespace" in `diff_tool.rs`, etc. per v8/v9 §1).
  Zero real stubs. No action.

## 2. Fmt + status (this lane)

- `cargo fmt --check 2>&1 | grep -c 'Diff in'` → **48** (CORR vs v6/v7/v8/v9 = 43; **+5**).
- `git status --porcelain` (bare `git` authority):
  **204 M + 188 ?? = 392 lines**.
  CORR vs INTEGRATION-9 (205 M + 174 ?? = 379) = **-1 tracked, +14 untracked**.
  CORR vs GUARD-TRIAGE-10 (205 M + 176 ?? = 381) = -1 tracked, +12 untracked drift.
  Drift source: worklogs + twin/guard-port product hunks (§3); no controller files.
- No `cargo fmt` executed (write-shaped op banned this lane).

## 3. Wiring checklist (on-disk truth, re-verified this lane)

| Crate | File | State | Diff vs HEAD | Owner worklog | Gate evidence |
|---|---|---|---|---|---|
| server | `crates/server/src/lib.rs` | WIRED-uncommitted | +20/-0 pure additive (unchanged vs v8/v9) | SERVER-WIRING-FINAL (45/45) | `cargo check` 0; 7 suites 48 passed |
| sessions | `crates/sessions/src/lib.rs` | WIRED-uncommitted | +3/-0 (unchanged) | SESSIONS-WIRING-FINAL | focused 20; full 288 exit 0 |
| foundation | `crates/foundation/src/lib.rs` | WIRED-uncommitted | +5/-2 (unchanged) | FOUNDATION-WIRING-FINAL | check 0; repo_ref 5 + repo_cache_store 5 |
| cli | `crates/cli/src/main.rs` | n/a, no edit needed | 0 | CLI-WIRING | `--tests` 23 passed |
| providers | `crates/providers/src/lib.rs` | n/a, no edit (deliberate) | 0 | PROVIDERS-WIRING | `cargo check` exit 0 |
| security | `crates/security/src/lib.rs` | UNPLANNED-uncommitted | 1-line alpha swap (unchanged) | none | none — attribute or revert |
| tools | `crates/tools/src/lib.rs` | UNPLANNED-uncommitted | 1-line alpha swap (unchanged) | none | none — attribute or revert |
| storage | `crates/storage/src/lib.rs` | UNPLANNED-uncommitted, GREW | +18/-9 fmt-only reflows (CORR vs v9 "fmt-only reflow": more rewraps, still zero mod add/remove) | none | none — attribute or revert |
| tools | `crates/tools/src/plugin_transform.rs` | GUARD-PORT-landed, uncommitted (NEW vs v9) | +9/-2: unknown-scope guard `entries.iter().any` + invariant docs (§3a) | EXT-TWINS-DISPOSITION pair-1 | needs canonical 5/5 re-run post-port |

- 6 lib.rs files differ (same set as v8/v9 §3). Storage hunk grew (+18/-9 vs prior
  reflow) but still zero mod add/remove; still unattributed: single-writer unverified.
  Integrator must attribute or revert before commit.
- Providers 9 unwired left to integrator on acceptance (YAGNI, no consumer).
- No `crates/*/tests/zz_*` in tree (probe absent, unchanged vs v8/v9).

### 3a. Guard-port status (EXT pair-1: twin → canonical)

- Disposition order (EXT-TWINS-DISPOSITION §pair-1): port twin's 1-line
  unknown-scope guard + invariant docs into `plugin_transform` first, then drop twin.
- Status: **PORT LANDED in worktree, uncommitted.** `git diff HEAD --
  crates/tools/src/plugin_transform.rs` = +9/-2:
  `set_scope_disabled` now guards `!disabled.contains(&scope) &&
  entries.iter().any(|t| t.scope == scope)`; `TransformLog` + method carry
  bounded-`disabled` invariant docs. Matches twin `ext_replay_lane` semantics.
- Remaining before drop: re-run canonical `plugin_transform` 5/5 GREEN on ported
  bytes (serial mandate pattern §6); then integrator drops `ext_replay_lane`
  src+test in same commit (`#[path]` coupling: twin test breaks iff src deleted).
- Pairs 2–8: NO port applied this lane (drops pending integrator; pair 5/6/7 need
  1-line `#![forbid(unsafe_code)]` port at drop time per disposition).

### 3b. Codex-bounds status (PROV-016 B3 validators: OPEN, no fix applied)

- `git diff HEAD -- crates/providers/src/codex_oauth.rs
  crates/providers/tests/codex_oauth.rs` → **empty (clean)**. No bounds fix, no
  test addition landed. Frozen tests untouched.
- Open gaps per RED-VALIDITY-PROV015-016 §PROV-016: `validate_device_code`
  (empty + `>MAX_DEVICE_CODE_BYTES`), `validate_consent_url` length cap
  (`>MAX_CONSENT_URL_BYTES`), zero-expiry rejection (`complete_login` + `refresh`)
  are dead enforcement from the suite's view — B3/B3alt/B3exp stubs stay 5/5 GREEN.
- Additive proposal (T06 device-code bounds / T07 URL length cap / T08 zero-expiry)
  NOT applied — verifier/test-owner authority. Slice stays flippable for lifecycle
  only; boundary-hardening flip needs new frozen tests.

## 4. RED-validity matrix (all slices; "probe" = recorded stub→fail→restore→GREEN)

In-repo receipt lives in `worklog/`; per-test logs in `/tmp/opencode` (ephemeral).
No RED receipts live in `crates/` (by design — frozen tests untouched).

| Slice | RED receipt | Location | Verdict |
|---|---|---|---|
| WEB-014/016/017 (suite-bite) + WEB-013/PROV-015/016 (entry-point) | behavior-stub per suite (full-fail 4; partial WEB-014 2/5, WEB-016 4/5, WEB-017 4/5 — valid suite-bite, negative-path survival correct) + 5/5 GREEN, sha pre==post | `worklog/RED-VALIDITY-WEB-PROV.md` + `/tmp/opencode/s3-*-red/green.log`, `bak-<id>.rs` | **RED-VALID (strong, entry-point). QUALIFIED by re-verification rows below — entry-point verdict stands, boundary precision does NOT extend.** |
| WEB-013 capabilities boundary (re-verification) | flag-flip FAIL (`available` false→true bites `:49`), missing-key FAIL (`Null != false`); reason-text-only stub PASSES (no bite) | `worklog/RED-VALIDITY-WEB013-015.md` + `/tmp/opencode/uC-WEB013-attempt{1,2,3}.log` | **WEAK-RED (partial pinning). Unavailable-boundary = YES pinned; reason-string precision = NO; T01-full/T03/T04/T05 = NOT pinned (no code/tests by design, story not accepted). Do NOT weaken suite; additive gap probes proposed (NEW-FILE, not applied).** |
| WEB-015 registry boundary (re-verification) | 3/3 stub strategies bite (scope-flag FAIL, phantom-workspace FAIL, project_root-leak FAIL) | same file + `/tmp/opencode/uC-WEB015-attempt{1,2,3}.log` | **WEAK-RED (partial pinning). Registry-metadata YES pinned 3/3; full T01 (membership/memory), T02 leak, T03–T05 = NOT pinned (no authority/tests by design).** |
| PROV-015 (re-verification) | A1 forced-Err 0/5, A2 wrong-state 4/1, A3 overflow-cap 4/1 — all bite | `worklog/RED-VALIDITY-PROV015-016.md` + `/tmp/opencode/uD-PROV015-016-a{1,2,3}.log` | **RED-VALID (strong). Constructor guards + projection + provenance + redaction + determinism all pin. GREEN after restore 5/5.** |
| PROV-016 (re-verification) | B1 forced-LoggedOut 0/5, B2 wrong-state 4/1 bite; B3 nocap device-code 5/5 PASS, B3alt nocap URL-length 5/5 PASS, B3exp zero-expiry 5/5 PASS — NO BITE | same file + `/tmp/opencode/uD-PROV015-016-b{1,2,3,3alt,3exp}.log` | **WEAK-RED (lifecycle pins, boundary validators DO NOT). Codex-bounds OPEN (§3b). Prior WEB-PROV verdict stands for entry-point only.** |
| EXT-009/010/011/012 | behavior-stub per suite (3× full-fail; EXT-010 4/5, T04 negative-path survival) + 5/5 GREEN, cmp identical | `worklog/RED-VALIDITY-EXT2.md` + `/tmp/opencode/rC-*-red/green.log` | **RED-VALID (strong, behavior)** |
| EXT-001/002/004/005/006/008 | behavior-stub per file (1–2 fail each: T03 boundary arms) + 5/5 GREEN, sha pre==post | `worklog/RED-VALIDITY-EXT1.md` + `/tmp/opencode/rB-*-red.log` | **RED-VALID (strong, behavior)** |
| AUTO-004/006 | behavior-stub per file (4/1 each: owner-cancel / milestone-marker arms) + 5/5 GREEN, agents-full GREEN | `worklog/RED-VALIDITY-AUTO.md` + `/tmp/opencode/rA-*.log` | **RED-VALID (strong, behavior). MECHANISM CAVEAT: AUTO-TOKIO-BROKER finds tokio REQUIRED by card but NOT MET (sync bool-flag cancel passes T04 vacuously); broker mirror SUFFICES (no card obligation). Needs card amendment OR integration lane + controller dep approval.** |
| AUTO-005 | declarative tool: PASS exit 0 + 3 mutated fixtures exit 2 (mutated-frozen / self-report / wrong-rev) | `tools/check_tdd_pipeline.py`, `/tmp/opencode/auto005/report-*.json` | **RED-VALID (declarative, no Rust RED per instructions)** |
| ACP-001/002, WSX-001/002, SDK-001/002, HEAD-001/002, RUN-001 | behavior-stub per file (partial-fail suite-bite; happy-path arms fail, negative paths survive) + full GREEN restore, cmp identical | `worklog/RED-VALIDITY-ACPSDK.md` + `/tmp/opencode/rH-*-red/green.log` | **RED-VALID (strong, behavior)** |
| TOOL-016..020 + UI-019 + SYNC-001/002 | behavior-stub per file (partial-fail suite-bite; SYNC-001 full-fail 0/5) + 5/5 GREEN, sha pre==post | `worklog/RED-VALIDITY-TOOL.md` + `/tmp/opencode/rG-*-red/green.log` | **RED-VALID (strong, behavior)** |
| OPS-001..009 | OPS2 behavior RED: signatures kept, one wrong behavior per file, suites COMPILE + fail on assertions (zero E0432/E0433) + 5/5 GREEN each, sha pre==post | `worklog/RED-VALIDITY-OPS2.md` (SUPERSEDES `RED-VALIDITY-OPS.md` weak compile-RED) + `/tmp/opencode/uB-*-red.log` | **RED-VALID (strong, behavior — UPGRADED vs v9 weak-RED). Caveat retained: tree dirty on entry (frozen tests pre-modified by another lane), so wiring proven on CURRENT content, NOT pristine-commit validity. Re-do on quiesced tree before acceptance flip.** |
| INT-001/002/003/005/006/007/009/010 | uA retry ON CURRENT bytes: per-file temp behavior-stub → compile+fail (INT-001 2/3, INT-002 0/5, INT-003 3/2, INT-005 3/2, INT-006 4/1, INT-007 4/1, INT-009 2/3, INT-010 0/5) → byte-identical restore → 5/5 GREEN each | `worklog/RED-VALIDITY-INT.md` §Retry 2026-09-16 (uA) + `/tmp/opencode/uA-INT*-red.log`, `uA-INT-green1/2.log`, `intretry/pre-stub-sha256.txt` | **RED-VALID (strong, behavior — UPGRADED vs v9 BLOCKED). Same dirt caveat as OPS: 6/8 src files carried fmt-only hunks at stub time; RED logged against current bytes, restore verified IDENTICAL. Frozen tests/lib.rs/ralph.json never touched. FINAL-file ABSENT so this section is the receipt.** |
| WEB-007..012 | none — GREEN-only 5/5 per suite, write-paths deliberately disabled | `worklog/WEB-007-012-ACCEPT.md` | GREEN-only HOLD (unchanged) |
| session_turn_stream_api | none — 3x serial GREEN (harness-race diagnosis, not behavior RED) | `worklog/TURN-STREAM-GATE.md` + `/tmp/opencode/stream_serial{1,2,3}/serial.log` | serial-mandate only (§5) |
| EXT002-T05 | flaky-by-construction: impl spawns zero threads; T05 measures libtest's own transient workers via `/proc/self/task`; serial 30/30 GREEN, parallel flake spectra reproduced | `worklog/EXT002-T05-DETERMINISM.md` + `/tmp/opencode/rI-t05.log` | NO-FIX. Serial mandate only (§5). Deterministic repair needs frozen-test waiver (owner authority). |
| EXT twins (8 pairs) | KEEP-canonical = `plugin_*`; pair-1 guard PORTED (§3a); pairs 2–8 drop-pending | `worklog/EXT-TWINS-DISPOSITION.md` + `/tmp/opencode/uM-ext.log` (35/35 7-suite) | disposition ACCEPTED-lane; execution = integrator |
| SHARE twins (5 pairs) | keep-both everywhere; rename-never-shim-never-delete; policy_lane/policy2_lane collision noted | `worklog/SHARE-TWINS-DISPOSITION.md` | survey only; redaction writer owns merge/queue |
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
| EXT twins pairs 2–8 | n/a (dedupe) | integrator: execute drops per §3a AFTER pair-1 GREEN re-run | EXT-TWINS-DISPOSITION |
| AUTO-004/006 | ? | review → ? RED-VALID §4 BUT tokio-mechanism caveat (card amendment or integration lane first) | RED-VALIDITY-AUTO + AUTO-TOKIO-BROKER |
| PROV-017..024 | not-started | HOLD → READY for gate: one-line fix in worktree (uncommitted) | `claude_oauth.rs:462` raw `{LOOPBACK_REDIRECT_URI}` (per INTEGRATION-5 §2; re-verify before flip) |
| WEB-007..012 | not-started | HOLD (models GREEN, write-paths disabled) | `worklog/WEB-007-012-ACCEPT.md` |
| WEB-013/015 | not-started | HOLD → PARTIAL-pin only (§4 weak-RED): boundary pinned, full contract NOT pinned; controller decides flip to in-progress | `worklog/RED-VALIDITY-WEB013-015.md` |
| WEB-014/016/017 + PROV-015 | not-started | HOLD → RED-validated (§4: WEB-014/016/017 suite-bite, PROV-015 strong): controller decides flip to in-progress | `worklog/RED-VALIDITY-WEB-PROV.md` + `RED-VALIDITY-PROV015-016.md` |
| PROV-016 | ? | HOLD-lifecycle-only — boundary validators non-pinning (§3b/§4); T06–T08 tests needed before full flip | `worklog/RED-VALIDITY-PROV015-016.md`, this file §3b |
| OPS-001..009 | ? | HOLD → behavior-RED proven (§4 OPS2) BUT dirty-tree caveat; re-run on quiesced tree before accept-flip | `worklog/RED-VALIDITY-OPS2.md`, this file §4 |
| INT-001..010 | ? | HOLD → behavior-RED proven (§4 uA retry) BUT dirty-tree caveat; re-run on quiesced tree before accept-flip | `worklog/RED-VALIDITY-INT.md`, this file §4 |
| Unknown prefixes SYNC/RUN/ACP/WSX/SDK/HEAD | n/a | controller: allowlist-or-map + reconcile accounting | plan validator 11 extra (GUARD-TRIAGE-10 §1) |
| FEATURES stale-accepted set | accepted | FEATURES.md sync only (controller authority) | guard log 122 lines (69 stale-mirror + 53 ledger/gap) |

## 7. Ranked blockers top 5 (owner lanes)

1. Wired-but-uncommitted + ungated + unattributed lib.rs drift + LANDED guard-port
   un-gated (owner: wiring lanes + verifier + integrator).
   Server +20, sessions +3, foundation +3 mods in worktree, zero committed.
   security/storage/tools lib.rs reorder/fmt-only diffs still no owning lane
   (storage GREW to +18/-9) — attribute or revert before the integration commit.
   `plugin_transform` guard-port (+9/-2) needs canonical 5/5 re-run, then twin drop.
   Action: serial lane-gate GREEN per suite (§5 mandates for
   `session_turn_stream_api` + `ext_builtins_lane`), then ONE integration commit.
2. Guard FAIL backlog exhaustion (owner: controller). 122 errors exit 1,
   byte-identical sets vs TRIAGE-9/10. 69 FEATURES sync + 53 ledger/gap; no lane edits ralph.json.
3. Dirty-tree RED caveat on INT + OPS + frozen-test reformats (owner: INT/OPS lanes + verifier).
   Behavior RED now proven for both (OPS2 + INT uA retry — v9 BLOCKED/weak rows UPGRADED),
   but logged against CURRENT dirty bytes, not pristine commit; frozen tests carry
   uncommitted rewraps. Neither slice accept-flippable until quiesced-tree re-run.
   FINAL receipt files absent — INT §Retry + OPS2 ARE the receipts; verifier must not
   wait on FINAL files that do not exist.
4. Codex-bounds + WEB-013/015 boundary gaps OPEN (owner: providers/server lanes + test owner).
   PROV-016 B3-class (device-code / URL-length / zero-expiry) non-pinning; WEB-013
   reason-string + full T01–T05, WEB-015 full contract non-pinning by absence.
   Additive T06–T08 / gap-probe patches proposed but NOT applied (frozen-test waiver needed).
   Lifecycle-only flips possible; boundary-hardening flips blocked.
5. PROV-017 fix gate + fmt/untracked drift + AUTO-004 tokio gap (owner: providers lane +
   verifier + integrator + controller).
   One-line worktree fix per INTEGRATION-5 §2; re-run `prov_017` (+018..024) first.
   `cargo fmt --check` 43→48 (+5); status 379→392 (-1 tracked, +14 untracked).
   AUTO-004 needs card amendment (sync state machine) or tokio integration lane —
   controller dep approval either way.

Merge order: (i) attribute/revert §3 unattributed lib.rs drift; (ii) canonical
`plugin_transform` 5/5 re-run on ported bytes → drop `ext_replay_lane` same commit
(§3a); (iii) quiesce INT/OPS owned paths + frozen tests → INT/OPS re-run (§4);
(iv) T06–T08 codex-bounds tests + gap probes ONLY with test-owner waiver (§3b/§4);
(v) lane gates GREEN per wired suite (serial mandates §5); (vi) refresh stale status
files; (vii) ONE integration commit (tracked fixes + lib.rs wirings, fmt at commit);
(viii) untracked modules only with passing gates; (ix) controller syncs
FEATURES.md/accounting + flips per §6.
