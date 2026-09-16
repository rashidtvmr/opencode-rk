# INTEGRATION-16 — per-slice GREEN matrix + b-wave receipts + wiring + flip table (v16)

Rev: `248f519` HEAD. Date: 2026-09-16.
Bounds: READ-ONLY except this file. No ralph.json/controller/product edits. No fmt run. No commit.
Supersedes INTEGRATION-15 where disk disagrees (marked CORR).
Inputs: `worklog/INTEGRATION-15.md`, `worklog/GUARD-TRIAGE-16.md`, `worklog/GUARD-TRIAGE-17.md`
(newest), `worklog/TURN-STREAM-GATE.md`, `worklog/TWINS-WATCH4.md` (newest), b-wave worklogs
`SHARE-DEDUP5.md` (aA), `PROV-016.md` bounds2 addendum, `PROV-016-FREEZE.md` (bE),
`WEB-013.md` / `WEB-015.md` full-suite addenda (bC/bD), `REL-LANEGATE.md` (bF),
`RED-VALIDITY-SHARE12.md` (zA), `RED-VALIDITY-WEB16.md` (zB), `RED-VALIDITY-WEB712.md` (zC),
`RED-VALIDITY-PROV1724.md` (zD), `RED-VALIDITY-INT4.md` (zG + aG/bG confirms),
`RED-VALIDITY-OPS5.md` (zH + aH/bH confirms), `RED-VALIDITY-AUTO4.md` (yA),
`REVERIFY-ZQ.md`, on-disk diffs, `ralph.json` (read-only, key CORR §7).

Guard: TRIAGE-16 (122 repo / 133 plan FAIL, diff-check 0, 447 = 204 M + 244 ??).
TRIAGE-17 (newer): same 122/133 FAIL, diff-check 0, 456 = 204 M + 252 ??.
This lane: **461 = 204 M + 257 ??** (+5 untracked vs TRIAGE-17; live worktree, worklogs +
additive suites only). Forbidden paths untouched. GUARD RED — no integration commit.
`ralph.json`: 258 stories, **176 accepted / 44 in-progress / 38 not-started**
(re-verified this lane via python count on `userStories` key; CORR vs v15 which cited
`stories` key — same numbers). NOT edited.

## 1. Stub / ignore scan (this lane)

- Strict grep: `grep -rn --include='*.rs' -e 'todo!' -e 'unimplemented!' -e '#\[ignore\]' crates/`
  → **0 hits** (exit 1, empty). Log: `/tmp/opencode/bP-stub.log` (0 lines, this lane). CLEAN.
  Matches STUB-FMT-8 (`/tmp/opencode/bO-stub.log`, 0 bytes).

## 2. Fmt + status (this lane)

- `cargo fmt --check 2>&1 | grep -c 'Diff in'` → **58** (vs v15 = 55, +3 drift; sibling-lane
  fmt churn, no fmt run this lane — write-shaped op banned). Matches STUB-FMT-8 raw count 58
  (STUB-FMT-7 rtk-filtered 48 is filter artifact, not drift).
- `git status --short`: **204 M + 257 ?? = 461** (tracked flat at 204; untracked +5 vs TRIAGE-17,
  +11 vs TRIAGE-16; drift = worklogs + additive suites only).
- `git diff HEAD --stat -- crates/`: **203 files, +2070/-1232** (identical totals to v14/v15 §2).

## 3. Wiring checklist (on-disk truth, re-verified this lane via numstat)

| Crate | File | State | Diff vs HEAD | Owner worklog | Gate evidence |
|---|---|---|---|---|---|
| server | `crates/server/src/lib.rs` | WIRED-uncommitted | +20/-0 pure additive (unchanged) | SERVER-WIRING-FINAL (45/45) | bL-srv.log 58 ok-lines, 0 FAILED; cap_full 5/5 + ws_full 5/5 in-log |
| sessions | `crates/sessions/src/lib.rs` | WIRED-uncommitted | +3/-0 (unchanged) | SESSIONS-WIRING-FINAL | focused 20; bA-share 10 suites 50/50 |
| foundation | `crates/foundation/src/lib.rs` | WIRED-uncommitted | +5/-2 (unchanged; reorder + `ops_parser_lane`, `repo_cache_store`, `repo_ref` adds) | FOUNDATION-WIRING-FINAL | bH-ops per-suite 5/5 |
| cli | `crates/cli/src/main.rs` | n/a, no edit needed | 0 | CLI-WIRING | `--tests` 23 passed |
| providers | `crates/providers/src/lib.rs` | n/a, no edit (deliberate) | 0 | PROVIDERS-WIRING | bJ-prov lib 51 passed + lanes |
| security | `crates/security/src/lib.rs` | UNPLANNED-uncommitted | 1/1 alpha swap (unchanged) | none | none — attribute or revert |
| tools | `crates/tools/src/lib.rs` | UNPLANNED-uncommitted | 1/1 alpha swap (unchanged) | none | none — attribute or revert |
| storage | `crates/storage/src/lib.rs` | UNPLANNED-uncommitted | fmt-only reflows (unchanged; zero mod add/remove) | none | none — attribute or revert |
| tools | `crates/tools/src/plugin_transform.rs` | GUARD-PORT-landed, uncommitted (unchanged) | +9/-2: unknown-scope guard + invariant docs | EXT-TWINS-DISPOSITION pair-1 | WATCH4 §3 re-verified byte-identical both sides; canonical 5/5 re-run still mandated §6 |
| providers | `crates/providers/src/claude_oauth.rs` | FIX-landed, uncommitted (PROV-017 gate CLEARED) | +1/-1: raw `{LOOPBACK_REDIRECT_URI}` (this lane: `format!` at begin_login, bounds fn adjacent) | PROV-VERIFY4 | 11 suites 56/56 GREEN |
| providers | `crates/providers/src/codex_oauth.rs` | clean impl | `git diff` empty (unchanged) — bounds pins live in NEW test files, impl untouched | PROV-016 bounds/bounds2 addenda + PROV-016-FREEZE | 5/5 + 6/6 + 6/6 = 17/17 (`bE-prov16.log` header; FREEZE doc) |
| tools | `crates/tools/tests/ext_manifest_lane.rs` | fmt-only drift (import reorder) | M 2+/3-, logic untouched | EXT-005 zF pass | bB-ext005 PRE hashes: impl `e54e9647…` identical to zF |

