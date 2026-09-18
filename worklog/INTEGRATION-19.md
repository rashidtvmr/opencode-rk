# INTEGRATION-19 — post-commit flip-check + 82/82 recompute on b60ceda (v19)

Rev: `b60ceda` HEAD (main, +2 ahead origin: `1be93d3` integrate + `b60ceda` receipt).
Base: `248f519`. Date: 2026-09-16.
Bounds: READ-ONLY except this file. No product/controller edits, no fmt run, no commit,
no ralph.json/FEATURES.md. Supersedes INTEGRATION-18 where disk disagrees (marked CORR).
Inputs: `worklog/INTEGRATION-18.md`, `worklog/GUARD-TRIAGE-19.md`,
`worklog/ACCEPTANCE-FLIP-PROPOSAL.md`, `worklog/CONFIRM-CI.md`, `worklog/CONFIRM-CK.md`,
`worklog/CONFIRM-DJ.md`, `worklog/CONFIRM-DK.md`, `worklog/CONFIRM-DL.md`,
`worklog/TWINS-WATCH6.md`, `worklog/TURN-STREAM-GATE.md`, `worklog/INTEGRATION-COMMIT.md`,
on-disk diffs, `ralph.json` (read-only count + HEAD-vs-worktree diff).
No `CONFIRM-P7..P13` lanes on disk (`ls worklog | grep CONFIRM` = CI/CK/DJ/DK/DL only).
Big change vs v18: **integration commit `1be93d3` LANDED** (483 files, +26141/-1232)
+ receipt `b60ceda`; worktree now **clean except `M ralph.json` + `?? ACCEPTANCE-FLIP-PROPOSAL.md`**;
**uncommitted ralph.json worktree diff flips all 82 INT-18 rows to accepted** (HEAD still
176/44/38). Flip NOT committed. Guard still RED (class changed: FEATURES/backlog, not drift).

Guard: TRIAGE-19: repo FAIL (122 class) + plan 134 lines, exit 1, `git diff --check` clean,
porcelain 471 = 204 M + 267 ??.
This lane: stub 0, fmt 58, `git diff --check` CLEAN, worktree `M ralph.json` (82 flips)
+ `?? worklog/ACCEPTANCE-FLIP-PROPOSAL.md` (controller-uncommitted).
`ralph.json` HEAD 258 (176 accepted / 44 in-progress / 38 not-started, re-verified via
python count on `git show HEAD:ralph.json`); worktree 258 (258 accepted / 0 / 0).
Committed-tree status counts: post-`1be93d3` crates diff vs `248f519` = 249 files
(9 key lib.rs/fix files +65/-20, rest evidence waves).
Forbidden paths: this lane touched only `worklog/INTEGRATION-19.md` + `/tmp/opencode/p15-stub.log`
+ `/tmp/opencode/p19-validate.log` (tmp, not repo). GUARD RED — no commit.

## 1. Stub / fmt / status (this lane)

- Strict grep `grep -rn --include='*.rs' -e 'todo!' -e 'unimplemented!' -e '#\[ignore\]' crates/`
  → **0 hits** (exit 1, empty). Log: `/tmp/opencode/p15-stub.log` (0 lines). CLEAN.
  Matches v18 `/tmp/opencode/dO-stub.log` (0) + COMMIT pre-flight 0 + DJ/DK/DL scans clean.
- `cargo fmt --check | grep -c 'Diff in'` → **58** (unchanged vs v16/v17/v18/STUB-FMT-9/10;
  no fmt run — banned). Post-commit fmt debt carried, not grown.
- `git status --porcelain=v1` (bare + rtk agree): **2 paths = `M ralph.json` +
  `?? worklog/ACCEPTANCE-FLIP-PROPOSAL.md`**. CORR vs TRIAGE-19 (471) and v18 (478):
  those counted the pre-commit dirty tree; `1be93d3` committed 483 paths, `b60ceda` +1
  worklog; remaining delta = this lane's owned file (untracked at read time) is the only
  growth. Zero non-M/?? classes. `rtk` vs bare off-by-one noted in COMMIT doc; bare authoritative.
- `git diff HEAD --stat -- crates/`: **empty** (all product bytes committed in `1be93d3`).
  CORR vs v18 (203 files +2071/-1232 uncommitted): that diff is now inside `1be93d3`.
  Committed delta `248f519..HEAD -- crates/` = 249 files.
- `git diff --check` → CLEAN (staged + unstaged per COMMIT pre-flight; this lane re-verified
  unstaged clean, exit 0).
- `git diff HEAD -- ralph.json`: **1 file, 82 insertions / 82 deletions**, all
  `in-progress`/`not-started` → `accepted` (verified row-by-row §7).

## 2. e-wave receipts (new since v18)

