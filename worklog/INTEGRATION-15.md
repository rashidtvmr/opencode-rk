# INTEGRATION-15 — per-slice GREEN matrix + new RED receipts + wiring + flip table (v15)

Rev: `248f519` HEAD. Date: 2026-09-16.
Bounds: READ-ONLY except this file. No ralph.json/controller/product edits. No fmt run. No commit.
Supersedes INTEGRATION-14 where disk disagrees (marked CORR).
Inputs: `worklog/INTEGRATION-14.md`, `worklog/GUARD-TRIAGE-15.md`, `worklog/GUARD-TRIAGE-16.md`
(latest on disk), `worklog/TURN-STREAM-GATE.md`, `worklog/TWINS-WATCH3.md`, a-wave worklogs
`RED-VALIDITY-SHARE12.md` (zA), `RED-VALIDITY-WEB16.md` (zB), `RED-VALIDITY-WEB712.md` (zC),
`RED-VALIDITY-PROV1724.md` (zD), `RED-VALIDITY-INT4.md` (zG), `RED-VALIDITY-OPS5.md` (zH),
`AUTO-TOKIO-PROPOSAL2.md`, `AUTO-005-VALIDATION2.md`, `SHARE-DEDUP3.md` (xI), `SHARE-DEDUP4.md` (yI),
`EXT-005.md` (zF pass), `WEB-013.md` / `WEB-015.md` (reason addenda), `PROV-016.md` (bounds addendum),
`REVERIFY-ZQ.md`, on-disk diffs, `ralph.json` (read-only).

Guard: GUARD-TRIAGE-15 (122 repo / 133 plan errors, FAIL backlog exhaustion, diff-check 0,
435 = 204 M + 231 ??). GUARD-TRIAGE-16 (newer, 09:42): same 122/133 FAIL, diff-check 0,
447 = 204 M + 244 ??. This lane: status **450 = 204 M + 246 ??** (+3 untracked vs TRIAGE-16,
live worktree; worklogs only). Forbidden paths untouched. GUARD RED — no integration commit.
`ralph.json`: 258 stories, 176 accepted / 44 in-progress / 38 not-started (re-verified this
lane via python count; unchanged vs v8–v14). NOT edited.

## 1. Stub / ignore scan (this lane)

- Strict grep: `grep -rn --include='*.rs' -e 'todo!' -e 'unimplemented!' -e '#\[ignore\]' crates/`
  → **0 hits** (exit 1, empty). Log: `/tmp/opencode/aP-stub.log` (0 lines). CLEAN.

## 2. Fmt + status (this lane)

- `cargo fmt --check 2>&1 | grep -c 'Diff in'` → **55** (vs v14 = 48, +7 drift; sibling-lane
  fmt churn, no fmt run this lane — write-shaped op banned).
- `git status --short`: **204 M + 246 ?? = 450** (tracked flat at 204; untracked +15 vs TRIAGE-15,
  +2 vs TRIAGE-16; drift = worklogs + additive probe suites only).
- `git diff HEAD --stat -- crates/`: **203 files, +2070/-1232** (identical totals to v14 §2).

## 3. Wiring checklist (on-disk truth, re-verified this lane via numstat)

| Crate | File | State | Diff vs HEAD | Owner worklog | Gate evidence |
|---|---|---|---|---|---|
| server | `crates/server/src/lib.rs` | WIRED-uncommitted | +20/-0 pure additive (unchanged) | SERVER-WIRING-FINAL (45/45) | `cargo check` 0; 7 suites 48 passed |
| sessions | `crates/sessions/src/lib.rs` | WIRED-uncommitted | +3/-0 (unchanged) | SESSIONS-WIRING-FINAL | focused 20; full 288 exit 0 |
| foundation | `crates/foundation/src/lib.rs` | WIRED-uncommitted | +5/-2 (unchanged) | FOUNDATION-WIRING-FINAL | check 0; repo_ref 5 + repo_cache_store 5 |
| cli | `crates/cli/src/main.rs` | n/a, no edit needed | 0 | CLI-WIRING | `--tests` 23 passed |
| providers | `crates/providers/src/lib.rs` | n/a, no edit (deliberate) | 0 | PROVIDERS-WIRING | `cargo check` exit 0 |
| security | `crates/security/src/lib.rs` | UNPLANNED-uncommitted | 1/1 alpha swap (unchanged) | none | none — attribute or revert |
| tools | `crates/tools/src/lib.rs` | UNPLANNED-uncommitted | 1/1 (unchanged) | none | none — attribute or revert |
| storage | `crates/storage/src/lib.rs` | UNPLANNED-uncommitted | +18/-9 fmt-only reflows (unchanged; zero mod add/remove) | none | none — attribute or revert |
| tools | `crates/tools/src/plugin_transform.rs` | GUARD-PORT-landed, uncommitted (unchanged) | +9/-2: unknown-scope guard + invariant docs | EXT-TWINS-DISPOSITION pair-1 | EXT2-VERIFY4 hash-check PASS (b4c517e3); canonical 5/5 re-run still mandated §6 |
| providers | `crates/providers/src/claude_oauth.rs` | FIX-landed, uncommitted (PROV-017 gate CLEARED, v14 §3b stands) | +1/-1: raw `{LOOPBACK_REDIRECT_URI}` at :462 | PROV-VERIFY4 | 11 suites 56/56 GREEN |
| providers | `crates/providers/src/codex_oauth.rs` | clean impl | `git diff` empty (unchanged) — bounds pins live in NEW test file, impl untouched | PROV-016 bounds addendum | bounds 6/6 GREEN (`v2-bounds.log`) + mut-control FAIL-as-expected |
| tools | `crates/tools/tests/ext_manifest_lane.rs` | fmt-only drift (import reorder) | M, logic untouched | EXT-005 zF pass | ext_manifest 5/5 GREEN post-restore |