- NEW untracked additive suites (no impl touch, test-owner freeze decision pending):
  `crates/providers/tests/codex_oauth_bounds.rs` (B06..B11, 6 tests),
  `crates/providers/tests/codex_oauth_bounds2.rs` (C12..C17, 6 tests),
  `crates/server/tests/web_capabilities_reason.rs` (3 tests),
  `crates/server/tests/web_capabilities_full.rs` (5 tests),
  `crates/server/tests/web_workspace_reason.rs` (3 tests),
  `crates/server/tests/web_workspace_full.rs` (5 tests).
- TWINS-WATCH4: 0/13 drift vs WATCH3; pair-1 guard PORTED both sides, bodies byte-identical
  (`plugin_transform.rs:128-139` vs `ext_replay_lane.rs:132-143`), droppable by integrator
  (NOT done here). `codex_oauth.rs` impl clean confirmed via FREEZE hashes.

### 3a. SHARE wiring verdict (b-wave close)

- `crates/sessions/src/lib.rs:17-28` wires `share`, `share_audit/count/expiry/invite/list/
  revoke/scope/token`, `share_merge`, `share_policy`, `share_queue` (+ `part_events`,
  `runner`, `tui_info_panel` at :47-49). NOT wired: `share_store`, `share_enterprise`,
  any `*_lane` (`grep -c lane` = 0, WATCH4 §5).
- All 10 lane suites `#[path]`-include their own src (`grep crate::` = 0), so unwired lanes
  stay testable; dropping a twin src breaks only its own test file. `share_policy.rs`
  (non-lane) imports via crate — wired module, expected.
- WATCH4 verdict: keep-both on all 5 SHARE pairs (pairs 9–10 DIVERGENT Lane*-renamed,
  pair 11 COMMENT-ONLY, pair 12 IMPL-DIVERGENT thiserror-vs-manual, pair 13 COMMENT-ONLY).
- Verdict: SHARE wiring COMPLETE as designed — canonical `share_merge/policy/queue` wired,
  lanes deliberately unwired, no shim. No integrator wiring action open on SHARE.

## 4. Per-slice GREEN matrix — this wave (latest receipt per slice wins)

Serial discipline all waves: one cargo cmd at a time, `timeout 120`, `rtk` prefix,
`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`, `--test-threads=1` unless noted.
No cargo runs by THIS lane (read-only rollup). z-wave = temp-stub-restore RED at
rev `248f519` on current worktree bytes (dirty-tree caveat §4h).