| Lane | Content | Verdict |
|---|---|---|
| INTEGRATION-COMMIT (`1be93d3`) | 483 files +26141/-1232 on `248f519`; pre-flight stub 0, diff-check clean, guard 122 noted; serial gates 58/58 (agents 15 + tools 10 + providers 15 + stream 2 + cli 16); post-commit status 0 paths | COMMIT LANDED, GREEN |
| b60ceda receipt | docs-only +1 worklog commit on `1be93d3`; product bytes identical to `1be93d3` | receipt, no product change |
| ACCEPTANCE-FLIP-PROPOSAL (verifier, b60ceda) | full-crate gates a–h: agents 35 + foundation 168 + providers 364 + tools 344 serial + sessions 288 + cli 23 + server 272 serial + stream 2/2 isolate + REL T01×3 exit 0 cmp clean = **1494 + 2 + 3**; validator hashes ab9b350e/79be6ef1/3a462032 pinned; full 82-row flip table proposed all→accepted; FEATURES sync + twin-drop list + caveats recorded | GREEN, PROPOSAL ONLY (no ralph/plan/product edits) |
| CONFIRM-DJ | agents 35/35 + INT 40/40 + OPS 45/45 + AUTO-005 tool 4 exits PASS; serial JOBS=1 THREADS=1; frozen/ralph.json untouched | GREEN re-confirm, 0 failed |
| CONFIRM-DK | SHARE 50/50 + EXT 50/50 (canonical plugin_* suites) + TOOL/SYNC/UI 40/40 = **140, 0 failed**; redaction probe 0 hits | GREEN re-confirm |
| CONFIRM-DL | PROV 62 + SRVNEW 48 + CLI 23 + WEB sweep 118 = **251, 0 failed**; stream excluded (serial mandate); zero edits | GREEN re-confirm |
| TWINS-WATCH6 | 0/13 pair-body drift vs WATCH5 (all sha8/wc/diff-lines identical); wiring drift y third-party only (`ext_manifest_lane` :19 carried + `tool_quota`/`tool_sandbox` order-only reorder); EXT-005 canonicalization flag both-KEEP; test headers `#[path]`-only holds | survey, no cargo runs |
| GUARD-TRIAGE-19 | repo 122 FAIL + plan 134 lines FAIL, exit 1, diff-check clean, 471 = 204 M + 267 ?? (pre-commit read) | GUARD RED (backlog, not wiring) |
| p19-validate (this lane, b60ceda worktree) | `validate_repository` exit 1, **178 error(s)** (class shift vs 122: now dominated by FEATURES stale + accepted/unknown backlog list incl. flipped 82); FEATURES.md hits 127; `validate_plan` 190 lines tail same FEATURES stales | GUARD RED, log `/tmp/opencode/p19-validate.log` |

## 3. Wiring checklist (committed truth: `248f519..HEAD`, re-verified via diff)

| Crate | File | State | Committed delta vs 248f519 | Owner worklog | Gate evidence |
|---|---|---|---|---|---|
| server | `crates/server/src/lib.rs` | WIRED-committed (`1be93d3`) | +20 additive | SERVER-WIRING-FINAL | COMMIT final-server 2/2; ACC gate g 272; cA/cB lifts |
| sessions | `crates/sessions/src/lib.rs` | WIRED-committed | +3 | SESSIONS-WIRING-FINAL | gate e 288; DK share 50/50 |
| foundation | `crates/foundation/src/lib.rs` | WIRED-committed | +5/-2 (7-line diff) | FOUNDATION-WIRING-FINAL | gate b 168; CI/DJ OPS 45/45 |
| cli | `crates/cli/src/main.rs` | n/a, no edit needed | 0 | CLI-WIRING | COMMIT cli 16/16; gate f 23 |
| providers | `crates/providers/src/lib.rs` | n/a, no edit (deliberate) | 0 | PROVIDERS-WIRING | gate c 364; DL prov 62 |
| security | `crates/security/src/lib.rs` | COMMITTED (was UNPLANNED-uncommitted) | 1/1 alpha swap | none | none — attribute or revert (unchanged call) |
| tools | `crates/tools/src/lib.rs` | COMMITTED (was UNPLANNED-uncommitted) | `pub mod ext_manifest_lane` (:19) + quota/sandbox order | none (parallel-lane wire) | gate d 344 incl. ext_manifest_lane 5/5; WATCH6 §5 |
| storage | `crates/storage/src/lib.rs` | COMMITTED, fmt-only | +18/-9 reflows, zero mod add/remove | none | none — attribute or revert |
| tools | `crates/tools/src/plugin_transform.rs` | GUARD-PORT-committed | +9/-2 | EXT-TWINS-DISPOSITION pair-1 | COMMIT final-tools 10/10; WATCH6 §3 |
| tools | `crates/tools/src/ext_replay_lane.rs` | twin fmt reflow (guard INTACT), committed | +6/-4 | none (sibling fmt churn) | guard semantics unchanged; pair-1 drop gated on §5 re-run |
| providers | `crates/providers/src/claude_oauth.rs` | FIX-committed (PROV-017 gate CLEARED) | +1/-1 `{LOOPBACK_REDIRECT_URI}` | PROV-VERIFY4 | COMMIT providers 15/15; gate c 364 |
| providers | `crates/providers/src/codex_oauth.rs` | clean impl, bounds in NEW test files (committed) | 0 in src | PROV-016-FREEZE (bE) | 17/17 frozen; gate c incl. bounds+bounds2 |

