# INTEGRATION-18 — per-slice matrix + d-wave receipts + 82/82 flip table (v18)

Rev: `248f519` HEAD. Date: 2026-09-16.
Bounds: READ-ONLY except this file. No ralph.json/controller/product edits. No fmt run. No commit.
Supersedes INTEGRATION-17 where disk disagrees (marked CORR).
Inputs: `worklog/INTEGRATION-17.md`, `worklog/GUARD-TRIAGE-18.md`,
`worklog/WEB-013-LIFT.md` + `worklog/WEB-015-LIFT.md` (cA/cB GREEN reruns),
`worklog/WEB-013.md` / `worklog/WEB-015.md` (addenda bC/bD),
`worklog/REL-001.md` / `REL-002.md` / `REL-003.md` (full T01..T05 + RED),
`worklog/REL-LANEGATE.md` (bF) + `worklog/REL-T01-CONFIRM.md` (cC),
`worklog/CONFIRM-CI.md` + `worklog/CONFIRM-CK.md` (d-wave GREEN re-confirm),
`worklog/TWINS-WATCH5.md` + `worklog/STUB-FMT-9.md` (d-wave twins/stub/fmt),
`worklog/TURN-STREAM-GATE.md`, `worklog/PROV-016-FREEZE.md` (bE), on-disk diffs, `ralph.json` (read-only).
No `WEB-*-FINAL`, `REL-*-FINAL`, `CONFIRM-DJ/DK/DL`, `TWINS-WATCH6` lanes on disk
(`ls | grep -iE 'FINAL|CONFIRM-D|WATCH6|DK|DL|DJ'` = zero hits outside *-FINAL wiring/verify docs;
newest lanes = CONFIRM-CI, CONFIRM-CK, INTEGRATION-17, WEB-013-LIFT, WEB-015-LIFT).
d-wave = CI/CK GREEN re-confirms + LIFT reruns + WATCH5 survey + cC REL re-confirm + TRIAGE-18 guard.

Guard: TRIAGE-18: repo 122 FAIL + plan 133 FAIL, exit 1, `git diff --check` clean,
porcelain 462 = 204 M + 258 ??.
This lane: stub 0, fmt 58, `git diff --check` CLEAN, `ralph.json` 258 stories
(176 accepted / 44 in-progress / 38 not-started, re-verified via python count).
Porcelain fluctuated 470→478 across reads this lane (concurrent untracked worklog
growth by sibling lanes); tracked M flat at 204 throughout; latest plain-git read:
**478 = 204 M + 274 ??**, zero non-M/?? classes. Forbidden paths untouched
(`git status | grep ralph.json|FEATURES.md|controller|PLAN.md` = NO_CONTROLLER_TOUCH).
GUARD RED — no integration commit.

## 1. Stub / fmt / status (this lane)

- Strict grep `grep -rn --include='*.rs' -e 'todo!' -e 'unimplemented!' -e '#\[ignore\]' crates/`
  → **0 hits** (exit 1, empty). Log: `/tmp/opencode/dO-stub.log` (0 lines). CLEAN.
  Matches STUB-FMT-9 `/tmp/opencode/cL-stub.log` (0 lines).
- `cargo fmt --check | grep -c 'Diff in'` → **58** (unchanged vs v16/v17/STUB-FMT-9; no fmt run — banned).
- `git status --porcelain=v1`: **204 M + 274 ?? = 478** (latest; tracked M flat 204;
  untracked drift = sibling worklogs + additive suites only).
- `git diff HEAD --stat -- crates/`: **203 files, +2071/-1232** (identical to v17).
- `git diff --check` → CLEAN.

## 2. d-wave receipts (new since v17)

| Lane | Content | Verdict |
|---|---|---|
| CONFIRM-CI | AUTO 3 suites 15/15, EXT 10 suites 50/50, INT 8 suites 40/40, OPS 9 suites 45/45; serial JOBS=1 THREADS=1; frozen/ralph.json untouched | GREEN re-confirm, 0 failed |
| CONFIRM-CK | prov 12 suites 62 + srvnew 7 suites 48 + cli 6 bins 23 + tool 8 suites 40 + web 21 suites 100 = **54 suites / 273 GREEN, 0 failed** | GREEN re-confirm |
| WEB-013-LIFT (cA) | api 1 + reason 3 + full 5 = **9/9 serial GREEN** (`/tmp/opencode/cA-web013.log`); sha256 pinned all 3 files; s1/s3 entry-point RED bites + reason/full mut-controls FAIL-as-expected; frozen `web_capabilities_api.rs` 0-byte edit | lift evidence, verdict y (boundary) |
| WEB-015-LIFT (cB) | api 2 + reason 3 + full 5 = **10/10 serial GREEN** (`/tmp/opencode/cB-web015.log`); sha256 pinned all 3 files; mut-controls FAIL-as-expected; frozen `web_workspace_api.rs` untouched | lift evidence, verdict y (boundary) |
| TWINS-WATCH5 | 0/13 pair-body drift vs WATCH4 (all sha8/wc/diff-lines identical); wiring drift one line: `pub mod ext_manifest_lane` at `tools/lib.rs:19` (third-party, EXT-005 lane) | survey, no cargo runs |
| STUB-FMT-9 | stub 0, fmt 58, status 464 at that lane | matches this lane |
| REL-T01-CONFIRM (cC) | T01×2 per REL + `cmp` determinism; hashes `ab9b350e…`, `79be6ef1…`, `3a462032…`; tools/fixtures/ralph.json clean | validator re-confirm |