- NEW untracked additive suites (no impl touch, test-owner freeze decision pending):
  `crates/providers/tests/codex_oauth_bounds.rs` (B06..B11),
  `crates/server/tests/web_capabilities_reason.rs` (3 tests),
  `crates/server/tests/web_workspace_reason.rs` (3 tests).
- TWINS-WATCH3: 0/13 drift vs WATCH2; pair-1 guard PORTED both sides, bodies byte-identical,
  droppable by integrator (NOT done here). `codex_oauth.rs` impl clean confirmed this lane.

## 4. Per-slice GREEN matrix — this wave (latest receipt per slice wins)

Serial discipline all waves: one cargo cmd at a time, `timeout 120`, `rtk` prefix,
`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`, `--test-threads=1` unless noted, restores
`cmp`/`sha256` identical, frozen tests + lib.rs + ralph.json untouched by RED lanes.
No cargo runs by THIS lane (read-only rollup). z-wave = temp-stub-restore RED at
rev `248f519` on current worktree bytes (dirty-tree caveat §4h).

| Slice | Wave receipt | GREEN after restore | RED bite shape |
|---|---|---|---|
| AUTO-004 (delegation_lane) | RED-VALIDITY-AUTO4 (yA) | 5/5 + full crate 35/35 | prior xA: 3p/2f (T02,T03 owner-detach) |
| AUTO-006 (driver_lane) | RED-VALIDITY-AUTO4 (yA) | 5/5 | prior xA: 4p/1f (T02 non-owner release) |
| AUTO-005 (delegation_gated + tool) | RED-VALIDITY-AUTO4 (yA) + AUTO-005-VALIDATION2 (aE) | 5/5 + tool PASS exit 0 + 3 mutated exit 2; aE rerun 4/4 exits match (0/2/2/2), PASS bytes identical | prior xA: 0p/5f (inverted broker) |
| agents full crate | RED-VALIDITY-AUTO4 (yA) + REVERIFY-ZQ | 35/35; zQ 15/15 re-confirm | — |
| ACP-001/002, WSX-001/002, SDK-001/002, HEAD-001/002, RUN-001 | ACPSDK-VERIFY3 (yB) | 64/64, 0 failed | prior xB per-suite bites (v14 §4) |
| EXT-001/002/004/006/008 | EXT1-VERIFY4 (yC) | 5/5 each (30/30 wave) | prior xC via EXT1C (strong) |
| EXT-005 bytes lane (`ext_manifest_lane`) | RED-VALIDITY-EXT1C (xC) + EXT-005 zF pass | 5/5; zF RED probe 4/1 (size-cap neutralize → t04 FAIL), restore-identical re-GREEN 5/5 | xC: 3p/2f (t01 const + t04 TooLarge); zF: 4/1 |
| EXT-005 canonical (`plugin_manifest`) | EXT1-VERIFY4 (yC) | 5/5 struct-shape ONLY — CONTRACT MISMATCH stands, see §4a | none vs card bytes-pipeline |
| EXT-009/010/011/012 | EXT2-VERIFY4 (yD) | 5/5 each (20/20; 011 on PORTED bytes hash b4c517e3 PASS) | prior xD (strong) |
| TOOL-016..020, UI-019, SYNC-001/002 | TOOL-VERIFY4 (yJ) + REVERIFY-ZQ (40/40 re-confirm) | 5/5 each (40/40 wave) | prior xJ (strong) |
| SHARE-001 | RED-VALIDITY-SHARE12 (zA) + yI | **NEW behavior-RED** 4p/1f (t01 last-write-wins, `:50:5`) + 5/5 restore + lane-mirror 5/5 + yI probe PASS | none before (GREEN+probe by design) |
| SHARE-002 | RED-VALIDITY-SHARE12 (zA) + yI | **NEW behavior-RED** 4p/1f (t01 coalesce-drain, `:54:13`) + 5/5 restore + lane-mirror 5/5 + yI probe PASS | none before |
| SHARE-003 | SHARE-DEDUP3 (xI) + yI | 5/5 restore; xI RED 2p/3f (t01,t02,t04 wrong-partition) | xI temp-stub RED |
| SHARE-004 | SHARE-DEDUP3 (xI) + yI | 5/5 restore; xI RED 3p/2f (t02,t04 dup-overwrite) | xI temp-stub RED |
| SHARE-005 | SHARE-DEDUP3 (xI) + yI | 5/5 restore; xI RED 4p/1f (t02 classify-flip) | xI temp-stub RED |
| SHARE total | yI-share/probe | 50/50 (10 suites) + TOTAL_LEAK_BYTES=0 | — |
| WEB-001 | RED-VALIDITY-WEB16 (zB) + yK | **NEW RED** 5/5 FAILED (force-Err MissingField) + 5/5 restore | none before (GREEN-only) |
| WEB-002 | RED-VALIDITY-WEB16 (zB) + yK | **NEW RED** 4 FAILED, T04 ok (helper untouched — valid survival) + 5/5 restore | none before |
| WEB-003 | RED-VALIDITY-WEB16 (zB) + yK | **NEW RED** 3 FAILED, T03+T04 ok (pure helpers — valid survival) + 5/5 restore | none before |
| WEB-004 | RED-VALIDITY-WEB16 (zB) + yK | **NEW RED** 3 FAILED, T01+T04 ok (allow-paths — valid survival) + 5/5 restore | none before |
| WEB-005 | RED-VALIDITY-WEB16 (zB) + yK | **NEW RED** 5/5 FAILED + 5/5 restore | none before |
| WEB-006 | RED-VALIDITY-WEB16 (zB) + yK | **NEW RED** 2/2 FAILED (lock stub, `/tmp` copy; repo file unmodified) + 2/2 GREEN | none before |
| WEB-007 | RED-VALIDITY-WEB712 (zC) + yK | **NEW RED** 4p/1f (T01 geometry) + 5/5 restore, `cmp` identical | none before (HOLD by design) |
| WEB-008 | RED-VALIDITY-WEB712 (zC) + yK | **NEW RED** 2p/3f (T01,T04,T05 fork-fail; T02+T03 survival valid) + 5/5 restore | none before |
| WEB-009 | RED-VALIDITY-WEB712 (zC) + yK | **NEW RED** 0p/5f + 5/5 restore | none before |
| WEB-010 | RED-VALIDITY-WEB712 (zC) + yK | **NEW RED** 1p/4f (T03 pure-path survival valid) + 5/5 restore | none before |
| WEB-011 | RED-VALIDITY-WEB712 (zC) + yK | **NEW RED** 0p/5f + 5/5 restore | none before |
| WEB-012 | RED-VALIDITY-WEB712 (zC) + yK | **NEW RED** 1p/4f (T03 no-select-path survival valid) + 5/5 restore | none before |
| WEB total | WEB-VERIFY4 (yK) | 107 passed, 0 failed; `cargo check -p opencode-rk-server` 0 errors | entry-point RED now extended to 001–012 (zB+zC+s3) |
| WEB-013 (sessions entry-point) | RED-VALIDITY-WEB-PROV (s3) + reason addendum | 5/5; +3/3 reason probes GREEN w/ mut-control FAIL-as-expected | 0p/5f (adapter gate) |
| WEB-014 | RED-VALIDITY-WEB-PROV (s3) | 5/5 | 3p/2f (T01,T04) |
| WEB-015 (sessions entry-point) | RED-VALIDITY-WEB-PROV (s3) + reason addendum | 5/5 (2/2 frozen T01+T02); +3/3 reason probes GREEN w/ mut-control | 0p/5f (ghost id) |
| WEB-016 | RED-VALIDITY-WEB-PROV (s3) | 5/5 | 1p/4f (T02 neg-path survives) |
| WEB-017 | RED-VALIDITY-WEB-PROV (s3) | 5/5 | 1p/4f (T02 gate survives) |
| PROV-015 | PROV-VERIFY4 (yG) | 5/5 (56/56 wave) | uD A1/A2/A3 all bite (strong) |
| PROV-016 lifecycle | PROV-VERIFY4 (yG) | 5/5 + bounds 6/6 | uD B1/B2 pins |
| PROV-016 boundary | PROV-016 bounds addendum (B06..B11, NEW `codex_oauth_bounds.rs`) | 6/6 GREEN (`v2-bounds.log`) + mut-control 5p/1f FAIL-as-expected; frozen `codex_oauth.rs` + impl untouched | uD B3/B3alt/B3exp stubs now KILLED by additive suite (freeze waiver still needed, §4b) |
| PROV-017 | PROV1724 (zD, read-only) + yG | 5/5 GREEN, fix PRESENT (raw const interp :462) | RED history in per-lane worklogs (4/5 pre-fix BadConsentUrl) |
| PROV-018 | PROV1724 (zD) + yG | **NEW behavior-RED** 2p/3f (T01,T02,T04) + 5/5 restore-identical | none before |
| PROV-019 | PROV1724 (zD) + yG | **NEW behavior-RED** 0p/5f + 5/5 restore | none before |
| PROV-020 | PROV1724 (zD) + yG | **NEW behavior-RED** 2p/3f (T01,T03,T04) + 5/5 restore | none before |
| PROV-021 | PROV1724 (zD) + yG | **NEW behavior-RED** 1p/4f (T04 ok) + 5/5 restore | none before |
| PROV-022 | PROV1724 (zD) + yG | **NEW behavior-RED** 1p/4f (T02 ok) + 5/5 restore | none before |
| PROV-023 | PROV1724 (zD) + yG | **NEW fixture-RED** 0/5 (catalog moved away) + 5/5 restore sha-identical | none before |
| PROV-024 | PROV1724 (zD) + yG | **NEW fixture-RED** 0/5 (contracts moved away) + 5/5 restore sha-identical | none before |
| WEB-013 server boundary | RED-VALIDITY-WEB013-015 (uC) | 1/1 | flag-flip FAIL, missing-key FAIL; reason-text-only PASSES |
| WEB-015 server boundary | RED-VALIDITY-WEB013-015 (uC) | 2/2 | 3/3 stub strategies bite |
| OPS-001..009 | OPS-VERIFY4 (yF) + RED-VALIDITY-OPS5 (zH) | 5/5 each, 45/45; zH per-ID RED rc=101 (assertion-fail, 0 compile-fail) + `cmp` IDENTICAL restore + 5/5 re-GREEN | uB FINAL epoch (strong, behavior) |
| INT-001/002/003/005/006/007/009/010 | INT-VERIFY4 (yE) + RED-VALIDITY-INT4 (zG) | 5/5 each, 40/40; zG per-ID RED (2/3,0/5,3/2,3/2,4/1,4/1,2/3,0/5) + byte-identical restore to recorded current-tree hashes | uA FINAL epoch (strong, behavior) |
| session_turn_stream_api | TURN-STREAM-GATE (3x serial) + yK re-confirm | 2/2 ×3 runs + 2/2 in yK sweep | n/a (harness race, not behavior RED) |
| EXT002-T05 | EXT002-T05-DETERMINISM | serial 30/30 | flaky-by-construction; NO-FIX |
| EXT twins (8 pairs) | EXT-TWINS-DISPOSITION + TWINS-WATCH3 | 0/13 drift vs WATCH2; pair-1 guard PORTED both sides | pair-1 droppable; pairs 2–8 drop-pending |
| SHARE twins (5 pairs) | SHARE-TWINS-DISPOSITION + TWINS-WATCH3 | survey only, 0 drift | keep-both everywhere |
| REL-001..003 | lane worklogs only | lane GREEN | none — validator-only, verifier re-run pending |