| Slice | Wave receipt | GREEN after restore | RED bite shape |
|---|---|---|---|
| AUTO-004 (delegation_lane) | yA + bI/bJ-agents | 5/5 + full crate (bJ lib 15 + lanes) | prior xA: 3p/2f (T02,T03 owner-detach) |
| AUTO-006 (driver_lane) | yA + bI/bJ | 5/5 | prior xA: 4p/1f (T02 non-owner release) |
| AUTO-005 (delegation_gated + tool) | yA + aE | 5/5 + tool PASS/mutated-exit-2; aE rerun 4/4 reproducible | prior xA: 0p/5f (inverted broker) |
| ACP-001/002, WSX-001/002, SDK-001/002, HEAD-001/002, RUN-001 | yB | 64/64, 0 failed | prior xB per-suite bites (v14 §4) |
| EXT-001/002/004/006/008 | yC | 5/5 each (30/30 wave) | prior xC via EXT1C (strong) |
| EXT-005 bytes lane (`ext_manifest_lane`) | xC + zF pass + bB PRE | 5/5; zF RED probe 4/1 + restore-identical; bB PRE hashes match zF bytes | xC: 3p/2f; zF: 4/1 |
| EXT-005 canonical (`plugin_manifest`) | yC | 5/5 struct-shape ONLY — CONTRACT MISMATCH stands, see §4a | none vs card bytes-pipeline |
| EXT-009/010/011/012 | yD | 5/5 each (20/20; 011 on PORTED bytes hash b4c517e3 PASS) | prior xD (strong) |
| TOOL-016..020, UI-019, SYNC-001/002 | yJ + zQ re-confirm | 5/5 each (40/40 wave) | prior xJ (strong) |
| SHARE-001 | zA + yI + bA | behavior-RED 4p/1f + 5/5 restore + lane-mirror 5/5 + yI probe PASS + bA 10-suite 50/50 | none before |
| SHARE-002 | zA + yI + bA | behavior-RED 4p/1f + restore + mirror + probe + bA 50/50 | none before |
| SHARE-003 | xI + yI + aA + bA | 5/5 restore; xI RED 2p/3f; aA RED 2p/3f re-confirm; bA 50/50 | xI/aA temp-stub RED |
| SHARE-004 | xI + yI + aA + bA | 5/5 restore; xI RED 3p/2f; aA RED 3p/2f re-confirm; bA 50/50 | xI/aA temp-stub RED |
| SHARE-005 | xI + yI + aA + bA | 5/5 restore; xI RED 4p/1f; aA RED 3p/2f both lanes re-confirm; bA 50/50 | xI/aA temp-stub RED |
| SHARE total | yI-share/probe + bA-share/bA-probe | 50/50 (10 suites) + TOTAL_LEAK_BYTES=0 | — |
| WEB-001..006 | zB + yK | per-ID RED (5/5,4F,3F,3F,5/5,2/2) + sha-identical restores + GREEN | none before (GREEN-only) |
| WEB-007..012 | zC + yK + bQ | entry-point RED (4/1,2/3,0/5,1/4,0/5,1/4) + 5/5 GREEN + `cmp` identical; bQ re-log | none before (HOLD by design) |
| WEB total | yK + bL-srv | 107 passed + cap_full 5/5 + ws_full 5/5 in bL-srv.log (58 ok-lines, 0 FAILED) | entry-point RED 001–012 (zB+zC) |
| WEB-013 sessions entry-point | s3 + reason addendum | 5/5; +3/3 reason probes GREEN w/ mut-control FAIL-as-expected | 0p/5f (adapter gate) |
| WEB-013 server boundary | uC + **bC full suite** | **5/5 `web_cap_full_t01..t05` GREEN (bL-srv.log)** + mut-control FAIL-as-expected (bC: t01 flip → FAILED `Bool(true) vs false`) | flag-flip FAIL, missing-key FAIL; full card T01–T05 still absent §4c |
| WEB-014 | s3 + SUITES doc | 5/5 (`chat_nav_lane` web014_t01..t05) + 2/2 HTTP boundary | 3p/2f (T01,T04) |
| WEB-015 sessions entry-point | s3 + reason addendum | 5/5 (2/2 frozen T01+T02); +3/3 reason probes GREEN w/ mut-control | 0p/5f (ghost id) |
| WEB-015 server boundary | uC + **bD full suite** | **5/5 `web_015_full_t01..t05` GREEN (bL-srv.log)** + mut-control FAIL-as-expected (bD: t01 flip → FAILED at `web_workspace_full.rs:76`) | 3/3 stub strategies bite; full card T01–T05 still absent §4c |
| WEB-016 | s3 | 5/5 | 1p/4f (T02 neg-path survives) |
| WEB-017 | s3 + SUITES doc | 5/5 (`web_artifact` artifact_t01..t05) + 2/2 HTTP boundary | 1p/4f (T02 gate survives) |
| PROV-015 | yG | 5/5 (56/56 wave) | uD A1/A2/A3 all bite (strong) |
| PROV-016 lifecycle | yG | 5/5 | uD B1/B2 pins |
| PROV-016 boundary | bounds addendum + **bounds2 + PROV-016-FREEZE (bE)** | **17/17** (frozen T01–T05 5/5 + B06..B11 6/6 + C12..C17 6/6); mut-controls FAIL-as-expected (B10-flip 5p/1f, C15-flip 5p/1f) | uD B3/B3alt/B3exp stub classes KILLED ×2 (freeze waiver still needed, §4b) |
| PROV-017 | zD (read-only) + yG | 5/5 GREEN, fix PRESENT (raw const interp) | pre-fix BadConsentUrl history |
| PROV-018..022 | zD + yG | NEW behavior-RED (3F,5F,3F,4F,4F) + 5/5 restore-identical | none before |
| PROV-023/024 | zD + yG | NEW fixture-RED 0/5 each + sha-identical restore | none before |
| OPS-001..009 | zH + aH + bH | 5/5 each; zH per-ID RED rc=101 assertion-only + `cmp` IDENTICAL; bH per-suite 5/5 + hashes match zH pres | uB FINAL epoch (strong, behavior) |
| INT-001/002/003/005/006/007/009/010 | zG + aG + bG | 5/5 each (bG 8 suites 40/40 on identical hashes); zG per-ID RED + byte-identical restore | uA FINAL epoch (strong, behavior) |
| session_turn_stream_api | TURN-STREAM-GATE (3x serial) + yK | 2/2 ×3 runs + 2/2 in yK sweep | n/a (harness race, not behavior RED) |
| EXT002-T05 | EXT002-T05-DETERMINISM | serial 30/30 | flaky-by-construction; NO-FIX |
| EXT twins (8 pairs) | DISPOSITION pair + WATCH4 | 0/13 drift vs WATCH3; pair-1 guard PORTED both sides | pair-1 droppable; pairs 2–8 drop-pending |
| SHARE twins (5 pairs) | DISPOSITION + WATCH4 | survey only, 0 drift | keep-both everywhere |
| REL-001..003 | lane worklogs + **REL-LANEGATE (bF)** | T01 triple GREEN + `cmp` byte-identical determinism + T02/T03 fail-triple spot checks; hashes match (`ab9b350e…`, `79be6ef1…`, `3a462032…`) | lane_gate.py has NO REL coverage (8 PASS modules, 4 UNRUN tests) — validator-only, verifier re-run pending |