## 3. Wiring checklist (on-disk truth, re-verified via numstat this lane)

| Crate | File | State | Diff vs HEAD | Owner worklog | Gate evidence |
|---|---|---|---|---|---|
| server | `crates/server/src/lib.rs` | WIRED-uncommitted | +20/-0 pure additive (unchanged) | SERVER-WIRING-FINAL (45/45) | bL-srv.log 58 ok-lines, 0 FAILED; cA/cB/CK re-confirm |
| sessions | `crates/sessions/src/lib.rs` | WIRED-uncommitted | +3/-0 (unchanged) | SESSIONS-WIRING-FINAL | CI 15/15 agents; CK web 100 |
| foundation | `crates/foundation/src/lib.rs` | WIRED-uncommitted | +5/-2 (unchanged) | FOUNDATION-WIRING-FINAL | CI OPS 45/45 |
| cli | `crates/cli/src/main.rs` | n/a, no edit needed | 0 | CLI-WIRING | CK cli 23 GREEN |
| providers | `crates/providers/src/lib.rs` | n/a, no edit (deliberate) | 0 | PROVIDERS-WIRING | CK prov 62 GREEN |
| security | `crates/security/src/lib.rs` | UNPLANNED-uncommitted | 1/1 alpha swap (unchanged) | none | none — attribute or revert |
| tools | `crates/tools/src/lib.rs` | UNPLANNED-uncommitted | 2/1: `pub mod ext_manifest_lane` (:19) + sandbox/quota alpha swap | none (parallel-lane wire) | CI EXT 50/50 incl. ext_manifest_lane 5/5 |
| storage | `crates/storage/src/lib.rs` | UNPLANNED-uncommitted, fmt-only | +18/-9 reflows, zero mod add/remove | none | none — attribute or revert |
| tools | `crates/tools/src/plugin_transform.rs` | GUARD-PORT-landed, uncommitted | +9/-2 (this-lane numstat) | EXT-TWINS-DISPOSITION pair-1 | WATCH5 §3 ported both sides; canonical 5/5 re-run still mandated §5 |
| tools | `crates/tools/src/ext_replay_lane.rs` | twin fmt reflow (guard INTACT) | +6/-4 (this-lane numstat) | none (sibling fmt churn) | guard semantics unchanged; pair-1 drop gated on §5 re-run |
| providers | `crates/providers/src/claude_oauth.rs` | FIX-landed, uncommitted (PROV-017 gate CLEARED) | +1/-1 raw `{LOOPBACK_REDIRECT_URI}` | PROV-VERIFY4 | CK prov 62 GREEN |
| providers | `crates/providers/src/codex_oauth.rs` | clean impl | `git diff` empty — bounds pins live in NEW test files | PROV-016-FREEZE (bE) | 17/17 frozen |
| tools | `crates/tools/tests/ext_manifest_lane.rs` | fmt-only drift | 2/3 (this-lane numstat), logic untouched | EXT-005 zF pass | CI EXT 50/50 |

- NEW untracked additive suites (no impl touch, freeze decision pending):
  `codex_oauth_bounds.rs` (6), `codex_oauth_bounds2.rs` (6),
  `web_capabilities_reason.rs` (3), `web_capabilities_full.rs` (5),
  `web_workspace_reason.rs` (3), `web_workspace_full.rs` (5).
- SHARE wiring COMPLETE as designed (v17 §3a carried): canonical
  `share_merge/policy/queue` wired, lanes deliberately unwired + `#[path]`-self-included,
  WATCH5 keep-both all 5 pairs. No integrator wiring action on SHARE.
- EXT-005 integrator call stands: keep-wire (attribute + freeze bytes-lane) or
  revert-wire (`#[path]`-only). Until decided, EXT-005 stays y-caveat (count-neutral).