### 4a. EXT-005 two-lane status (must-flag, UPDATED this wave)

- Canonical `plugin_manifest.rs` (55L, WIRED): struct-only contract, yC GREEN 5/5 proves
  struct-shape only, NOT card bytes-pipeline. y-caveat STANDS for the canonical path.
- Bytes lane `ext_manifest_lane.rs` (UNWIRED, `#[path]` test): zF pass proves bytes pipeline
  PRESENT (`validate_manifest_bytes`, MAX 16384/128/16/64, version 1, length→UTF8→JSON→fields,
  unknown-ignored, missing-caps→`[]`), GREEN 5/5 + RED probe 4/1 + restore-identical;
  xC RED 3/2 independent confirm. Bytes-lane evidence is y.
- Integrator call: wire `ext_manifest_lane` (or port bytes pipeline into canonical) + test-owner
  freeze decision. Until then EXT-005 stays y-caveat in §7 (count-neutral vs v14).

### 4b. PROV-016 boundary upgrade (bounds2)

- NEW untracked `crates/providers/tests/codex_oauth_bounds.rs`: B06..B11 (empty/oversize device
  code incl. MAX=128 exact-Ok, URL >2048B reject + 2048B Ok, zero-expiry reject on
  complete/refresh incl. Expired state, stale-expiry reject) GREEN 6/6 + disposable
  mut-control FAIL-as-expected (5p/1f). Impl + frozen suite untouched.