### 4a. EXT-005 two-lane status (must-flag, UPDATED b-wave)

- Canonical `plugin_manifest.rs` (55L, WIRED): struct-only contract, yC GREEN 5/5 proves
  struct-shape only, NOT card bytes-pipeline. y-caveat STANDS for the canonical path.
- Bytes lane `ext_manifest_lane.rs` (UNWIRED, `#[path]` test): zF pass proves bytes pipeline
  PRESENT; bB-ext005 PRE hashes match zF bytes exactly (impl `e54e9647…`, test `92701b3f…`).
- Gap-closed: **y** — EXT1-VERIFY4 bytes-pipeline gap closed by ext_manifest_lane
  (EXT-005.md:61). Integrator call stands: wire `ext_manifest_lane` (or port bytes pipeline
  into canonical) + test-owner freeze decision. Until then EXT-005 stays y-caveat in §7
  (count-neutral vs v15).

### 4b. PROV-016 freeze status (bE — UPDATED)

- PROV-016-FREEZE.md pins exact sha256: impl `e16ef548…` (tracked clean), frozen
  `codex_oauth.rs` `aa7459f6…`, bounds `9d2c8d87…` (B06..B11), bounds2 `3595ed6c…`
  (C12..C17). GREEN 17/17 (`bE-prov16.log` header `=== bE-prov16 GREEN ===`).
- Kills uD B3/B3alt/B3exp stub classes twice over (independent second pin; bounds2 does
  not replace bounds). LIFT stands: partial → **y-caveat** (acceptance needs
  controller/test-owner freeze of both additive suites; card T01–T05 frozen text unchanged).
- Waiver request (test-owner/controller): freeze both additive files at pinned hashes,
  record all three suites + verifier-rerun commands in verifier manifest, re-run
  mut-controls at freeze time if policy requires. This lane cannot freeze (ADR-007).

### 4c. WEB-013/015 full-suite lifts (bC/bD — NEW, count-neutral)

- NEW untracked `web_capabilities_full.rs` (5 tests: available-flags, reasons-non-empty,
  unknown-scope, surfaces-pinned, determinism) GREEN 5/5 in `bL-srv.log` + /tmp mut-control
  FAIL-as-expected (`bC-cap-full-mut.log`: t01 flip → FAILED). Frozen
  `web_capabilities_api.rs` never edited.
- NEW untracked `web_workspace_full.rs` (5 tests: catalog-read, missing-catalog, scope-param,
  registry-fields-only, determinism) GREEN 5/5 in `bL-srv.log` + mut-control
  FAIL-as-expected (`bD-ws-full-mut.log`: 4/1, t01 flip → FAILED at `:76`). Frozen
  `web_workspace_api.rs` never edited.
- Lift: boundary pinning upgraded from 3/3 reason probes to 5/5 full boundary suites +
  single-test mut-controls. Card T01–T05 (executor/replay for 013; membership/memory for
  015) still absent per both cards' NOT-ACCEPTED boundary — **partial STANDS** (§7 rows 51/53).

### 4h. Dirty-tree caveat (carried, qualifies z-wave RED)

- zA/zC/zD/zG/zH stubs applied to CURRENT (dirty) bytes, restored byte-identical
  (sha256/cmp ALL_IDENTICAL, zero TEMP markers). RED proves test→impl wiring on current
  content, not pristine-commit validity. Quiesced-tree re-run still required before
  accept-flip for INT/OPS/SHARE y-caveat rows. zB used standalone `rustc --test` (workspace
  `cargo test -p` blocked by unrelated `sessions/share_merge.rs` parse error) + prebuilt
  rlibs; method weaker than cargo, restores sha-identical; re-run under clean cargo
  before accept-flip. b-wave confirms (aG/bG, aH/bH, aA/bA) re-witness GREEN on identical
  hashes but do not lift the dirty-tree qualifier on the RED half.

## 5. RED receipt inventory (which waves cover which IDs)