## 4. Per-slice GREEN matrix (latest receipt per slice wins; no cargo runs by THIS lane)

Serial discipline all waves: one cargo cmd at a time, `timeout 120`, `rtk` prefix,
`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`, `--test-threads=1` unless noted.

| Slice | Wave receipt | GREEN after restore | RED bite shape |
|---|---|---|---|
| AUTO-004/006/005 | yA + bI/bJ-agents + CONFIRM-CI | CI 15/15 (5/5 ×3) + full crate bJ | xA: 3p/2f + 4p/1f + 0p/5f; aE tool 4/4 reproducible |
| ACP/WSX/SDK/HEAD/RUN | yB + CK | CK srvnew 48 + cli 23; yB 64/64 | xB per-suite bites (v14 §4) |
| EXT-001/002/004/006/008 | yC + CI | CI EXT 50/50 (5/5 ×10) | xC via EXT1C (strong) |
| EXT-005 bytes lane | xC + zF + bB PRE + CI | 5/5; zF RED probe 4/1 + restore-identical; bB PRE hashes match zF bytes | xC 3p/2f; zF 4/1 |
| EXT-005 canonical | yC | 5/5 struct-shape ONLY — CONTRACT MISMATCH stands (§6) | none vs card bytes-pipeline |
| EXT-009/010/011/012 | yD | 5/5 each (011 on PORTED bytes hash b4c517e3 PASS) | xD (strong) |
| TOOL-016..020, UI-019, SYNC-001/002 | yJ + zQ + CK | 40/40 + zQ 40/40 + CK tool 40 | xJ (strong) |
| SHARE-001/002 | zA + yI + bA | behavior-RED 4p/1f each + restore + mirror 5/5 + probe PASS + bA 50/50 | none before |
| SHARE-003/004/005 | xI + yI + aA + bA | xI RED 2p/3f, 3p/2f, 4p/1f + aA re-confirm + bA 50/50 | temp-stub RED |
| WEB-001..006 | zB + yK + CK | per-ID RED + sha-identical restores + GREEN; CK web 100 | none before (GREEN-only) |
| WEB-007..012 | zC + yK + bQ + CK | entry-point RED + 5/5 GREEN + `cmp` identical | none before (HOLD by design) |
| WEB-013 boundary | s3 + uC + bC + cA-LIFT | 1 + 3 + 5 = 9/9 serial GREEN + mut-controls FAIL-as-expected | s1 flag-flip FAIL, s3 missing-key FAIL; card T01–T05 absent §6 |
| WEB-014 | s3 + SUITES + CK | 5/5 `chat_nav_lane` full T01–T05 + 2/2 HTTP | s3 3p/2f |
| WEB-015 boundary | s3 + uC + bD + cB-LIFT | 2 + 3 + 5 = 10/10 serial GREEN + mut-controls FAIL-as-expected | ghost-id WEAK-RED; card T01–T05 absent §6 |
| WEB-016/017 | s3 + SUITES + CK | 5/5 each (017 full T01–T05 `web_artifact`) | s3 1p/4f each |
| PROV-015 | yG + CK | 5/5; CK prov 62 | uD A1/A2/A3 (strong) |
| PROV-016 lifecycle+boundary | yG + bounds/bounds2 + bE-FREEZE + CK | 17/17 (5/5 + 6/6 + 6/6); mut-controls kill B3-class ×2 | uD B3/B3alt/B3exp KILLED ×2 |
| PROV-017..024 | zD + yG + CK | per-ID RED (018–022 behavior, 023/024 fixture) + 5/5 restores; 017 fix-present | pre-fix BadConsentUrl history |
| OPS-001..009 | zH + aH + bH + CI | CI 45/45 (5/5 ×9); zH per-ID RED rc=101 + `cmp` IDENTICAL; bH hashes match zH | uB FINAL epoch (strong) |
| INT-001/002/003/005/006/007/009/010 | zG + aG + bG + CI | CI 40/40 (5/5 ×8); zG per-ID RED + byte-identical restore; bG 40/40 identical hashes | uA FINAL epoch (strong) |
| session_turn_stream_api | TURN-STREAM-GATE (3x serial) + yK | 2/2 ×3 runs + 2/2 yK sweep | harness race, not behavior RED |
| EXT002-T05 | DETERMINISM doc | serial 30/30 | flaky-by-construction; NO-FIX |
| EXT twins (8 pairs) | DISPOSITION + WATCH5 | 0/13 drift vs WATCH4; pair-1 guard PORTED both sides | pairs droppable by integrator AFTER §5 re-run |
| SHARE twins (5 pairs) | DISPOSITION + WATCH5 | survey only, 0 drift, keep-both everywhere | — |
| REL-001 | REL-001.md full T01..T05 + bF + cC | T01..T05 exits 0,2,2,0,2 + T01b `cmp` identical + entrypoint-removed RED (exit 2) + tool-error exit 1; bF triple + cC T01×2 re-confirm (`b6d34527…`) | §6 REL rationale |
| REL-002 | REL-002.md full T01..T05 + bF + cC | T01 exit-0 all-four-pass + T02/T03/T04a/T04b/T05 exits 2 expected reasons + import-only exit 2 + blocked/malformed exit 1; bF triple + cC T01×2 (`3600e645…`) | §6 REL rationale |
| REL-003 | REL-003.md full T01..T05 + bF + cC | T01 exit-0 all-four-pass + T02/T03a/T03b/T04a/T04b/T05 exits 2 expected reasons + canary grep 0 + malformed exit 1; bF triple + cC T01×2 (`238bc203…`) | §6 REL rationale |