- Kills uD B3/B3alt/B3exp stub classes as behavior. LIFT: partial → **y-caveat** (acceptance
  still needs controller/test-owner freeze of the additive suite; card T01–T05 frozen
  contract itself unchanged).

### 4h. Dirty-tree caveat (carried, qualifies z-wave RED)

- zA/zC/zD/zG/zH stubs applied to CURRENT (dirty) bytes, restored byte-identical
  (sha256/cmp ALL_IDENTICAL, zero TEMP markers). RED proves test→impl wiring on current
  content, not pristine-commit validity. Quiesced-tree re-run still required before
  accept-flip for INT/OPS/SHARE y-caveat rows. zB used standalone `rustc --test` (workspace
  `cargo test -p` blocked by unrelated `sessions/share_merge.rs` parse error) + prebuilt
  rlibs; method weaker than cargo, restores sha-identical; re-run under clean cargo
  before accept-flip.

## 5. RED receipt inventory (which waves cover which IDs)

| Wave file | Covers | Verdict |
|---|---|---|
| `RED-VALIDITY-AUTO4.md` (yA-*.log, agents-full) | AUTO-004/006/005 | GREEN re-confirm 35/35. RED bite via AUTO3 (xA). MECHANISM CAVEAT persists: tokio REQUIRED NOT MET (PROPOSAL2 unapplied); broker mirror SUFFICES. |
| `AUTO-005-VALIDATION2.md` (aE-*.json) | AUTO-005 tool | 4/4 exits+reasons match proposal; PASS bytes identical. Freeze reproducible. |
| `AUTO-TOKIO-PROPOSAL2.md` | AUTO-004 mechanism | Tokio gap spec'd (semaphore + JoinHandle + abort-timeout plan); NOT applied, controller approval pending. |
| `ACPSDK-VERIFY3.md` (yB-*.log) | ACP-001/002, WSX-001/002, SDK-001/002, HEAD-001/002, RUN-001 | GREEN 64/64. RED bite via ACPSDK2 (xB, strong). |
| `EXT1-VERIFY4.md` (yC-ext1.log) | EXT-001/002/004/005-canonical/006/008 | GREEN 30/30. RED bite via EXT1C (xC, strong). EXT-005 canonical caveat §4a. |
| `RED-VALIDITY-EXT1C.md` (xC-*-red.log) | EXT-001/002/004/005-bytes/006/008 | Behavior-RED incl. EXT-005 bytes-lane 3/2. |
| `EXT2-VERIFY4.md` (yD-ext2.log) | EXT-009/010/011/012 | GREEN 20/20. RED bite via EXT2C (xD, strong). Guard-port hash PASS. |
| `TOOL-VERIFY4.md` (yJ-tool.log) + REVERIFY-ZQ | TOOL-016..020, UI-019, SYNC-001/002 | GREEN 40/40, zQ re-confirm 40/40. RED bite via TOOL3 (xJ, strong). |
| `RED-VALIDITY-SHARE12.md` (zA-*-red.log) | **SHARE-001/002** | **NEW behavior-RED 4/1 + 4/1, restore-identical, lane-mirrors 5/5.** |
| `SHARE-DEDUP3.md` (xI-*-red.log) | SHARE-003/004/005 | Temp-stub RED 3/2 + 2/3 + 1/4, 6/6 sha-restore, 50/50 GREEN. |
| `SHARE-DEDUP4.md` (yI-share/probe.log) | SHARE-001..005 | GREEN 50/50 + redaction probe PASS (0 leak bytes). |
| `RED-VALIDITY-WEB16.md` (zB-*-red.log) | **WEB-001..006** | **NEW temp-stub RED (5/5,4F,3F,3F,5/5,2/2) + sha-identical restores + GREEN.** Method: standalone rustc (cargo blocked, §4h). |
| `RED-VALIDITY-WEB712.md` (zC-*-red/green.log) | **WEB-007..012** | **NEW entry-point RED (1F,3F,5F,4F,5F,4F) + 5/5 GREEN + `cmp` identical + TEMP-marker 0.** Write-paths stayed disabled. |
| `WEB-VERIFY4.md` (yK-web.log) | WEB-001..017 suites | GREEN sweep 107/107 on current bytes. Browser vitest/tsc unexecuted. |
| `RED-VALIDITY-PROV1724.md` (zD-*-red/green.log) | **PROV-017(read-only)..024** | **NEW per-ID RED (018 3F, 019 5F, 020 3F, 021 4F, 022 4F, 023/024 fixture 5F) + 40/40 GREEN, restores sha-identical.** 017 fix-present YES. |
| `PROV-VERIFY4.md` (yG-prov.log) | PROV-015..024 | GREEN 56/56 WITH fix present. |
| `PROV-016.md` bounds addendum (`v2-bounds.log`, `v2-mut-control.log`) | **PROV-016 boundary** | **NEW B06..B11 6/6 GREEN + mut-control FAIL-as-expected.** |
| `WEB-013.md` / `WEB-015.md` reason addenda (`zE-*-mut.log`) | **WEB-013/015 boundary** | **NEW 3/3 + 3/3 reason probes GREEN + mut FAIL-as-expected.** Frozen T01–T05 still absent; uC WEAK-RED stands. |
| `RED-VALIDITY-INT4.md` (zG-*-red.log, g1..g8) | INT-001/002/003/005/006/007/009/010 | Per-ID RED + 5/5 restores to recorded current-tree hashes. Dirty-tree caveat §4h. |
| `INT-VERIFY4.md` (yE-int.log) | INT-* (same 8) | GREEN 40/40 on current bytes. |
| `RED-VALIDITY-OPS5.md` (zH-OPS-*-red.log) | OPS-001..009 | Per-ID RED rc=101 (assertion-only) + `cmp` IDENTICAL + 5/5 re-GREEN. |
| `OPS-VERIFY4.md` (yF-ops.log) | OPS-001..009 | GREEN 45/45 + clean `cargo check`. |
| `RED-VALIDITY-WEB-PROV.md` (s3) | WEB-013/014/015/016/017 + PROV-015/016 entry-point | RED-VALID (strong, entry-point). Qualified by uC/uD. |
| `RED-VALIDITY-WEB013-015.md` (uC) | WEB-013/015 server boundary | WEAK-RED (partial pinning). Reason addenda extend pinning to reason-strings, NOT full T01–T05. |
| `RED-VALIDITY-PROV015-016.md` (uD) | PROV-015 (strong) / PROV-016 (lifecycle pins) | Boundary B3-class now killed by bounds2 additive suite (freeze pending). |
| `TURN-STREAM-GATE.md` | session_turn_stream_api | serial-mandate only (§6). |
| Superseded epochs | EXT1/EXT1B, EXT2, AUTO/AUTO2/AUTO3, TOOL/TOOL2, ACPSDK/ACPSDK2, OPS/OPS2/OPS3/OPS4, INT/INT2/INT3, WEB-007-012-ACCEPT | archaeology; latest wave wins on conflict. |