- 6 additive suites now COMMITTED (were untracked in v18): `codex_oauth_bounds.rs` (6),
  `codex_oauth_bounds2.rs` (6), `web_capabilities_reason.rs` (3), `web_capabilities_full.rs` (5),
  `web_workspace_reason.rs` (3), `web_workspace_full.rs` (5). Freeze decision now = tag hashes
  in place (bE/cA/cB pins), not freeze-or-stage.
- SHARE wiring COMPLETE as designed (v17 §3a carried through commit): canonical
  `share_merge/policy/queue` wired, lanes deliberately unwired + `#[path]`-self-included,
  WATCH6 keep-both all 5 pairs. No integrator wiring action on SHARE.
- EXT-005 integrator call stands post-commit: keep-wire (attribute + freeze bytes-lane) or
  revert-wire (`#[path]`-only) as follow-up commit. Until decided, EXT-005 stays y-caveat
  (count-neutral).
- Worktree `M ralph.json` + `?? ACCEPTANCE-FLIP-PROPOSAL.md` are the ONLY uncommitted paths;
  `git diff HEAD -- crates/` empty proves zero product drift since `1be93d3`/`b60ceda`.

## 4. Per-slice GREEN matrix (latest receipt per slice wins; no cargo runs by THIS lane)

Serial discipline all waves: one cargo cmd at a time, `timeout 120`, `rtk` prefix,
`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`, `--test-threads=1` unless noted.
e-wave upgrades marked NEW (committed-tree gates a–h + DJ/DK/DL on `248f519` bytes =
`1be93d3` product bytes; ACC gates re-proved on `b60ceda` committed tree).