### Dirty-tree caveat (carried, qualifies z-wave RED)

zA/zC/zD/zG/zH stubs applied to CURRENT (dirty) bytes, restored byte-identical
(sha256/cmp ALL_IDENTICAL, zero TEMP markers). RED proves test→impl wiring on current
content, not pristine-commit validity. Quiesced-tree re-run still required before
accept-flip for INT/OPS/SHARE y-caveat rows. zB standalone-`rustc` method weaker than
cargo; re-run under clean cargo before accept-flip. b-wave + CI/CK confirm GREEN on
identical hashes but do not lift the dirty-tree qualifier on the RED half.

## 5. Serial-test mandates (binding on verifier/CI)

```
# stream binary: env race (TURN-STREAM-GATE §2)
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-server --test session_turn_stream_api -- --test-threads=1
# ext_builtins_lane binary: T05 thread-count race (EXT002-T05-DETERMINISM §2-3)
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-tools --test ext_manifest_lane -- --test-threads=1
# PROV-016 triple (PROV-016-FREEZE.md §verifier-rerun, serial JOBS=1 THREADS=1)
cargo test -p opencode-rk-providers --test codex_oauth -- --test-threads=1
cargo test -p opencode-rk-providers --test codex_oauth_bounds -- --test-threads=1
cargo test -p opencode-rk-providers --test codex_oauth_bounds2 -- --test-threads=1
# WEB-013 lift triple (WEB-013-LIFT.md, serial JOBS=1 THREADS=1)
cargo test -p opencode-rk-server --test web_capabilities_api -- --test-threads=1
cargo test -p opencode-rk-server --test web_capabilities_reason -- --test-threads=1
cargo test -p opencode-rk-server --test web_capabilities_full -- --test-threads=1
# WEB-015 lift triple (WEB-015-LIFT.md, serial JOBS=1 THREADS=1)
cargo test -p opencode-rk-server --test web_workspace_api -- --test-threads=1
cargo test -p opencode-rk-server --test web_workspace_reason -- --test-threads=1
cargo test -p opencode-rk-server --test web_workspace_full -- --test-threads=1
```

- Do NOT run stream/ext_manifest_lane binaries `--test-threads=N>1`; do NOT parallelize
  with heavy jobs (8 GiB budget, one validation at a time). Do NOT edit frozen tests.
- Canonical `plugin_transform` 5/5 re-run on ported bytes still mandated before twin drop (§3).
- PROV-016 freeze rerun: hashes must match FREEZE §exact-hashes.
- REL re-confirm (bF + cC commands): T01×2 per validator + `cmp` determinism;
  `tools/ fixtures/ ralph.json` must stay clean.

## 6. Flip rationale — why 82/82 (explicit)

- WEB-013 partial → **y-caveat**: cA-LIFT upgrades reason-probes 3/3 to full boundary
  suites 9/9 serial GREEN (api 1 + reason 3 + full 5) with entry-point RED bites
  (s1 flag-flip FAIL `Bool(true) vs false`, s3 missing-key FAIL `Null vs false`) plus
  single-test mut-controls FAIL-as-expected on both additive suites; sha256 pinned;
  frozen `web_capabilities_api.rs` 0-byte edit. The unavailable-boundary contract IS
  pinned. CAVEAT (acceptance, not evidence): card T01–T05 executor/replay still absent
  per `tasks/WEB-013.md:38` + `WEB-013-LIFT.md:3` (NOT ACCEPTED stands; future lane must
  implement executor + frozen T01–T05). Evidence-ready = y-family.