## 6. Serial-test mandates (binding on verifier/CI)

```
# stream binary: env race (TURN-STREAM-GATE §2)
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-server --test session_turn_stream_api -- --test-threads=1
# ext_builtins_lane binary: T05 thread-count race (EXT002-T05-DETERMINISM §2-3)
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-tools --test ext_builtins_lane -- --test-threads=1
```

- Stream root cause: process-global env race (unlocked `EnvGuard`, 2 tests same binary).
  3x serial GREEN + yK re-confirm 2/2. No server bug; `TURN_PERMITS` cap not a fix.
- T05 root cause: `/proc/self/task` counts libtest siblings; impl deterministic (zero spawns).
  Parallel stays flaky; serial 30/30 GREEN. Treat parallel line-175 failures as harness noise.
- Do NOT run either binary `--test-threads=N>1`; do NOT parallelize with heavy jobs
  (8 GiB budget, one validation at a time). Do NOT edit frozen tests.
- `env_lock` proposal + T05 options: NOT applied — verifier/test-owner authority.
- Canonical `plugin_transform` 5/5 re-run on ported bytes still mandated before twin drop (§3).

## 7. ralph flip table — all 82 non-accepted IDs (EVIDENCE ONLY — do NOT edit)

ready = behavior-RED receipt complete + GREEN on disk (wiring-uncommitted does not
block evidence readiness). `caveat` = acceptance still blocked (see note).
Counts: **ready-y 77 / partial 2 / not-ready 3** (total 82 = 44 in-progress + 38 not-started).

y-evidence lifts vs v14 (70 → 77, +7):
- WEB-007..012 (+6): n → y-caveat. WEB712 gives what v14 lacked entirely: per-ID
  entry-point RED (007 4/1, 008 2/3, 009 0/5, 010 1/4, 011 0/5, 012 1/4) + 5/5 GREEN +
  `cmp`-identical restores. y-caveat (not full y): standalone-rustc method (§4h),
  frozen-test fmt drift pre-existing, write-paths disabled by design, browser unexecuted.
- PROV-016 (+1): partial → y-caveat. Bounds2 B06..B11 GREEN + mut-control kills uD
  B3/B3alt/B3exp stub classes behaviorally (§4b). y-caveat (not full y): additive suite
  untracked, needs controller/test-owner freeze; card T01–T05 frozen text unchanged.