| Slice | Wave receipt | GREEN after restore/commit | RED bite shape |
|---|---|---|---|
| AUTO-004/006/005 | yA + bI/bJ-agents + CI + DJ + ACC-a | DJ 35/35 + CI 15/15 + ACC agents 35; tool 4/4 | xA: 3p/2f + 4p/1f + 0p/5f; aE tool 4/4 reproducible |
| ACP/WSX/SDK/HEAD/RUN | yB + CK + DL + ACC-f/g | DL srvnew 48 + cli 23; yB 64/64; ACC cli 23 + server 272 | xB per-suite bites (v14 §4) |
| EXT-001/002/004/006/008 | yC + CI + DK + ACC-d | DK EXT 50/50 + CI 50/50 + ACC tools 344 | xC via EXT1C (strong) |
| EXT-005 bytes lane | xC + zF + bB PRE + CI + COMMIT-tools | COMMIT 10/10 (transform+replay); zF 4/1 + bB PRE hash-match | xC 3p/2f; zF 4/1 |
| EXT-005 canonical | yC + WATCH6 §4 | 5/5 struct-shape ONLY — CONTRACT MISMATCH stands (§6) | none vs card bytes-pipeline |
| EXT-009/010/011/012 | yD + ACC-d | 5/5 each (011 PORTED b4c517e3); ACC tools 344 | xD (strong) |
| TOOL-016..020, UI-019, SYNC-001/002 | yJ + zQ + CK + DK + ACC-d | DK tool 40/40 + zQ 40/40 + CK 40 + ACC 344 | xJ (strong) |
| SHARE-001/002 | zA + yI + bA + DK + ACC-e | DK share 50/50 + bA 50/50 + ACC sessions 288 | none before |
| SHARE-003/004/005 | xI + yI + aA + bA + DK + ACC-e | xI RED 2p/3f, 3p/2f, 4p/1f + aA + DK 50/50 + ACC 288 | temp-stub RED |
| WEB-001..006 | zB + yK + CK + DL + ACC-g | DL web 118 + CK 100 + ACC server 272 | none before (GREEN-only) |
| WEB-007..012 | zC + yK + bQ + CK + DL + ACC-g | entry-point RED + 5/5 + `cmp` + bQ + DL 118 + ACC 272 | none before (HOLD by design) |
| WEB-013 boundary | s3 + uC + bC + cA-LIFT + ACC-g | 9/9 serial (1+3+5) + mut-controls; ACC server 272 | s1 flag-flip FAIL, s3 missing-key FAIL; card T01–T05 absent §6 |
| WEB-014 | s3 + SUITES + CK + DL + ACC-g | 5/5 `chat_nav_lane` T01–T05 + 2/2 HTTP + DL 118 | s3 3p/2f |
| WEB-015 boundary | s3 + uC + bD + cB-LIFT + ACC-g | 10/10 serial (2+3+5) + mut-controls; ACC 272 | ghost-id WEAK-RED; card T01–T05 absent §6 |
| WEB-016/017 | s3 + SUITES + CK + DL + ACC-g | 5/5 each (017 T01–T05 `web_artifact`) + DL 118 | s3 1p/4f each |
| PROV-015 | yG + CK + DL + ACC-c | 5/5; DL 62; ACC 364 | uD A1/A2/A3 (strong) |
| PROV-016 lifecycle+boundary | yG + bounds/bounds2 + bE-FREEZE + DL + ACC-c | 17/17 (5+6+6); B3 kills ×2; DL 62; ACC 364 | uD B3/B3alt/B3exp KILLED ×2 |
| PROV-017..024 | zD + yG + CK + COMMIT-prov + DL + ACC-c | COMMIT 15/15 sample + DL 62 + ACC 364; 017 fix-present | pre-fix BadConsentUrl history |
| OPS-001..009 | zH + aH + bH + CI + DJ + ACC-b | DJ 45/45 + CI 45/45 + ACC foundation 168 | uB FINAL epoch (strong) |
| INT-001/002/003/005/006/007/009/010 | zG + aG + bG + CI + DJ + ACC-c | DJ 40/40 + CI 40/40 + ACC providers 364 | uA FINAL epoch (strong) |
| session_turn_stream_api | TURN-STREAM-GATE (3x serial) + yK + COMMIT-server + ACC-g2 | COMMIT 2/2 + ACC 2/2 isolate + 3x serial | harness race, not behavior RED |
| EXT002-T05 | DETERMINISM doc + ACC-d | serial 30/30; ACC tools 344 run serial | flaky-by-construction; NO-FIX |
| EXT twins (8 pairs) | DISPOSITION + WATCH6 | 0/13 drift vs WATCH5; pair-1 guard PORTED both sides, droppable | pairs droppable by integrator AFTER §5 re-run |
| SHARE twins (5 pairs) | DISPOSITION + WATCH6 | survey only, 0 drift, keep-both everywhere | — |
| REL-001 | REL-001.md T01..T05 + bF + cC + ACC-h | T01..T05 (0,2,2,0,2) + entrypoint RED exit 2 + tool-error 1 + `cmp` clean + ACC T01 exit 0 (`ab9b350e…`) | §6 REL rationale |
| REL-002 | REL-002.md T01..T05 + bF + cC + ACC-h | T01 all-pass + T02/T03/T04a/T04b/T05 exits 2 + import-only RED 2 + blocked/malformed 1 + ACC exit 0 (`79be6ef1…`) | §6 REL rationale |
| REL-003 | REL-003.md T01..T05 + bF + cC + ACC-h | T01 all-pass + T02/T03a/T03b/T04a/T04b/T05 exits 2 + canary 0 + malformed 1 + ACC exit 0 (`3a462032…`) | §6 REL rationale |

### Dirty-tree caveat (NARROWED, not lifted)

zA/zC/zD/zG/zH stubs applied to CURRENT (dirty-at-the-time) bytes, restored byte-identical
(sha256/cmp ALL_IDENTICAL, zero TEMP markers). RED proves test→impl wiring on that content.
`1be93d3` committed those exact bytes, and ACC gates a–h re-proved full-crate GREEN on the
committed tree (`b60ceda` product-identical) — the GREEN half is now committed-tree evidence.
The RED half keeps its dirty-tree qualifier: quiesced-tree RED re-run still required before
accept-flip for INT/OPS/SHARE y-caveat rows (controller/verifier call §8). zB standalone-`rustc`
method weaker than cargo; re-run under clean cargo before accept-flip. b-wave + CI/CK/DJ/DK/DL
+ ACC confirm GREEN on identical hashes but do not lift the dirty-tree qualifier on RED.

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
- REL re-confirm (bF + cC commands + ACC-h): T01×2 per validator + `cmp` determinism;
  `tools/ fixtures/ ralph.json` must stay clean.
- ACC full-crate gates a–h are the committed-tree GREEN baseline; any re-run uses same
  serial discipline (tools/server `--test-threads=1`, JOBS=1 THREADS=1).

## 6. Flip rationale — why 82/82 stands post-commit (explicit)

- WEB-013 partial → **y-caveat**: UNCHANGED from v18 (cA-LIFT 9/9 + RED bites + mut-controls;
  sha256 pinned; frozen api 0-byte). Card T01–T05 executor/replay absent
  (`tasks/WEB-013.md:38` + `WEB-013-LIFT.md:3`; NOT ACCEPTED stands; future lane implements
  executor + frozen T01–T05). ACC-g (server 272) adds committed-tree GREEN. Evidence-ready = y-family.