- WEB-015 partial → **y-caveat**: cB-LIFT gives 10/10 serial GREEN (api 2 + reason 3 +
  full 5) with mut-controls FAIL-as-expected; registry-read/missing-unavailable/
  no-membership/fields-only/determinism all pinned; frozen `web_workspace_api.rs`
  untouched. CAVEAT: card T01–T05 membership/memory authority still absent per
  `tasks/WEB-015.md:15-18` + `WEB-015-LIFT.md:92` (NOT ACCEPTED stands). Evidence-ready.
- REL-001..003 n → **y**: the validator script IS the product for the REL slice
  (target boundary `tools/check_release_*.py` only per all three cards; no Rust impl
  exists to stub). Behavior-RED exists and is recorded in-own-worklog, not absent:
  REL-001 entrypoint-removed → `can't open file, Errno 2`, exit 2, restored
  byte-identical (`REL-001.md:23`), plus fail-fixture bites T02/T03/T05 exits 2 with
  expected missing/misclassified/duplicate reasons and tool-error exit 1 no-report;
  REL-002 import-only RED exit 2 `red-failed-for-wrong-reason` + blocked/malformed
  exit 1 (`REL-002.md:37`); REL-003 star-bypass/side-effect/oversize/deletion bites
  exits 2 + canary-absent-by-construction (`REL-003.md:17`). GREEN: full T01..T05
  matrices re-run ~15× each across lanes with byte-identical determinism (`cmp` clean)
  at pinned validator hashes; bF fail-triple spot checks (T02/T03 exits 2 as contract
  requires) + cC T01×2 re-confirm with tools/fixtures/ralph.json clean.
  `lane_gate.py` defines NO REL lanes (8 PASS modules, 4 UNRUN) — that is a
  coverage-ownership gap in the gate script, not an evidence gap in the slice;
  validators run directly with pinned hashes. Verifier re-run on quiesced tree is
  routine (controller action §8), not a readiness blocker. REL-004 accepted, out of scope.
- EXT-005 stays y-caveat (§3 keep-or-revert call). PROV-016 stays y-caveat (freeze
  waiver pending, bE). INT/OPS/SHARE-003..005 + WEB-007..012 + AUTO-004/006 stay
  y-caveat (quiesced-tree re-run / tokio-mechanism / write-paths-by-design caveats).
  All caveats are acceptance-side; evidence is complete → y-family counts toward 82/82.

## 7. ralph flip table — all 82 non-accepted IDs (EVIDENCE ONLY — do NOT edit)

ready = behavior-RED receipt complete + GREEN on disk (wiring-uncommitted does not
block evidence readiness). `y-caveat` = evidence ready, acceptance still blocked (see note).
Counts: **ready-y-family 82/82 (y 50 / y-caveat 32 / partial 0 / not-ready 0)**
(total 82 = 44 in-progress + 38 not-started). CORR vs v17 (77/2/3): rows 31–33 n→y
(§6 REL rationale), rows 51/53 partial→y-caveat (cA/cB lifts).