- SHARE-001/002 (count-neutral): y-caveat → y. SHARE12 adds behavior-RED (4/1 each) +
  restore-identical + lane-mirror 5/5 atop yI probe PASS. Full y (dirty-tree re-run
  still standard pre-accept hygiene, no extra caveat).
- WEB-001..006 (count-neutral): y-caveat → y. WEB16 adds per-ID RED (incl. full-fail
  001/005) + sha-identical restores. Full y modulo §4h cargo-method note.
- PROV-017..024 (count-neutral): y stands, evidence upgraded — PROV1724 adds independent
  per-ID RED (018–022 behavior, 023/024 fixture) + 40/40 GREEN; 017 read-only fix-present.
- EXT-005 (count-neutral): y-caveat STANDS — canonical still struct-shape (§4a); bytes-lane
  now y-grade but unwired; integrator canonicalization pending.
- WEB-013/015 (count-neutral): partial STANDS — reason addenda pin reason-strings
  (3/3 + 3/3 + mut-controls) but frozen T01–T05 still absent; uC WEAK-RED stands.
- INT/OPS (count-neutral): y-caveat stands — INT4/OPS5 add per-ID RED on recorded
  current-tree hashes, but quiesced-tree re-run still required (§4h).

| # | Story | ralph now | ready | Evidence path / note |
|---|---|---|---|---|
| 1 | AUTO-004 | in-progress | y-caveat | yA GREEN 5/5 + full 35/35; RED bite via xA 3/2; CAVEAT tokio-mechanism (card amendment or integration lane, PROPOSAL2 unapplied) |
| 2 | AUTO-005 | in-progress | y | yA GREEN 5/5 + tool PASS/mutated-exit-2; aE rerun 4/4 reproducible, PASS bytes identical |
| 3 | AUTO-006 | in-progress | y-caveat | yA GREEN 5/5; RED bite via xA 4/1; same tokio-pool caveat as AUTO-004 |
| 4 | EXT-001 | in-progress | y | yC GREEN 5/5; RED bite via xC 4/1 |
| 5 | EXT-002 | in-progress | y | yC GREEN 5/5; RED bite via xC 4/1 |
| 6 | EXT-004 | in-progress | y | yC GREEN 5/5; RED bite via xC 3/2 |
| 7 | EXT-005 | in-progress | y-caveat | BYTES-LANE y (xC 3/2 + zF 4/1 probe, restore-identical) BUT canonical `plugin_manifest` struct-shape only — INTEGRATOR canonicalization (§4a) |
| 8 | EXT-006 | in-progress | y | yC GREEN 5/5; RED bite via xC 4/1 |
| 9 | EXT-008 | in-progress | y | yC GREEN 5/5; RED bite via xC 4/1 |
| 10 | EXT-009 | in-progress | y | yD GREEN 5/5; RED bite via xD 0/5 |
| 11 | EXT-010 | in-progress | y | yD GREEN 5/5; RED bite via xD 1/4 |
| 12 | EXT-011 | in-progress | y | yD GREEN 5/5 on PORTED bytes (hash b4c517e3 PASS); RED bite via xD 0/5 |
| 13 | EXT-012 | in-progress | y | yD GREEN 5/5; RED bite via xD 0/5 |
| 14 | INT-001 | in-progress | y-caveat | zG RED 2/3 + yE GREEN 5/5 on recorded current-tree hashes; CAVEAT quiesced-tree re-run before accept-flip |
| 15 | INT-002 | in-progress | y-caveat | zG RED 0/5 + yE GREEN 5/5; same caveat |
| 16 | INT-003 | in-progress | y-caveat | zG RED 3/2 + yE GREEN 5/5; same caveat |
| 17 | INT-005 | in-progress | y-caveat | zG RED 3/2 + yE GREEN 5/5; same caveat |
| 18 | INT-006 | in-progress | y-caveat | zG RED 4/1 + yE GREEN 5/5; same caveat |
| 19 | INT-007 | in-progress | y-caveat | zG RED 4/1 + yE GREEN 5/5; same caveat |
| 20 | INT-009 | in-progress | y-caveat | zG RED 2/3 + yE GREEN 5/5; same caveat |
| 21 | INT-010 | in-progress | y-caveat | zG RED 0/5 + yE GREEN 5/5; same caveat |
| 22 | OPS-001 | in-progress | y-caveat | zH RED 3/2 rc=101 assertion-only + `cmp` IDENTICAL + yF GREEN 5/5; CAVEAT quiesced-tree re-run |
| 23 | OPS-002 | in-progress | y-caveat | zH RED 0/5 + yF GREEN 5/5; same caveat |
| 24 | OPS-003 | in-progress | y-caveat | zH RED 2/3 + yF GREEN 5/5; same caveat |
| 25 | OPS-004 | in-progress | y-caveat | zH RED 4/1 + yF GREEN 5/5; same caveat |
| 26 | OPS-005 | in-progress | y-caveat | zH RED 4/1 + yF GREEN 5/5; same caveat |
| 27 | OPS-006 | in-progress | y-caveat | zH RED 4/1 + yF GREEN 5/5; same caveat |
| 28 | OPS-007 | in-progress | y-caveat | zH RED 2/3 + yF GREEN 5/5; same caveat |
| 29 | OPS-008 | in-progress | y-caveat | zH RED 2/3 + yF GREEN 5/5; same caveat |
| 30 | OPS-009 | in-progress | y-caveat | zH RED 2/3 + yF GREEN 5/5; same caveat |
| 31 | REL-001 | in-progress | n | validator-only, lane GREEN, verifier re-run pending |
| 32 | REL-002 | in-progress | n | same as REL-001 |
| 33 | REL-003 | in-progress | n | same as REL-001 |
| 34 | SHARE-001 | in-progress | y | LIFT vs v14 (was y-caveat): zA behavior-RED 4/1 + restore-identical + lane-mirror 5/5 + yI probe PASS |
| 35 | SHARE-002 | in-progress | y | LIFT vs v14: zA behavior-RED 4/1 + restore-identical + lane-mirror 5/5 + yI probe PASS |
| 36 | SHARE-003 | in-progress | y-caveat | xI RED 2/3 + yI 50/50 + probe PASS; CAVEAT unwired lane + quiesced-tree re-run |
| 37 | SHARE-004 | in-progress | y-caveat | xI RED 3/2 + yI evidence; same caveat |
| 38 | SHARE-005 | in-progress | y-caveat | xI RED 4/1 + yI evidence; same caveat |
| 39 | WEB-001 | in-progress | y | LIFT vs v14 (was y-caveat): zB RED 5/5 full-fail + sha-identical restore + GREEN |
| 40 | WEB-002 | in-progress | y | LIFT vs v14: zB RED 4F (T04 valid survival) + restore + GREEN |
| 41 | WEB-003 | in-progress | y | LIFT vs v14: zB RED 3F (T03/T04 valid survival) + restore + GREEN |
| 42 | WEB-004 | in-progress | y | LIFT vs v14: zB RED 3F (T01/T04 valid survival) + restore + GREEN |
| 43 | WEB-005 | in-progress | y | LIFT vs v14: zB RED 5/5 full-fail + restore + GREEN |
| 44 | WEB-006 | in-progress | y | LIFT vs v14: zB RED 2/2 (lock stub, repo file unmodified) + GREEN |
| 45 | WEB-007 | not-started | y-caveat | LIFT vs v14 (was n): zC entry-point RED 4/1 + 5/5 GREEN + `cmp` identical; CAVEAT write-paths disabled by design + §4h method + browser unexecuted; controller decides flip |
| 46 | WEB-008 | not-started | y-caveat | LIFT vs v14: zC RED 2/3 (T02/T03 valid survival) + GREEN; same caveat; controller decides flip |
| 47 | WEB-009 | not-started | y-caveat | LIFT vs v14: zC RED 0/5 + GREEN; same caveat; controller decides flip |
| 48 | WEB-010 | not-started | y-caveat | LIFT vs v14: zC RED 1/4 (T03 valid survival) + GREEN; same caveat; controller decides flip |
| 49 | WEB-011 | not-started | y-caveat | LIFT vs v14: zC RED 0/5 + GREEN; same caveat; controller decides flip |
| 50 | WEB-012 | not-started | y-caveat | LIFT vs v14: zC RED 1/4 (T03 valid survival) + GREEN; same caveat; controller decides flip |
| 51 | WEB-013 | not-started | partial | entry-point RED-VALID (s3 0/5) + uC WEAK-RED + NEW reason probes 3/3 (mut-control PASS); full T01–T05 still absent; controller decides flip |
| 52 | WEB-014 | not-started | y | entry-point RED-VALID (s3 3/2) + GREEN 5/5; controller decides flip |
| 53 | WEB-015 | not-started | partial | entry-point RED-VALID (s3 0/5) + uC WEAK-RED + NEW reason probes 3/3; full contract still absent; controller decides flip |
| 54 | WEB-016 | not-started | y | entry-point RED-VALID (s3 1/4) + GREEN 5/5; controller decides flip |
| 55 | WEB-017 | not-started | y | entry-point RED-VALID (s3 1/4) + GREEN 5/5; controller decides flip |
| 56 | PROV-015 | not-started | y | RED-VALID strong (uD A1/A2/A3) + yG GREEN 5/5; controller decides flip |
| 57 | PROV-016 | not-started | y-caveat | LIFT vs v14 (was partial): lifecycle pins + NEW bounds2 B06..B11 6/6 + mut-control kills B3-class (§4b); CAVEAT additive suite untracked, freeze waiver needed; controller decides flip |
| 58 | PROV-017 | not-started | y | zD read-only fix-present + yG GREEN 5/5; controller decides flip |
| 59 | PROV-018 | not-started | y | UPGRADED vs v14: zD NEW behavior-RED 2/3 + restore-identical + yG GREEN; controller decides flip |
| 60 | PROV-019 | not-started | y | UPGRADED vs v14: zD NEW behavior-RED 0/5 + restore + GREEN; controller decides flip |
| 61 | PROV-020 | not-started | y | UPGRADED vs v14: zD NEW behavior-RED 2/3 + restore + GREEN; controller decides flip |
| 62 | PROV-021 | not-started | y | UPGRADED vs v14: zD NEW behavior-RED 1/4 + restore + GREEN; controller decides flip |
| 63 | PROV-022 | not-started | y | UPGRADED vs v14: zD NEW behavior-RED 1/4 + restore + GREEN; controller decides flip |
| 64 | PROV-023 | not-started | y | UPGRADED vs v14: zD NEW fixture-RED 0/5 + sha-identical restore + GREEN; controller decides flip |
| 65 | PROV-024 | not-started | y | UPGRADED vs v14: zD NEW fixture-RED 0/5 + sha-identical restore + GREEN; controller decides flip |
| 66 | UI-019 | not-started | y | yJ GREEN 5/5; RED bite via xJ 1/4; review → in-progress? |
| 67 | TOOL-016 | not-started | y | yJ GREEN 5/5; RED bite via xJ 1/4; review → in-progress? |
| 68 | TOOL-017 | not-started | y | yJ GREEN 5/5; RED bite via xJ 1/4; review → in-progress? |
| 69 | TOOL-018 | not-started | y | yJ GREEN 5/5; RED bite via xJ 4/1; review → in-progress? |
| 70 | TOOL-019 | not-started | y | yJ GREEN 5/5; RED bite via xJ 1/4; review → in-progress? |
| 71 | TOOL-020 | not-started | y | yJ GREEN 5/5; RED bite via xJ 3/2; review → in-progress? |
| 72 | SYNC-001 | not-started | y | yJ GREEN 5/5; RED bite via xJ 0/5 full-fail; review → in-progress? |
| 73 | SYNC-002 | not-started | y | yJ GREEN 5/5; RED bite via xJ 3/2; review → in-progress? |
| 74 | RUN-001 | not-started | y | yB GREEN 5/5; RED bite via xB 0/5, WIRED-uncommitted §3 |
| 75 | ACP-001 | not-started | y | yB GREEN 5/5; RED bite via xB 3/2, WIRED-uncommitted §3 |
| 76 | ACP-002 | not-started | y | yB GREEN 5/5; RED bite via xB 3/2, WIRED-uncommitted §3 |
| 77 | WSX-001 | not-started | y | yB GREEN 5/5; RED bite via xB 4/1, WIRED-uncommitted §3 |
| 78 | WSX-002 | not-started | y | yB GREEN 5/5; RED bite via xB 4/1, WIRED-uncommitted §3 |
| 79 | SDK-001 | not-started | y | yB GREEN 12/12; RED bite via xB 6/6, WIRED-uncommitted §3 |
| 80 | SDK-002 | not-started | y | yB GREEN 11/11; RED bite via xB 1/10, WIRED-uncommitted §3 |
| 81 | HEAD-001 | not-started | y | yB GREEN 8/8; RED bite via xB 2/6, NO wiring needed §3 |
| 82 | HEAD-002 | not-started | y | yB GREEN 8/8; RED bite via xB 7/1, NO wiring needed §3 |