- WEB-015 partial → **y-caveat**: UNCHANGED (cB-LIFT 10/10 + mut-controls; frozen api untouched).
  Card T01–T05 membership/memory absent (`tasks/WEB-015.md:15-18`; NOT ACCEPTED stands).
  ACC-g adds committed-tree GREEN. Evidence-ready.
- REL-001..003 n → **y**: UNCHANGED v18 rationale (validator IS the product; in-own-worklog
  behavior-RED; full T01..T05 ~15× byte-identical; bF + cC). ACC-h adds committed-tree T01
  exit-0 + `cmp` clean at pinned hashes on `b60ceda`. `lane_gate.py` REL gap is gate-script
  coverage, not slice evidence. Verifier re-run on quiesced tree routine (§8), not blocker.
- EXT-005 stays y-caveat (§3 keep-or-revert follow-up commit). PROV-016 stays y-caveat (freeze
  waiver: additive suites now COMMITTED at bE hashes — waiver = tag/attest, not stage).
  INT/OPS/SHARE-003..005 + WEB-007..012 + AUTO-004/006 stay y-caveat (quiesced-tree RED re-run /
  tokio-mechanism / write-paths-by-design). All caveats acceptance-side; evidence complete → y-family.
- POST-COMMIT upgrade (all rows): GREEN half now committed-tree (ACC a–h on `b60ceda`,
  product-identical to `1be93d3`); `git diff HEAD -- crates/` empty proves no drift since.
  Only the RED-half dirty-tree qualifier (§4) and the acceptance caveats above hold rows at
  y-caveat instead of y. Nothing regressed vs v18.

## 7. ralph flip table — worktree-vs-HEAD check on all 82 INT-18 rows (EVIDENCE ONLY — do NOT edit)

ready = behavior-RED receipt complete + GREEN on disk (post-commit: GREEN committed).
`y-caveat` = evidence ready, acceptance still blocked (see note).
Recompute: **ready-y-family 82/82 (y 50 / y-caveat 32 / partial 0 / not-ready 0)**,
identical set to v18. Flip-status: worktree `M ralph.json` flips **all 82/82 to accepted**
(python HEAD-vs-worktree diff: flipped_count 82, all→accepted, missing ∅, extra ∅).
CORR vs TRIAGE-19-era counts: HEAD on disk is still 176/44/38; 258/0/0 exists ONLY in the
uncommitted worktree diff (controller-owned, uncommitted — flip-status y on all rows below =
worktree shows accepted, NOT HEAD).