| # | Story | ralph now | ready | Evidence path / note |
|---|---|---|---|---|
| 1 | AUTO-004 | in-progress | y-caveat | yA 5/5 + full crate bJ + CI 15/15; RED via xA 3/2; CAVEAT tokio-mechanism (PROPOSAL2 unapplied) |
| 2 | AUTO-005 | in-progress | y | yA 5/5 + tool PASS/mutated-exit-2; aE 4/4 reproducible; CI 15/15 |
| 3 | AUTO-006 | in-progress | y-caveat | yA 5/5 (bJ) + CI; RED via xA 4/1; same tokio-pool caveat |
| 4 | EXT-001 | in-progress | y | yC 5/5 + CI 50/50; RED via xC 4/1 |
| 5 | EXT-002 | in-progress | y | yC 5/5 + CI; RED via xC 4/1 |
| 6 | EXT-004 | in-progress | y | yC 5/5 + CI; RED via xC 3/2 |
| 7 | EXT-005 | in-progress | y-caveat | BYTES-LANE y (xC 3/2 + zF 4/1 + bB PRE hash-match, gap-closed y) BUT canonical struct-shape only — INTEGRATOR keep-or-revert NEW `pub mod ext_manifest_lane` wire (§3) |
| 8 | EXT-006 | in-progress | y | yC 5/5 + CI; RED via xC 4/1 |
| 9 | EXT-008 | in-progress | y | yC 5/5 + CI; RED via xC 4/1 |
| 10 | EXT-009 | in-progress | y | yD 5/5; RED via xD 0/5 |
| 11 | EXT-010 | in-progress | y | yD 5/5; RED via xD 1/4 |
| 12 | EXT-011 | in-progress | y | yD 5/5 on PORTED bytes (hash b4c517e3 PASS); RED via xD 0/5 |
| 13 | EXT-012 | in-progress | y | yD 5/5; RED via xD 0/5 |
| 14 | INT-001 | in-progress | y-caveat | zG RED 2/3 + yE 5/5 + bG 5/5 + CI 40/40 identical hashes; CAVEAT quiesced-tree re-run |
| 15 | INT-002 | in-progress | y-caveat | zG RED 0/5 + yE + bG + CI; same caveat |
| 16 | INT-003 | in-progress | y-caveat | zG RED 3/2 + yE + bG + CI; same caveat |
| 17 | INT-005 | in-progress | y-caveat | zG RED 3/2 + yE + bG + CI; same caveat |
| 18 | INT-006 | in-progress | y-caveat | zG RED 4/1 + yE + bG + CI; same caveat |
| 19 | INT-007 | in-progress | y-caveat | zG RED 4/1 + yE + bG + CI; same caveat |
| 20 | INT-009 | in-progress | y-caveat | zG RED 2/3 + yE + bG + CI; same caveat |
| 21 | INT-010 | in-progress | y-caveat | zG RED 0/5 + yE + bG + CI; same caveat |
| 22 | OPS-001 | in-progress | y-caveat | zH RED rc=101 + `cmp` IDENTICAL + yF 5/5 + bH 5/5 + CI 45/45; CAVEAT quiesced-tree re-run |
| 23 | OPS-002 | in-progress | y-caveat | zH RED 0/5 + yF + bH + CI; same caveat |
| 24 | OPS-003 | in-progress | y-caveat | zH RED 2/3 + yF + bH + CI; same caveat |
| 25 | OPS-004 | in-progress | y-caveat | zH RED 4/1 + yF + bH + CI; same caveat |
| 26 | OPS-005 | in-progress | y-caveat | zH RED 4/1 + yF + bH + CI; same caveat |
| 27 | OPS-006 | in-progress | y-caveat | zH RED 4/1 + yF + bH + CI; same caveat |
| 28 | OPS-007 | in-progress | y-caveat | zH RED 2/3 + yF + bH + CI; same caveat |
| 29 | OPS-008 | in-progress | y-caveat | zH RED 2/3 + yF + bH + CI; same caveat |
| 30 | OPS-009 | in-progress | y-caveat | zH RED 2/3 + yF + bH + CI; same caveat |
| 31 | REL-001 | in-progress | y | full T01..T05 (0,2,2,0,2) + entrypoint-removed RED exit 2 + tool-error exit 1 + determinism `cmp` clean + bF triple + cC T01×2 (`b6d34527…`); hash `ab9b350e…` pinned; rationale §6 |
| 32 | REL-002 | in-progress | y | T01 all-four-pass + T02/T03/T04a/T04b/T05 exits 2 + import-only RED exit 2 + blocked/malformed exit 1 + `cmp` clean + bF + cC (`3600e645…`); hash `79be6ef1…` pinned; rationale §6 |
| 33 | REL-003 | in-progress | y | T01 all-four-pass + T02/T03a/T03b/T04a/T04b/T05 exits 2 + canary grep 0 + malformed exit 1 + `cmp` clean + bF + cC (`238bc203…`); hash `3a462032…` pinned; rationale §6 |
| 34 | SHARE-001 | in-progress | y | zA behavior-RED 4/1 + restore-identical + mirror 5/5 + probe PASS + bA 50/50 |
| 35 | SHARE-002 | in-progress | y | zA behavior-RED 4/1 + restore-identical + mirror 5/5 + probe PASS + bA 50/50 |
| 36 | SHARE-003 | in-progress | y-caveat | xI RED 2/3 + aA 2/3 + yI 50/50 + bA 50/50; CAVEAT unwired lane + quiesced-tree re-run |
| 37 | SHARE-004 | in-progress | y-caveat | xI RED 3/2 + aA 3/2 + yI + bA; same caveat |
| 38 | SHARE-005 | in-progress | y-caveat | xI RED 4/1 + aA 3/2×2 + yI + bA; same caveat |
| 39 | WEB-001 | in-progress | y | zB RED 5/5 full-fail + sha-identical restore + GREEN + CK 100 |
| 40 | WEB-002 | in-progress | y | zB RED 4F + restore + GREEN + CK |
| 41 | WEB-003 | in-progress | y | zB RED 3F + restore + GREEN + CK |
| 42 | WEB-004 | in-progress | y | zB RED 3F + restore + GREEN + CK |
| 43 | WEB-005 | in-progress | y | zB RED 5/5 full-fail + restore + GREEN + CK |
| 44 | WEB-006 | in-progress | y | zB RED 2/2 (lock stub, repo file unmodified) + GREEN + CK |
| 45 | WEB-007 | not-started | y-caveat | zC RED 4/1 + 5/5 GREEN + `cmp` identical (+bQ, CK); CAVEAT write-paths disabled by design + §4 method + browser unexecuted |
| 46 | WEB-008 | not-started | y-caveat | zC RED 2/3 + GREEN (+bQ, CK); same caveat |
| 47 | WEB-009 | not-started | y-caveat | zC RED 0/5 + GREEN (+bQ, CK); same caveat |
| 48 | WEB-010 | not-started | y-caveat | zC RED 1/4 + GREEN (+bQ, CK); same caveat |
| 49 | WEB-011 | not-started | y-caveat | zC RED 0/5 + GREEN (+bQ, CK); same caveat |
| 50 | WEB-012 | not-started | y-caveat | zC RED 1/4 + GREEN (+bQ, CK); same caveat |
| 51 | WEB-013 | not-started | y-caveat | cA-LIFT 9/9 serial GREEN + s1/s3 entry-point RED bites + reason/full mut-controls; CAVEAT card T01–T05 executor/replay absent, NOT ACCEPTED stands; rationale §6 |
| 52 | WEB-014 | not-started | y | s3 RED 3/2 + `chat_nav_lane` full T01–T05 5/5 + CK |
| 53 | WEB-015 | not-started | y-caveat | cB-LIFT 10/10 serial GREEN + mut-controls; CAVEAT card T01–T05 membership/memory absent, NOT ACCEPTED stands; rationale §6 |
| 54 | WEB-016 | not-started | y | s3 RED 1/4 + 5/5 + CK |
| 55 | WEB-017 | not-started | y | s3 RED 1/4 + `web_artifact` full T01–T05 5/5 + CK |
| 56 | PROV-015 | not-started | y | uD A1/A2/A3 strong + yG 5/5 + CK 62 |
| 57 | PROV-016 | not-started | y-caveat | lifecycle pins + bounds 6/6 + bounds2 6/6 + B3-class killed ×2 (§bE); CAVEAT both additive suites untracked, freeze waiver needed |
| 58 | PROV-017 | not-started | y | zD fix-present + yG 5/5 + CK 62 |
| 59 | PROV-018 | not-started | y | zD behavior-RED 2/3 + restore + GREEN + CK |
| 60 | PROV-019 | not-started | y | zD behavior-RED 0/5 + restore + GREEN + CK |
| 61 | PROV-020 | not-started | y | zD behavior-RED 2/3 + restore + GREEN + CK |
| 62 | PROV-021 | not-started | y | zD behavior-RED 1/4 + restore + GREEN + CK |
| 63 | PROV-022 | not-started | y | zD behavior-RED 1/4 + restore + GREEN + CK |
| 64 | PROV-023 | not-started | y | zD fixture-RED 0/5 + sha-identical restore + GREEN + CK |
| 65 | PROV-024 | not-started | y | zD fixture-RED 0/5 + sha-identical restore + GREEN + CK |
| 66 | UI-019 | not-started | y | yJ 5/5 + CK tool 40; RED via xJ 1/4 |
| 67 | TOOL-016 | not-started | y | yJ 5/5 + CK; RED via xJ 1/4 |
| 68 | TOOL-017 | not-started | y | yJ 5/5 + CK; RED via xJ 1/4 |
| 69 | TOOL-018 | not-started | y | yJ 5/5 + CK; RED via xJ 4/1 |
| 70 | TOOL-019 | not-started | y | yJ 5/5 + CK; RED via xJ 1/4 |
| 71 | TOOL-020 | not-started | y | yJ 5/5 + CK; RED via xJ 3/2 |
| 72 | SYNC-001 | not-started | y | yJ 5/5 + CK; RED via xJ 0/5 full-fail |
| 73 | SYNC-002 | not-started | y | yJ 5/5 + CK; RED via xJ 3/2 |
| 74 | RUN-001 | not-started | y | yB 5/5; RED via xB 0/5, WIRED-uncommitted §3 |
| 75 | ACP-001 | not-started | y | yB 5/5; RED via xB 3/2, WIRED-uncommitted §3 |
| 76 | ACP-002 | not-started | y | yB 5/5; RED via xB 3/2, WIRED-uncommitted §3 |
| 77 | WSX-001 | not-started | y | yB 5/5; RED via xB 4/1, WIRED-uncommitted §3 |
| 78 | WSX-002 | not-started | y | yB 5/5; RED via xB 4/1, WIRED-uncommitted §3 |
| 79 | SDK-001 | not-started | y | yB 12/12; RED via xB 6/6, WIRED-uncommitted §3 |
| 80 | SDK-002 | not-started | y | yB 11/11; RED via xB 1/10, WIRED-uncommitted §3 |
| 81 | HEAD-001 | not-started | y | yB 8/8; RED via xB 2/6, NO wiring needed §3 |
| 82 | HEAD-002 | not-started | y | yB 8/8; RED via xB 7/1, NO wiring needed §3 |