| Wave file | Covers | Verdict |
|---|---|---|
| `RED-VALIDITY-AUTO4.md` (yA) + bI/bJ | AUTO-004/006/005 | GREEN re-confirm (bJ lib 15 + lanes). RED bite via AUTO3 (xA). MECHANISM CAVEAT persists: tokio REQUIRED NOT MET (PROPOSAL2 unapplied); broker mirror SUFFICES. |
| `AUTO-005-VALIDATION2.md` (aE) | AUTO-005 tool | 4/4 exits+reasons match proposal; PASS bytes identical. Freeze reproducible. |
| `AUTO-TOKIO-PROPOSAL2.md` | AUTO-004 mechanism | Tokio gap spec'd; NOT applied, controller approval pending. |
| `ACPSDK-VERIFY3.md` (yB) | ACP-001/002, WSX-001/002, SDK-001/002, HEAD-001/002, RUN-001 | GREEN 64/64. RED bite via ACPSDK2 (xB, strong). |
| `EXT1-VERIFY4.md` (yC) | EXT-001/002/004/005-canonical/006/008 | GREEN 30/30. RED bite via EXT1C (xC, strong). EXT-005 canonical caveat §4a. |
| `RED-VALIDITY-EXT1C.md` (xC) | EXT-001/002/004/005-bytes/006/008 | Behavior-RED incl. EXT-005 bytes-lane 3/2. |
| `EXT2-VERIFY4.md` (yD) | EXT-009/010/011/012 | GREEN 20/20. RED bite via EXT2C (xD, strong). Guard-port hash PASS. |
| `TOOL-VERIFY4.md` (yJ) + REVERIFY-ZQ | TOOL-016..020, UI-019, SYNC-001/002 | GREEN 40/40, zQ re-confirm 40/40. RED bite via TOOL3 (xJ, strong). |
| `RED-VALIDITY-SHARE12.md` (zA) | SHARE-001/002 | Behavior-RED 4/1 + 4/1, restore-identical, lane-mirrors 5/5. |
| `SHARE-DEDUP3.md` (xI) + `SHARE-DEDUP5.md` (aA) | SHARE-003/004/005 | Temp-stub RED (xI) + independent re-confirm RED (aA: 2/3, 3/2, 3/2+3/2). |
| `SHARE-DEDUP4.md` (yI) + bA-share/bA-probe | SHARE-001..005 | GREEN 50/50 + redaction probe PASS (0 leak bytes); bA re-witness 10-suite 50/50. |
| `RED-VALIDITY-WEB16.md` (zB) | WEB-001..006 | Temp-stub RED + sha-identical restores + GREEN. Method: standalone rustc (§4h). |
| `RED-VALIDITY-WEB712.md` (zC) + bQ | WEB-007..012 | Entry-point RED + 5/5 GREEN + `cmp` identical; bQ re-log. Write-paths disabled. |
| `WEB-VERIFY4.md` (yK) + bL-srv + bM-webnew | WEB-001..017 suites | 107/107 + cap_full 5/5 + ws_full 5/5 (bL-srv 58 ok-lines, 0 FAILED). Browser vitest/tsc unexecuted. |
| `RED-VALIDITY-PROV1724.md` (zD) | PROV-017(read-only)..024 | Per-ID RED (018–022 behavior, 023/024 fixture) + 40/40 GREEN, restores sha-identical. 017 fix-present YES. |
| `PROV-VERIFY4.md` (yG) + bJ-prov | PROV-015..024 | GREEN 56/56 WITH fix present; bJ-prov lib 51 + lanes. |
| `PROV-016.md` bounds/bounds2 + `PROV-016-FREEZE.md` (bE) | PROV-016 boundary | B06..B11 6/6 + C12..C17 6/6 + mut-controls FAIL-as-expected; freeze waiver pending. |
| `WEB-013.md` / `WEB-015.md` full-suite addenda (bC/bD) | WEB-013/015 boundary | 5/5 + 5/5 full boundary suites GREEN + single-test mut FAIL-as-expected. Frozen T01–T05 still absent; uC WEAK-RED stands. |
| `RED-VALIDITY-INT4.md` (zG) + aG + bG | INT-001/002/003/005/006/007/009/010 | Per-ID RED + 5/5 restores to recorded current-tree hashes; bG 40/40 GREEN on identical hashes. Dirty-tree caveat §4h. |
| `INT-VERIFY4.md` (yE) | INT-* (same 8) | GREEN 40/40 on current bytes. |
| `RED-VALIDITY-OPS5.md` (zH) + aH + bH | OPS-001..009 | Per-ID RED rc=101 (assertion-only) + `cmp` IDENTICAL + 5/5 re-GREEN; bH per-suite 5/5, hashes match zH. |
| `OPS-VERIFY4.md` (yF) | OPS-001..009 | GREEN 45/45 + clean `cargo check`. |
| `RED-VALIDITY-WEB-PROV.md` (s3) | WEB-013/014/015/016/017 + PROV-015/016 entry-point | RED-VALID (strong, entry-point). Qualified by uC/uD. |
| `RED-VALIDITY-WEB013-015.md` (uC) | WEB-013/015 server boundary | WEAK-RED (partial pinning). bC/bD extend pinning to full boundary suites, NOT card T01–T05. |
| `RED-VALIDITY-PROV015-016.md` (uD) | PROV-015 (strong) / PROV-016 (lifecycle pins) | Boundary B3-class now killed ×2 by bounds+bounds2 (freeze pending). |
| `REL-LANEGATE.md` (bF) | REL-001/002/003 | T01 triple GREEN + determinism `cmp` PASS + fail-triple spot checks; lane_gate.py has NO REL lanes (8 PASS modules, 4 UNRUN tests). Validator-only; verifier re-run pending. |
| `TURN-STREAM-GATE.md` | session_turn_stream_api | serial-mandate only (§6). |
| Superseded epochs | EXT1/EXT1B, EXT2, AUTO/AUTO2/AUTO3, TOOL/TOOL2, ACPSDK/ACPSDK2, OPS/OPS2/OPS3/OPS4, INT/INT2/INT3, WEB-007-012-ACCEPT | archaeology; latest wave wins on conflict. |

## 6. Serial-test mandates (binding on verifier/CI)

```
# stream binary: env race (TURN-STREAM-GATE §2)
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-server --test session_turn_stream_api -- --test-threads=1
# ext_builtins_lane binary: T05 thread-count race (EXT002-T05-DETERMINISM §2-3)
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-tools --test ext_manifest_lane -- --test-threads=1
# PROV-016 triple (PROV-016-FREEZE.md §verifier-rerun, serial JOBS=1 THREADS=1)
cargo test -p opencode-rk-providers --test codex_oauth -- --test-threads=1
cargo test -p opencode-rk-providers --test codex_oauth_bounds -- --test-threads=1
cargo test -p opencode-rk-providers --test codex_oauth_bounds2 -- --test-threads=1
```