| # | Story | HEAD (committed) | worktree | ready | flipped y/n | Evidence path / note |
|---|---|---|---|---|---|---|
| 1 | AUTO-004 | in-progress | accepted | y-caveat | y | yA 5/5 + bJ + CI 15/15 + DJ 35 + ACC-a 35; RED xA 3/2; CAVEAT tokio-mechanism (PROPOSAL2 unapplied) |
| 2 | AUTO-005 | in-progress | accepted | y | y | yA 5/5 + aE 4/4 + CI + DJ + ACC-a; tool 4 exits |
| 3 | AUTO-006 | in-progress | accepted | y-caveat | y | yA 5/5 (bJ) + CI + DJ; RED xA 4/1; same tokio-pool caveat |
| 4 | EXT-001 | in-progress | accepted | y | y | yC 5/5 + CI/DK 50/50 + ACC-d 344; RED xC 4/1 |
| 5 | EXT-002 | in-progress | accepted | y | y | yC 5/5 + CI/DK + ACC-d; RED xC 4/1 |
| 6 | EXT-004 | in-progress | accepted | y | y | yC 5/5 + CI/DK; RED xC 3/2 |
| 7 | EXT-005 | in-progress | accepted | y-caveat | y | BYTES y (xC 3/2 + zF 4/1 + bB PRE + COMMIT 10/10) BUT canonical struct-only — follow-up keep-or-revert wire (§3) |
| 8 | EXT-006 | in-progress | accepted | y | y | yC 5/5 + CI/DK; RED xC 4/1 |
| 9 | EXT-008 | in-progress | accepted | y | y | yC 5/5 + CI/DK; RED xC 4/1 |
| 10 | EXT-009 | in-progress | accepted | y | y | yD 5/5 + ACC-d; RED xD 0/5 |
| 11 | EXT-010 | in-progress | accepted | y | y | yD 5/5 + ACC-d; RED xD 1/4 |
| 12 | EXT-011 | in-progress | accepted | y | y | yD 5/5 PORTED b4c517e3 + ACC-d; RED xD 0/5 |
| 13 | EXT-012 | in-progress | accepted | y | y | yD 5/5 + ACC-d; RED xD 0/5 |
| 14 | INT-001 | in-progress | accepted | y-caveat | y | zG 2/3 + yE + bG + CI/DJ 40/40 + ACC-c 364; CAVEAT quiesced-tree RED re-run |
| 15 | INT-002 | in-progress | accepted | y-caveat | y | zG 0/5 + yE + bG + CI/DJ; same caveat |
| 16 | INT-003 | in-progress | accepted | y-caveat | y | zG 3/2 + yE + bG + CI/DJ; same caveat |
| 17 | INT-005 | in-progress | accepted | y-caveat | y | zG 3/2 + yE + bG + CI/DJ; same caveat |
| 18 | INT-006 | in-progress | accepted | y-caveat | y | zG 4/1 + yE + bG + CI/DJ; same caveat |
| 19 | INT-007 | in-progress | accepted | y-caveat | y | zG 4/1 + yE + bG + CI/DJ; same caveat |
| 20 | INT-009 | in-progress | accepted | y-caveat | y | zG 2/3 + yE + bG + CI/DJ; same caveat |
| 21 | INT-010 | in-progress | accepted | y-caveat | y | zG 0/5 + yE + bG + CI/DJ; same caveat |
| 22 | OPS-001 | in-progress | accepted | y-caveat | y | zH rc=101 + `cmp` IDENTICAL + yF + bH + CI/DJ 45/45 + ACC-b 168; same caveat |
| 23 | OPS-002 | in-progress | accepted | y-caveat | y | zH 0/5 + yF + bH + CI/DJ; same caveat |
| 24 | OPS-003 | in-progress | accepted | y-caveat | y | zH 2/3 + yF + bH + CI/DJ; same caveat |
| 25 | OPS-004 | in-progress | accepted | y-caveat | y | zH 4/1 + yF + bH + CI/DJ; same caveat |
| 26 | OPS-005 | in-progress | accepted | y-caveat | y | zH 4/1 + yF + bH + CI/DJ; same caveat |
| 27 | OPS-006 | in-progress | accepted | y-caveat | y | zH 4/1 + yF + bH + CI/DJ; same caveat |
| 28 | OPS-007 | in-progress | accepted | y-caveat | y | zH 2/3 + yF + bH + CI/DJ; same caveat |
| 29 | OPS-008 | in-progress | accepted | y-caveat | y | zH 2/3 + yF + bH + CI/DJ; same caveat |
| 30 | OPS-009 | in-progress | accepted | y-caveat | y | zH 2/3 + yF + bH + CI/DJ; same caveat |
| 31 | REL-001 | in-progress | accepted | y | y | T01..T05 (0,2,2,0,2) + entrypoint RED 2 + tool-error 1 + `cmp` + bF + cC + ACC-h (`ab9b350e…`) |
| 32 | REL-002 | in-progress | accepted | y | y | T01 all-pass + exits 2 + import-only RED 2 + blocked/malformed 1 + bF + cC + ACC-h (`79be6ef1…`) |
| 33 | REL-003 | in-progress | accepted | y | y | T01 all-pass + exits 2 + canary 0 + malformed 1 + bF + cC + ACC-h (`3a462032…`) |
| 34 | SHARE-001 | in-progress | accepted | y | y | zA 4/1 + mirror 5/5 + bA/DK 50/50 + ACC-e 288 |
| 35 | SHARE-002 | in-progress | accepted | y | y | zA 4/1 + mirror 5/5 + bA/DK + ACC-e |
| 36 | SHARE-003 | in-progress | accepted | y-caveat | y | xI 2/3 + aA + yI/bA/DK; CAVEAT unwired lane + quiesced-tree re-run |
| 37 | SHARE-004 | in-progress | accepted | y-caveat | y | xI 3/2 + aA + yI/bA/DK; same caveat |
| 38 | SHARE-005 | in-progress | accepted | y-caveat | y | xI 4/1 + aA + yI/bA/DK; same caveat |
| 39 | WEB-001 | in-progress | accepted | y | y | zB 5/5 + restore + CK/DL + ACC-g 272 |
| 40 | WEB-002 | in-progress | accepted | y | y | zB 4F + restore + CK/DL + ACC-g |
| 41 | WEB-003 | in-progress | accepted | y | y | zB 3F + restore + CK/DL + ACC-g |
| 42 | WEB-004 | in-progress | accepted | y | y | zB 3F + restore + CK/DL + ACC-g |
| 43 | WEB-005 | in-progress | accepted | y | y | zB 5/5 + restore + CK/DL + ACC-g |
| 44 | WEB-006 | in-progress | accepted | y | y | zB 2/2 (lock stub) + CK/DL + ACC-g |
| 45 | WEB-007 | not-started | accepted | y-caveat | y | zC 4/1 + 5/5 + `cmp` + bQ + CK/DL + ACC-g; CAVEAT write-paths by design + browser unexecuted |
| 46 | WEB-008 | not-started | accepted | y-caveat | y | zC 2/3 + GREEN + bQ + CK/DL + ACC-g; same caveat |
| 47 | WEB-009 | not-started | accepted | y-caveat | y | zC 0/5 + GREEN + bQ + CK/DL + ACC-g; same caveat |
| 48 | WEB-010 | not-started | accepted | y-caveat | y | zC 1/4 + GREEN + bQ + CK/DL + ACC-g; same caveat |
| 49 | WEB-011 | not-started | accepted | y-caveat | y | zC 0/5 + GREEN + bQ + CK/DL + ACC-g; same caveat |
| 50 | WEB-012 | not-started | accepted | y-caveat | y | zC 1/4 + GREEN + bQ + CK/DL + ACC-g; same caveat |
| 51 | WEB-013 | not-started | accepted | y-caveat | y | cA-LIFT 9/9 + s1/s3 RED bites + mut-controls + ACC-g; CAVEAT executor/replay absent, NOT ACCEPTED stands |
| 52 | WEB-014 | not-started | accepted | y | y | s3 3/2 + `chat_nav_lane` T01–T05 5/5 + CK/DL + ACC-g |
| 53 | WEB-015 | not-started | accepted | y-caveat | y | cB-LIFT 10/10 + mut-controls + ACC-g; CAVEAT membership/memory absent, NOT ACCEPTED stands |
| 54 | WEB-016 | not-started | accepted | y | y | s3 1/4 + 5/5 + CK/DL + ACC-g |
| 55 | WEB-017 | not-started | accepted | y | y | s3 1/4 + `web_artifact` T01–T05 5/5 + CK/DL + ACC-g |
| 56 | PROV-015 | not-started | accepted | y | y | uD A1/A2/A3 + yG 5/5 + CK/DL 62 + ACC-c 364 |
| 57 | PROV-016 | not-started | accepted | y-caveat | y | 17/17 (5+6+6) + B3 kills ×2 + DL + ACC-c; CAVEAT committed-but-unwaived additive suites, freeze attestation needed |
| 58 | PROV-017 | not-started | accepted | y | y | zD fix-present + yG + CK/DL + COMMIT 15/15 + ACC-c |
| 59 | PROV-018 | not-started | accepted | y | y | zD 2/3 + restore + CK/DL + ACC-c |
| 60 | PROV-019 | not-started | accepted | y | y | zD 0/5 + restore + CK/DL + ACC-c |
| 61 | PROV-020 | not-started | accepted | y | y | zD 2/3 + restore + CK/DL + ACC-c |
| 62 | PROV-021 | not-started | accepted | y | y | zD 1/4 + restore + CK/DL + ACC-c |
| 63 | PROV-022 | not-started | accepted | y | y | zD 1/4 + restore + CK/DL + ACC-c |
| 64 | PROV-023 | not-started | accepted | y | y | zD fixture 0/5 + restore + CK/DL + ACC-c |
| 65 | PROV-024 | not-started | accepted | y | y | zD fixture 0/5 + restore + CK/DL + ACC-c |
| 66 | UI-019 | not-started | accepted | y | y | yJ 5/5 + CK/DK + ACC-d; RED xJ 1/4 |
| 67 | TOOL-016 | not-started | accepted | y | y | yJ 5/5 + CK/DK; RED xJ 1/4 |
| 68 | TOOL-017 | not-started | accepted | y | y | yJ 5/5 + CK/DK; RED xJ 1/4 |
| 69 | TOOL-018 | not-started | accepted | y | y | yJ 5/5 + CK/DK; RED xJ 4/1 |
| 70 | TOOL-019 | not-started | accepted | y | y | yJ 5/5 + CK/DK; RED xJ 1/4 |
| 71 | TOOL-020 | not-started | accepted | y | y | yJ 5/5 + CK/DK; RED xJ 3/2 |
| 72 | SYNC-001 | not-started | accepted | y | y | yJ 5/5 + CK/DK; RED xJ 0/5 |
| 73 | SYNC-002 | not-started | accepted | y | y | yJ 5/5 + CK/DK; RED xJ 3/2 |
| 74 | RUN-001 | not-started | accepted | y | y | yB 5/5; RED xB 0/5; WIRED-committed §3; ACC-f/g |
| 75 | ACP-001 | not-started | accepted | y | y | yB 5/5; RED xB 3/2; WIRED-committed; ACC-g |
| 76 | ACP-002 | not-started | accepted | y | y | yB 5/5; RED xB 3/2; WIRED-committed; ACC-g |
| 77 | WSX-001 | not-started | accepted | y | y | yB 5/5; RED xB 4/1; WIRED-committed; ACC-g |
| 78 | WSX-002 | not-started | accepted | y | y | yB 5/5; RED xB 4/1; WIRED-committed; ACC-g |
| 79 | SDK-001 | not-started | accepted | y | y | yB 12/12; RED xB 6/6; WIRED-committed; ACC-g |
| 80 | SDK-002 | not-started | accepted | y | y | yB 11/11; RED xB 1/10; WIRED-committed; ACC-g |
| 81 | HEAD-001 | not-started | accepted | y | y | yB 8/8; RED xB 2/6, no wiring needed; ACC-f |
| 82 | HEAD-002 | not-started | accepted | y | y | yB 8/8; RED xB 7/1, no wiring needed; ACC-f |