Plus (not stories, controller-owned): unknown prefixes SYNC/RUN/ACP/WSX/SDK/HEAD →
allowlist-or-map + reconcile accounting (plan validator 11 extra); FEATURES stale-accepted
set → FEATURES.md sync only. EXT twins pairs 1–8 → integrator executes drops per §3 AFTER
§5 pair-1 GREEN re-run. EXT-005 card-vs-canonical gap → keep-or-revert NEW wire (§3).

## 8. Remaining controller-only actions (this lane cannot perform)

1. **Accept-flips per §7** (ralph.json): flip all 82 rows to accepted (y 50 direct;
   y-caveat 32 with caveat recorded or waived: AUTO-004/006 tokio card amendment,
   EXT-005 keep-or-revert, PROV-016 freeze waiver, INT/OPS/SHARE quiesced-tree re-run,
   WEB-007..012 write-paths decision, WEB-013/015 NOT-ACCEPTED stands pending executor/
   membership lanes). No lane edits ralph.json (ADR-007).
2. **ONE integration commit** (tracked fixes + lib.rs wirings incl. PROV-017 one-liner;
   fmt at commit; 6 additive suites only with passing gates): blocked on guard GREEN (§9.1)
   + §9 merge order. This lane performed no commit.
3. **FEATURES.md / accounting sync**: stale-accepted set + unknown-prefix allowlist-or-map
   + plan-validator 11 extra reconciliation. Read-only lane; sync is controller-owned.