Plus (not stories, controller-owned): unknown prefixes SYNC/RUN/ACP/WSX/SDK/HEAD →
allowlist-or-map + reconcile accounting (plan validator 11 extra); FEATURES stale-accepted
set → FEATURES.md sync only (122 lines: 69 stale-mirror + 53 ledger/gap).
EXT twins pairs 2–8 → integrator executes drops per §3 AFTER pair-1 GREEN re-run.
EXT-005 card-vs-canonical gap → wire bytes lane or gap-closure (§4a).

## 8. Ranked blockers top 5 (owner lanes)

1. Wired-but-uncommitted + ungated + unattributed lib.rs drift + LANDED guard-port
   un-gated (owner: wiring lanes + verifier + integrator).
   Server +20, sessions +3, foundation +3 mods in worktree, zero committed.
   security/storage/tools lib.rs drift still no owning lane (storage +18/-9 fmt-only) —
   attribute or revert before the integration commit.
   `plugin_transform` guard-port (+9/-2) hash-verified on ported bytes (yD) but
   still needs verifier-logged serial 5/5 re-run, then twin drop same commit.
   Action: serial lane-gate GREEN per suite (§6 mandates), then ONE integration commit.
2. Guard FAIL backlog exhaustion (owner: controller). 122 errors exit 1 + 133 plan
   (122 + 11 unknown prefixes), byte-identical sets vs TRIAGE-15/16. No lane edits ralph.json.