- Stream root cause: process-global env race (unlocked `EnvGuard`, 2 tests same binary).
  3x serial GREEN + yK re-confirm 2/2. No server bug; `TURN_PERMITS` cap not a fix.
- T05 root cause: `/proc/self/task` counts libtest siblings; impl deterministic (zero spawns).
  Parallel stays flaky; serial 30/30 GREEN. Treat parallel line-175 failures as harness noise.
- Do NOT run either binary `--test-threads=N>1`; do NOT parallelize with heavy jobs
  (8 GiB budget, one validation at a time). Do NOT edit frozen tests.
- `env_lock` proposal + T05 options: NOT applied — verifier/test-owner authority.
- Canonical `plugin_transform` 5/5 re-run on ported bytes still mandated before twin drop (§3).
- PROV-016 freeze rerun: hashes must match FREEZE §exact-hashes; lib.rs/ralph.json clean.

## 7. ralph flip table — all 82 non-accepted IDs (EVIDENCE ONLY — do NOT edit)

ready = behavior-RED receipt complete + GREEN on disk (wiring-uncommitted does not
block evidence readiness). `caveat` = acceptance still blocked (see note).
Counts: **ready-y 77 / partial 2 / not-ready 3** (total 82 = 44 in-progress + 38 not-started).
CORR vs v15 header: ralph key is `userStories`, not `stories` — numbers identical.

b-wave lifts vs v15 (ALL COUNT-NEUTRAL — 77 / 2 / 3 unchanged):
- WEB-013/015 full suites (bC/bD): reason-probes 3/3 → full boundary suites 5/5 + 5/5 with
  single-test mut-controls. Stays partial: suites pin boundary flags/reasons/determinism,
  NOT card T01–T05 (013 executor+replay, 015 membership+memory absent per cards).
- PROV-016 bounds2 (bE/FREEZE): second independent B3-kill (C12..C17) + exact-hash freeze
  proposal. Stays y-caveat: both additive suites untracked, freeze waiver pending.
- SHARE bA (10-suite 50/50) + aA RED re-confirm: evidence upgraded, y / y-caveat rows stand.
- INT bG (40/40 identical hashes) + OPS bH (per-suite 5/5, hashes match zH): GREEN re-witness
  on recorded bytes; y-caveat stands (§4h quiesced-tree re-run still required).
- REL bF lanegate: T01 triple + determinism receipts; stays n (validator-only, no RED of
  product behavior, verifier re-run pending; lane_gate.py covers storage lanes only).
- EXT-005 bB: PRE hashes match zF bytes exactly; gap-closed y stands, y-caveat stands
  (canonicalization pending, §4a).