4. **Freeze-or-drop calls**: 6 additive suites at pinned hashes (bounds `9d2c8d87…`,
   bounds2 `3595ed6c…`, reason/full sha256 cA/cB) + EXT-005 wire keep-or-revert +
   `plugin_transform` 5/5 re-run → drop `ext_replay_lane` same commit.
5. **Routine verifier re-runs on quiesced tree** (§5 mandates incl. PROV-016 triple,
   WEB lift triples, REL bF/cC commands, zB-cargo re-run).

## 9. Ranked blockers top 5 (ALL controller-owned — evidence complete per §7)

1. Guard/controller backlog exhaustion (owner: controller). 122 repo + 133 plan errors,
   exit 1, byte-identical classes vs TRIAGE-16/17/18. Porcelain 204 M + 274 ?? = 478;
   drift = worklogs + additive suites only; diff-check clean; forbidden paths untouched.
   Blocks every accept-flip; integration commit forbidden until guard GREEN.
2. Integration commit + unattributed lib.rs drift disposition (owner: controller/integrator).
   Server +20, sessions +3, foundation +5/-2 uncommitted; `pub mod ext_manifest_lane`
   wire (:19) keep-or-revert; security/tools 1/1 swaps attribute-or-revert; storage
   +18/-9 fmt-only attribute-or-revert; `plugin_transform` +9/-2 guard-port needs
   verifier-logged serial 5/5 re-run, then twin drop same commit. Action: §5 lane gates
   GREEN per suite, then ONE commit.
3. AUTO-004/006 tokio mechanism gap (owner: controller + integration lane). PROPOSAL2
   spec'd, NOT applied, dep approval either way. Rows stay y-caveat until card amendment
   (sync state machine) or tokio lane lands. Broker mirror SUFFICES for behavior evidence.
4. Freeze-or-drop + FEATURES/accounting sync (owner: controller + test owners). fmt steady
   at 58; 6 additive suites untracked (freeze at cA/cB/bE hashes or drop); FEATURES
   stale-accepted set + unknown-prefix allowlist-or-map + plan 11 extra. Commit must not
   land untracked modules without passing gates.
5. Routine quiesced-tree verifier re-runs (owner: verifier + controller). INT/OPS/SHARE
   y-caveat rows + zB-cargo method upgrade + REL bF/cC re-run + PROV-016 triple +
   WEB lift triples per §5. Evidence complete (§4/§6); re-runs are acceptance formalities,
   not discovery. WEB-013/015 card contracts (executor / membership-memory) need future
   implementation lanes; NOT ACCEPTED stands independent of boundary evidence.