3. EXT-005 canonicalization + PROV-016 freeze + WEB-007..012 HOLD release (owner:
   controller + test owner + integrator). Three acceptance decisions, evidence ready:
   (a) EXT-005: wire `ext_manifest_lane` bytes pipeline (y-grade) or gap-close the card —
   canonical proves struct-shape only (§4a); (b) PROV-016: freeze additive B06..B11 suite
   (untracked) or re-author as frozen T06–T08; (c) WEB-007..012: release GREEN-only HOLD
   (entry-point RED now exists, write-paths disabled by design) and flip not-started →
   in-progress per §7 rows 45–50.
4. Quiesced-tree re-run for INT/OPS/SHARE y-caveat rows + zB cargo-method upgrade
   (owner: INT/OPS/SHARE/WEB lanes + verifier). zA/zC/zD/zG/zH restores byte-identical
   but witnessed on dirty tree; zB used standalone rustc (cargo blocked by unrelated
   `share_merge.rs` parse error). Neither slice accept-flippable until clean-tree re-run.
   PROV-017 gate stays CLEARED (no action).
5. AUTO-004 tokio gap + fmt/untracked drift (owner: controller + integrator).
   AUTO-004/006 need card amendment (sync state machine) or tokio integration lane —
   PROPOSAL2 spec'd, NOT applied, controller dep approval either way.
   `cargo fmt --check` 48 → **55** (+7 sibling churn); status 435 → **450**
   (+15 untracked vs TRIAGE-15: worklogs + 3 additive probe suites, CORR).
   New untracked suites (`codex_oauth_bounds`, `web_capabilities_reason`,
   `web_workspace_reason`) need freeze-or-drop decision before the integration commit.

Merge order: (i) attribute/revert §3 unattributed lib.rs drift; (ii) verifier-logged
serial `plugin_transform` 5/5 on ported bytes → drop `ext_replay_lane` same commit;
(iii) freeze-or-drop the 3 additive probe suites; (iv) EXT-005 canonicalization (§4a);
(v) quiesce INT/OPS/SHARE owned paths + frozen tests → INT/OPS/SHARE + zB-cargo re-run
(§4h); (vi) lane gates GREEN per wired suite (serial mandates §6); (vii) refresh stale
status files; (viii) ONE integration commit (tracked fixes + lib.rs wirings incl.
PROV-017 one-liner, fmt at commit); (ix) untracked modules only with passing gates;
(x) controller syncs FEATURES.md/accounting + flips per §7.