| # | Story | ralph now | ready | Evidence path / note |
|---|---|---|---|---|
| 1 | AUTO-004 | in-progress | y-caveat | yA GREEN 5/5 + full crate (bJ); RED bite via xA 3/2; CAVEAT tokio-mechanism (card amendment or integration lane, PROPOSAL2 unapplied) |
| 2 | AUTO-005 | in-progress | y | yA GREEN 5/5 + tool PASS/mutated-exit-2; aE rerun 4/4 reproducible, PASS bytes identical |
| 3 | AUTO-006 | in-progress | y-caveat | yA GREEN 5/5 (bJ); RED bite via xA 4/1; same tokio-pool caveat as AUTO-004 |
| 4 | EXT-001 | in-progress | y | yC GREEN 5/5; RED bite via xC 4/1 |
| 5 | EXT-002 | in-progress | y | yC GREEN 5/5; RED bite via xC 4/1 |
| 6 | EXT-004 | in-progress | y | yC GREEN 5/5; RED bite via xC 3/2 |
| 7 | EXT-005 | in-progress | y-caveat | BYTES-LANE y (xC 3/2 + zF 4/1 probe + bB PRE hash-match, gap-closed y) BUT canonical `plugin_manifest` struct-shape only — INTEGRATOR canonicalization (§4a) |
| 8 | EXT-006 | in-progress | y | yC GREEN 5/5; RED bite via xC 4/1 |
| 9 | EXT-008 | in-progress | y | yC GREEN 5/5; RED bite via xC 4/1 |
| 10 | EXT-009 | in-progress | y | yD GREEN 5/5; RED bite via xD 0/5 |
| 11 | EXT-010 | in-progress | y | yD GREEN 5/5; RED bite via xD 1/4 |
| 12 | EXT-011 | in-progress | y | yD GREEN 5/5 on PORTED bytes (hash b4c517e3 PASS); RED bite via xD 0/5 |
| 13 | EXT-012 | in-progress | y | yD GREEN 5/5; RED bite via xD 0/5 |
| 14 | INT-001 | in-progress | y-caveat | zG RED 2/3 + yE GREEN 5/5 + bG 5/5 identical hashes; CAVEAT quiesced-tree re-run before accept-flip |
| 15 | INT-002 | in-progress | y-caveat | zG RED 0/5 + yE GREEN 5/5 + bG 5/5; same caveat |
| 16 | INT-003 | in-progress | y-caveat | zG RED 3/2 + yE GREEN 5/5 + bG 5/5; same caveat |
| 17 | INT-005 | in-progress | y-caveat | zG RED 3/2 + yE GREEN 5/5 + bG 5/5; same caveat |
| 18 | INT-006 | in-progress | y-caveat | zG RED 4/1 + yE GREEN 5/5 + bG 5/5; same caveat |
| 19 | INT-007 | in-progress | y-caveat | zG RED 4/1 + yE GREEN 5/5 + bG 5/5; same caveat |
| 20 | INT-009 | in-progress | y-caveat | zG RED 2/3 + yE GREEN 5/5 + bG 5/5; same caveat |
| 21 | INT-010 | in-progress | y-caveat | zG RED 0/5 + yE GREEN 5/5 + bG 5/5; same caveat |
| 22 | OPS-001 | in-progress | y-caveat | zH RED rc=101 assertion-only + `cmp` IDENTICAL + yF GREEN 5/5 + bH 5/5 hash-match; CAVEAT quiesced-tree re-run |
| 23 | OPS-002 | in-progress | y-caveat | zH RED 0/5 + yF GREEN 5/5 + bH 5/5; same caveat |
| 24 | OPS-003 | in-progress | y-caveat | zH RED 2/3 + yF GREEN 5/5 + bH 5/5; same caveat |
| 25 | OPS-004 | in-progress | y-caveat | zH RED 4/1 + yF GREEN 5/5 + bH 5/5; same caveat |
| 26 | OPS-005 | in-progress | y-caveat | zH RED 4/1 + yF GREEN 5/5 + bH 5/5; same caveat |
| 27 | OPS-006 | in-progress | y-caveat | zH RED 4/1 + yF GREEN 5/5 + bH 5/5; same caveat |
| 28 | OPS-007 | in-progress | y-caveat | zH RED 2/3 + yF GREEN 5/5 + bH 5/5; same caveat |
| 29 | OPS-008 | in-progress | y-caveat | zH RED 2/3 + yF GREEN 5/5 + bH 5/5; same caveat |
| 30 | OPS-009 | in-progress | y-caveat | zH RED 2/3 + yF GREEN 5/5 + bH 5/5; same caveat |
| 31 | REL-001 | in-progress | n | validator-only, bF T01 GREEN + determinism PASS, verifier re-run pending |
| 32 | REL-002 | in-progress | n | same as REL-001 |
| 33 | REL-003 | in-progress | n | same as REL-001 |
| 34 | SHARE-001 | in-progress | y | zA behavior-RED 4/1 + restore-identical + lane-mirror 5/5 + yI probe PASS + bA 50/50 |
| 35 | SHARE-002 | in-progress | y | zA behavior-RED 4/1 + restore-identical + lane-mirror 5/5 + yI probe PASS + bA 50/50 |
| 36 | SHARE-003 | in-progress | y-caveat | xI RED 2/3 + aA re-confirm 2/3 + yI 50/50 + bA 50/50; CAVEAT unwired lane + quiesced-tree re-run |
| 37 | SHARE-004 | in-progress | y-caveat | xI RED 3/2 + aA re-confirm 3/2 + yI evidence + bA; same caveat |
| 38 | SHARE-005 | in-progress | y-caveat | xI RED 4/1 + aA re-confirm 3/2×2 + yI evidence + bA; same caveat |
| 39 | WEB-001 | in-progress | y | zB RED 5/5 full-fail + sha-identical restore + GREEN |
| 40 | WEB-002 | in-progress | y | zB RED 4F (T04 valid survival) + restore + GREEN |
| 41 | WEB-003 | in-progress | y | zB RED 3F (T03/T04 valid survival) + restore + GREEN |
| 42 | WEB-004 | in-progress | y | zB RED 3F (T01/T04 valid survival) + restore + GREEN |
| 43 | WEB-005 | in-progress | y | zB RED 5/5 full-fail + restore + GREEN |
| 44 | WEB-006 | in-progress | y | zB RED 2/2 (lock stub, repo file unmodified) + GREEN |
| 45 | WEB-007 | not-started | y-caveat | zC entry-point RED 4/1 + 5/5 GREEN + `cmp` identical (+bQ); CAVEAT write-paths disabled by design + §4h method + browser unexecuted; controller decides flip |
| 46 | WEB-008 | not-started | y-caveat | zC RED 2/3 (T02/T03 valid survival) + GREEN (+bQ); same caveat; controller decides flip |
| 47 | WEB-009 | not-started | y-caveat | zC RED 0/5 + GREEN (+bQ); same caveat; controller decides flip |
| 48 | WEB-010 | not-started | y-caveat | zC RED 1/4 (T03 valid survival) + GREEN (+bQ); same caveat; controller decides flip |
| 49 | WEB-011 | not-started | y-caveat | zC RED 0/5 + GREEN (+bQ); same caveat; controller decides flip |
| 50 | WEB-012 | not-started | y-caveat | zC RED 1/4 (T03 valid survival) + GREEN (+bQ); same caveat; controller decides flip |
| 51 | WEB-013 | not-started | partial | entry-point RED-VALID (s3 0/5) + uC WEAK-RED + reason 3/3 + NEW full boundary suite 5/5 (bC) + mut-control; card T01–T05 still absent; controller decides flip |
| 52 | WEB-014 | not-started | y | entry-point RED-VALID (s3 3/2) + GREEN 5/5 (`chat_nav_lane` full T01–T05 per SUITES doc); controller decides flip |
| 53 | WEB-015 | not-started | partial | entry-point RED-VALID (s3 0/5) + uC WEAK-RED + reason 3/3 + NEW full boundary suite 5/5 (bD) + mut-control; card T01–T05 still absent; controller decides flip |
| 54 | WEB-016 | not-started | y | entry-point RED-VALID (s3 1/4) + GREEN 5/5; controller decides flip |
| 55 | WEB-017 | not-started | y | entry-point RED-VALID (s3 1/4) + GREEN 5/5 (`web_artifact` full T01–T05 per SUITES doc); controller decides flip |
| 56 | PROV-015 | not-started | y | RED-VALID strong (uD A1/A2/A3) + yG GREEN 5/5; controller decides flip |
| 57 | PROV-016 | not-started | y-caveat | lifecycle pins + bounds B06..B11 6/6 + bounds2 C12..C17 6/6 + mut-controls kill B3-class ×2 (§4b); CAVEAT both additive suites untracked, freeze waiver needed; controller decides flip |
| 58 | PROV-017 | not-started | y | zD read-only fix-present + yG GREEN 5/5; controller decides flip |
| 59 | PROV-018 | not-started | y | zD NEW behavior-RED 2/3 + restore-identical + yG GREEN; controller decides flip |
| 60 | PROV-019 | not-started | y | zD NEW behavior-RED 0/5 + restore + GREEN; controller decides flip |
| 61 | PROV-020 | not-started | y | zD NEW behavior-RED 2/3 + restore + GREEN; controller decides flip |
| 62 | PROV-021 | not-started | y | zD NEW behavior-RED 1/4 + restore + GREEN; controller decides flip |
| 63 | PROV-022 | not-started | y | zD NEW behavior-RED 1/4 + restore + GREEN; controller decides flip |
| 64 | PROV-023 | not-started | y | zD NEW fixture-RED 0/5 + sha-identical restore + GREEN; controller decides flip |
| 65 | PROV-024 | not-started | y | zD NEW fixture-RED 0/5 + sha-identical restore + GREEN; controller decides flip |
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
   Server +20, sessions +3, foundation +5/-2 in worktree, zero committed.
   security/tools lib.rs 1/1 swaps + storage fmt-only reflows still no owning lane —
   attribute or revert before the integration commit.
   `plugin_transform` guard-port (+9/-2) verified byte-identical both sides (WATCH4 §3) but
   still needs verifier-logged serial 5/5 re-run, then twin drop same commit.
   Action: serial lane-gate GREEN per suite (§6 mandates), then ONE integration commit.