Plus (not stories, controller-owned): unknown prefixes SYNC/RUN/ACP/WSX/SDK/HEAD →
allowlist-or-map + reconcile accounting (plan validator 11 extra, still open); FEATURES stale-accepted
set now LARGER (127 FEATURES.md hits in p19 log; sync is the guard's main class) → FEATURES.md sync only.
EXT twins pairs 1–8 → integrator executes drops AFTER §5 pair-1 re-run (WATCH6: pair-1 droppable like
2–8, integrator call, NOT done). EXT-005 card-vs-canonical gap → keep-or-revert follow-up commit (§3).

## 8. Remaining controller-only actions (this lane cannot perform)

1. **Commit-or-revert the worktree flip** (`M ralph.json` 82 flips + `?? ACCEPTANCE-FLIP-PROPOSAL.md`):
   worktree shows 258/0/0, HEAD shows 176/44/38. y 50 direct-commit; y-caveat 32 commit with caveat
   recorded or waived (AUTO-004/006 tokio card amendment, EXT-005 keep-or-revert follow-up, PROV-016
   freeze attestation, INT/OPS/SHARE quiesced-tree RED re-run, WEB-007..012 write-paths decision,
   WEB-013/015 NOT-ACCEPTED stands pending executor/membership lanes). No lane edits ralph.json (ADR-007);
   authorship of the worktree diff is third-party (not this lane) — controller verifies provenance
   before committing. Until committed, HEAD counts govern.
2. **FEATURES.md / accounting sync**: 127 FEATURES.md stale hits in p19 log + unknown-prefix
   allowlist-or-map + plan-validator extras. Read-only lane; sync is controller-owned. This is now
   the LARGEST guard class (was drift; drift is gone).
3. **Guard GREEN**: `validate_repository` exit 1, 178 error(s) (up from 122 class — recompute, not
   regression: flip widens accepted/unknown + FEATURES stale sets); `validate_plan` 190 lines FAIL.
   Backlog exhaustion + FEATURES sync block every accept-commit until controller reconciles.
4. **Freeze-attest + twin-drop follow-ups**: 6 additive suites now committed at cA/cB/bE hashes
   (attest, don't stage) + EXT-005 wire keep-or-revert + `plugin_transform` 5/5 re-run →
   drop `ext_replay_lane` (+ pairs 2–8) in a follow-up commit. SHARE: no action (keep-both).
5. **Routine verifier re-runs on quiesced tree** (§5 mandates incl. PROV-016 triple, WEB lift triples,
   REL bF/cC/ACC-h, zB-cargo method upgrade, INT/OPS/SHARE RED-half re-runs).

## 9. Ranked blockers top 5 (ALL controller-owned — evidence complete per §7)

1. Guard GREEN + FEATURES/backlog reconciliation (owner: controller). `validate_repository` exit 1,
   178 error(s): accepted/unknown backlog list + 127 FEATURES.md stales; `validate_plan` 190 lines FAIL.
   Drift class RESOLVED by `1be93d3` (status 2 paths, crates diff empty, diff-check clean) but guard
   still RED on accounting. Blocks committing the 82-flip worktree diff.
2. Worktree-flip provenance + commit decision (owner: controller). `M ralph.json` flips exactly the
   82 INT-18 rows (missing ∅, extra ∅) but is uncommitted third-party work; HEAD still 176/44/38.
   Action: verify author, record/waive 32 caveats, commit flip + `ACCEPTANCE-FLIP-PROPOSAL.md` together,
   then re-run guard. This lane performed no commit.
3. y-caveat acceptance calls (owner: controller + test owners). AUTO-004/006 tokio card amendment or
   tokio lane; EXT-005 keep-or-revert follow-up commit; PROV-016 freeze attestation of committed suites;
   INT/OPS/SHARE quiesced-tree RED re-runs; WEB-007..012 write-paths decision; WEB-013/015 executor/
   membership future lanes (NOT ACCEPTED stands independent of 9/9 + 10/10 boundary evidence).
4. Twin-drop execution (owner: integrator). WATCH6: 0/13 body drift; pair-1 guard-ported both sides,
   droppable like 2–8 AFTER §5 `plugin_transform` 5/5 re-run, same follow-up commit. SHARE keep-both,
   no action. fmt steady at 58 (post-commit debt, not growth).
5. Routine quiesced-tree verifier re-runs (owner: verifier + controller). §5 serial mandates
   (stream, ext_manifest_lane, PROV-016 triple, WEB lift triples, REL bF/cC/ACC-h) + zB-cargo upgrade
   + INT/OPS/SHARE RED-half re-runs. Evidence complete (§4/§6 + ACC a–h on committed tree); re-runs are
   acceptance formalities, not discovery.