2. Guard FAIL backlog exhaustion (owner: controller). 122 errors exit 1 + 133 plan
   (122 + 11 unknown prefixes), byte-identical sets vs TRIAGE-16/17 (one line-order swap
   only). No lane edits ralph.json.
3. EXT-005 canonicalization + PROV-016 freeze + WEB-007..012 HOLD release (owner:
   controller + test owner + integrator). Three acceptance decisions, evidence ready:
   (a) EXT-005: wire `ext_manifest_lane` bytes pipeline (y-grade, gap-closed y, bB
   hash-match) or gap-close the card — canonical proves struct-shape only (§4a);
   (b) PROV-016: freeze additive B06..B11 + C12..C17 suites at FREEZE pinned hashes
   (untracked) or re-author as frozen T06–T08+; (c) WEB-007..012: release GREEN-only HOLD
   (entry-point RED now exists + bQ, write-paths disabled by design) and flip not-started →
   in-progress per §7 rows 45–50.
4. Quiesced-tree re-run for INT/OPS/SHARE y-caveat rows + zB cargo-method upgrade
   (owner: INT/OPS/SHARE/WEB lanes + verifier). zA/zC/zD/zG/zH restores byte-identical
   but witnessed on dirty tree; b-wave confirms (bG/bH/bA) re-witness GREEN on identical
   hashes without lifting the RED-half qualifier. zB used standalone rustc (cargo blocked
   by unrelated `share_merge.rs` parse error). Neither slice accept-flippable until
   clean-tree re-run. PROV-017 gate stays CLEARED (no action).
5. AUTO-004 tokio gap + fmt/untracked drift (owner: controller + integrator).
   AUTO-004/006 need card amendment (sync state machine) or tokio integration lane —
   PROPOSAL2 spec'd, NOT applied, controller dep approval either way.
   `cargo fmt --check` 55 → **58** (+3 sibling churn); status 450 → **461**
   (+11 untracked vs v15: worklogs + 6 additive suites — bounds, bounds2, 2×reason, 2×full).
   New untracked suites (bounds, bounds2, reason ×2, full ×2) need freeze-or-drop decision
   before the integration commit. WEB-013/015 full suites stay partial — they pin the
   boundary, not the card contract (§4c).

Merge order: (i) attribute/revert §3 unattributed lib.rs drift; (ii) verifier-logged
serial `plugin_transform` 5/5 on ported bytes → drop `ext_replay_lane` same commit;
(iii) freeze-or-drop the 6 additive suites (bounds, bounds2, 2×reason, 2×full);
(iv) EXT-005 canonicalization (§4a); (v) quiesce INT/OPS/SHARE owned paths + frozen tests
→ INT/OPS/SHARE + zB-cargo re-run (§4h); (vi) lane gates GREEN per wired suite (serial
mandates §6, incl. PROV-016 triple); (vii) refresh stale status files; (viii) ONE
integration commit (tracked fixes + lib.rs wirings incl. PROV-017 one-liner, fmt at
commit); (ix) untracked modules only with passing gates; (x) controller syncs
FEATURES.md/accounting + flips per §7.
