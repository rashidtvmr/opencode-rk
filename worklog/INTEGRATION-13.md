# INTEGRATION-13 — per-slice GREEN matrix + RED receipt inventory + wiring + flip table (v13)

Rev: `248f519` HEAD. Date: 2026-09-16.
Bounds: READ-ONLY except this file. No ralph.json/controller/product edits. No fmt run. No commit.
Supersedes INTEGRATION-12 where disk disagrees (marked CORR).
Inputs: `worklog/INTEGRATION-12.md`, `worklog/GUARD-TRIAGE-13.md` (latest on disk),
`worklog/TURN-STREAM-GATE.md`, `worklog/RED-VALIDITY-INT-FINAL.md`,
`worklog/RED-VALIDITY-OPS-FINAL.md`, `worklog/RED-VALIDITY-WEB-PROV.md`,
`worklog/RED-VALIDITY-WEB013-015.md`, `worklog/RED-VALIDITY-PROV015-016.md`,
`RED-VALIDITY-EXT1C/EXT2C/TOOL3/ACPSDK2/AUTO3.md`,
`worklog/EXT-TWINS-DISPOSITION.md`, `worklog/SHARE-TWINS-DISPOSITION.md`,
`worklog/AUTO-TOKIO-BROKER.md`, on-disk diffs, `ralph.json` (read-only).

Guard (GUARD-TRIAGE-13, no re-run this lane): validate_repository exit 1,
122 errors (0 fixed / 0 new, pre-existing); validate_plan exit 1, 133 = 122 + 11
unknown prefixes (ACP/HEAD/RUN/SDK/SYNC/WSX); `git diff --check` exit 0.
`ralph.json`: 258 stories, 176 accepted / 44 in-progress / 38 not-started
(re-verified this lane via python count; unchanged vs v8–v12). NOT edited.

## 1. Stub / ignore scan (this lane)

- Strict grep: `grep -rn --include='*.rs' -e 'todo!' -e 'unimplemented!' -e '#\[ignore\]' crates/`
  → **0 hits** (exit 1, empty). Log: `/tmp/opencode/yQ-stub.log` (0 lines). CLEAN.
- Loose `ignore` matches remain only known-benign (per v8–v12 §1). No action.

## 2. Fmt + status (this lane)

- `cargo fmt --check 2>&1 | grep -c 'Diff in'` → **48** (flat vs v10/v11/v12 = 48).
- `git status --porcelain`: **204 M + 217 ?? (wc -l 420, awk/wc rounding)**.
  CORR vs INTEGRATION-12 (204 M + 205 ?? = 409) = **0 tracked, +12 untracked**.
  CORR vs GUARD-TRIAGE-13 (204 M + 205 ?? = 409) = same +12 untracked drift.
  Drift source: worklogs only; no new product files this wave; no controller files.
- No `cargo fmt` executed (write-shaped op banned this lane).

## 3. Wiring checklist (on-disk truth, re-verified this lane)

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
| tools | `crates/tools/src/plugin_transform.rs` | GUARD-PORT-landed, uncommitted (unchanged) | +9/-2: unknown-scope guard + invariant docs (§3a) | EXT-TWINS-DISPOSITION pair-1 | needs canonical 5/5 re-run post-port |

- 6 lib.rs files differ (same set as v8–v12 §3). Storage hunk still +18/-9
  pure rewrap; still unattributed: single-writer unverified.
  Integrator must attribute or revert before commit.
- Providers 9 unwired left to integrator on acceptance (YAGNI, no consumer).
- `codex_oauth.rs` + its frozen test: `git diff` empty (clean, unchanged) — no
  bounds fix, no test addition landed (§3b).

### 3a. Guard-port status (EXT pair-1: twin → canonical)

- Disposition order (EXT-TWINS-DISPOSITION §pair-1): port twin's 1-line
  unknown-scope guard + invariant docs into `plugin_transform` first, then drop twin.
- Status: **PORT STILL LANDED in worktree, uncommitted, UNCHANGED vs v10/v11/v12.**
  `plugin_transform.rs` numstat 9/2: `set_scope_disabled` guards
  `!disabled.contains(&scope) && entries.iter().any(|t| t.scope == scope)`;
  `TransformLog` + method carry bounded-`disabled` invariant docs.
- Remaining before drop: re-run canonical `plugin_transform` 5/5 GREEN on ported
  bytes (serial mandate §6); then integrator drops `ext_replay_lane`
  src+test in same commit (`#[path]` coupling: twin test breaks iff src deleted).
- Pairs 2–8: NO port applied this lane (drops pending integrator; pair 5/6/7 need
  1-line `#![forbid(unsafe_code)]` port at drop time per disposition).

### 3b. Codex-bounds status (PROV-016 B3 validators: OPEN, no fix applied)

- Open gaps per RED-VALIDITY-PROV015-016 §PROV-016: `validate_device_code`
  (empty + `>MAX_DEVICE_CODE_BYTES`), `validate_consent_url` length cap
  (`>MAX_CONSENT_URL_BYTES`), zero-expiry rejection (`complete_login` + `refresh`)
  are dead enforcement from the suite's view — B3/B3alt/B3exp stubs stay 5/5 GREEN.
- Additive proposal (T06 device-code bounds / T07 URL length cap / T08 zero-expiry)
  NOT applied — verifier/test-owner authority. Slice stays flippable for lifecycle
  only; boundary-hardening flip needs new frozen tests.

## 4. Per-slice GREEN matrix — this wave (latest receipt per slice wins)

Serial discipline all waves: one cargo cmd at a time, `timeout 120`, `rtk` prefix,
`--test-threads=1` unless noted, restores `cmp`/`diff -q` identical, frozen
tests + lib.rs + ralph.json untouched. No cargo runs by THIS lane (read-only rollup).

| Slice | Wave receipt | GREEN after restore | RED bite shape |
|---|---|---|---|
| AUTO-004 (delegation_lane) | RED-VALIDITY-AUTO3 (xA) | 5/5 (`xA-AUTO-004-green.log`) | 3p/2f (T02,T03 owner-detach) |
| AUTO-006 (driver_lane) | RED-VALIDITY-AUTO3 (xA) | 5/5 (`xA-AUTO-006-green.log`) | 4p/1f (T02 non-owner release) |
| AUTO-005 (delegation_gated + tool) | RED-VALIDITY-AUTO3 (xA) | 5/5 + tool PASS exit 0 + 3 mutated exit 2 | 0p/5f (inverted broker) |
| agents full crate | RED-VALIDITY-AUTO3 (xA) | 35/35 (lib 15 + gated 5 + lane 5 + driver 5 + turn 5) | — |
| ACP-001 | RED-VALIDITY-ACPSDK2 (xB) | 5/5 | 3p/2f (T01,T02 frame newline) |
| ACP-002 | RED-VALIDITY-ACPSDK2 (xB) | 5/5 | 3p/2f (T03,T05 Terminal gate) |
| WSX-001 | RED-VALIDITY-ACPSDK2 (xB) | 5/5 | 4p/1f (T02 BadEndpoint) |
| WSX-002 | RED-VALIDITY-ACPSDK2 (xB) | 5/5 | 4p/1f (T02 backoff table) |
| SDK-001 | RED-VALIDITY-ACPSDK2 (xB) | 12/12 | 6p/6f (base/determinism/rewrite/redaction) |
| SDK-002 | RED-VALIDITY-ACPSDK2 (xB) | 11/11 | 1p/10f (spawn short-circuit) |
| HEAD-001 | RED-VALIDITY-ACPSDK2 (xB) | 8/8 | 2p/6f (exit code) |
| HEAD-002 | RED-VALIDITY-ACPSDK2 (xB) | 8/8 | 7p/1f (T04 secret leak) |
| RUN-001 | RED-VALIDITY-ACPSDK2 (xB) | 5/5 | 0p/5f (start short-circuit) |
| EXT-001 | RED-VALIDITY-EXT1C (xC) | 5/5 | 4p/1f (T03 dup guard) |
| EXT-002 | RED-VALIDITY-EXT1C (xC) | 5/5 | 4p/1f (T01 caps) |
| EXT-004 | RED-VALIDITY-EXT1C (xC) | 5/5 | 3p/2f (T01,T04 reload-kind) |
| EXT-005 | RED-VALIDITY-EXT1C (xC) | 5/5 | 3p/2f (T01,T04 byte cap) |
| EXT-006 | RED-VALIDITY-EXT1C (xC) | 5/5 | 4p/1f (T03 broker-deny) |
| EXT-008 | RED-VALIDITY-EXT1C (xC) | 5/5 | 4p/1f (T03 dup guard) |
| EXT-009 | RED-VALIDITY-EXT2C (xD) | 5/5 | 0p/5f full-fail |
| EXT-010 | RED-VALIDITY-EXT2C (xD) | 5/5 | 1p/4f (T04 negative-path survives) |
| EXT-011 | RED-VALIDITY-EXT2C (xD) | 5/5 | 0p/5f full-fail |
| EXT-012 | RED-VALIDITY-EXT2C (xD) | 5/5 | 0p/5f full-fail |
| TOOL-016 | RED-VALIDITY-TOOL3 (xJ) | 5/5 | 1p/4f (T05 empty-path survives) |
| TOOL-017 | RED-VALIDITY-TOOL3 (xJ) | 5/5 | 1p/4f (T02 selection-only survives) |
| TOOL-018 | RED-VALIDITY-TOOL3 (xJ) | 5/5 | 4p/1f (T01 ready gate) |
| TOOL-019 | RED-VALIDITY-TOOL3 (xJ) | 5/5 | 1p/4f (T04 validation-only survives) |
| TOOL-020 | RED-VALIDITY-TOOL3 (xJ) | 5/5 | 3p/2f (T01,T04) |
| UI-019 | RED-VALIDITY-TOOL3 (xJ) | 5/5 | 1p/4f (T04 gate fires pre-stub) |
| SYNC-002 | RED-VALIDITY-TOOL3 (xJ) | 5/5 | 3p/2f (T01,T04) |
| SYNC-001 | RED-VALIDITY-TOOL3 (xJ) | 5/5 | 0p/5f full-fail |
| WEB-013 (sessions entry-point) | RED-VALIDITY-WEB-PROV (s3) | 5/5 | 0p/5f (adapter gate) |
| WEB-014 | RED-VALIDITY-WEB-PROV (s3) | 5/5 | 3p/2f (T01,T04) |
| WEB-015 (sessions entry-point) | RED-VALIDITY-WEB-PROV (s3) | 5/5 | 0p/5f (ghost id) |
| WEB-016 | RED-VALIDITY-WEB-PROV (s3) | 5/5 | 1p/4f (T02 neg-path survives) |
| WEB-017 | RED-VALIDITY-WEB-PROV (s3) | 5/5 | 1p/4f (T02 gate survives) |
| PROV-015 | RED-VALIDITY-PROV015-016 (uD A1/A2/A3) | 5/5 | 0/5, 4/1, 4/1 — all bite |
| PROV-016 lifecycle | RED-VALIDITY-PROV015-016 (uD B1/B2) | 5/5 | 0/5, 4/1 — bite |
| PROV-016 boundary | RED-VALIDITY-PROV015-016 (uD B3/B3alt/B3exp) | 5/5 GREEN-ON-STUB | **NO BITE 5/5 ×3 — OPEN (§3b)** |
| WEB-013 server boundary | RED-VALIDITY-WEB013-015 (uC) | 1/1 | flag-flip FAIL, missing-key FAIL; reason-text-only PASSES |
| WEB-015 server boundary | RED-VALIDITY-WEB013-015 (uC) | 2/2 | 3/3 stub strategies bite |
| OPS-001..009 | RED-VALIDITY-OPS-FINAL (uB epoch) | 5/5 each (9/9) | FAILED ×9 (2× partial: OPS-001 3/2, OPS-003 2/3), 0 compile-fail |
| INT-001/002/003/005/006/007/009/010 | RED-VALIDITY-INT-FINAL (uA retry) | 5/5 each (8/8) | 2/3, 0/5, 3/2, 3/2, 4/1, 4/1, 2/3, 0/5 |
| session_turn_stream_api | TURN-STREAM-GATE (3x serial) | 2/2 ×3 runs | n/a (harness race, not behavior RED) |
| EXT002-T05 | EXT002-T05-DETERMINISM | serial 30/30 | flaky-by-construction; NO-FIX |
| WEB-007..012 | WEB-007-012-ACCEPT | 5/5 per suite | none — GREEN-only HOLD |
| EXT twins (8 pairs) | EXT-TWINS-DISPOSITION (uM 35/35) | 35/35 7-suite | pair-1 guard PORTED (§3a); pairs 2–8 drop-pending |
| SHARE twins (5 pairs) | SHARE-TWINS-DISPOSITION | survey only | keep-both everywhere |
| REL-001..003 | lane worklogs only | lane GREEN | none — validator-only, verifier re-run pending |
| PROV-017..024 | none this lane | one-line fix in worktree, uncommitted | re-run `prov_017` (+018..024) first |
| WEB-001..006 | prior worklogs | unchanged | none this lane |

## 5. RED receipt inventory (which waves cover which IDs)

In-repo receipt lives in `worklog/`; per-test logs in `/tmp/opencode` (ephemeral).
No RED receipts live in `crates/` (by design — frozen tests untouched).

| Wave file | Covers | Verdict |
|---|---|---|
| `RED-VALIDITY-AUTO3.md` (xA-*-red/green.log, agents-full-green) | AUTO-004/006/005 | **RED-VALID (strong, behavior). MECHANISM CAVEAT: AUTO-TOKIO-BROKER finds tokio REQUIRED by card but NOT MET (sync bool-flag cancel passes T04 vacuously); broker mirror SUFFICES. Card amendment OR integration lane + controller dep approval.** |
| `RED-VALIDITY-ACPSDK2.md` (xB-*-red/green.log) | ACP-001/002, WSX-001/002, SDK-001/002, HEAD-001/002, RUN-001 | **RED-VALID (strong, behavior)** |
| `RED-VALIDITY-EXT1C.md` (xC-*-red.log) | EXT-001/002/004/005/006/008 | **RED-VALID (strong, behavior)** |
| `RED-VALIDITY-EXT2C.md` (xD-*-red/green.log) | EXT-009/010/011/012 | **RED-VALID (strong, behavior)** |
| `RED-VALIDITY-TOOL3.md` (xJ-*-red/green.log) | TOOL-016..020, UI-019, SYNC-001/002 | **RED-VALID (strong, behavior)** |
| `RED-VALIDITY-WEB-PROV.md` (s3-*-red/green.log, bak-*.rs) | WEB-013/014/015/016/017 + PROV-015/016 entry-point | **RED-VALID (strong, entry-point). QUALIFIED by uC/uD rows — entry-point verdict stands, boundary precision does NOT extend.** |
| `RED-VALIDITY-WEB013-015.md` (uC-attempt logs) | WEB-013/015 server boundary | **WEAK-RED (partial pinning). Unavailable/registry-metadata = YES pinned; reason-string precision + full T01–T05 = NOT pinned. Additive gap probes proposed (NEW-FILE, not applied).** |
| `RED-VALIDITY-PROV015-016.md` (uD-a/b logs) | PROV-015 (strong) / PROV-016 (lifecycle pins, boundary OPEN) | **PROV-015 RED-VALID; PROV-016 WEAK-RED. Codex-bounds OPEN (§3b).** |
| `RED-VALIDITY-OPS-FINAL.md` (PRIMARY; verifies OPS2; uB-*-red.log 9/9) | OPS-001..009 | **RED-VALID (strong, behavior). Caveat: tree dirty on entry — wiring proven on CURRENT content, NOT pristine-commit validity. Re-do on quiesced tree before acceptance flip.** |
| `RED-VALIDITY-INT-FINAL.md` (PRIMARY; uA-INT*-red.log 8/8 + green1/2) | INT-001/002/003/005/006/007/009/010 | **RED-VALID (strong, behavior). Same dirt caveat as OPS: 6/8 src files carried fmt-only hunks at stub time; restore verified IDENTICAL. Frozen tests/lib.rs/ralph.json never touched.** |
| `TURN-STREAM-GATE.md` (stream_serial{1,2,3}) | session_turn_stream_api | serial-mandate only (§6) |
| Superseded epochs (detail only, NOT primary) | EXT1/EXT1B, EXT2, AUTO/AUTO2, TOOL/TOOL2, ACPSDK, OPS/OPS2, INT/INT2/INT3, WEB-007-012-ACCEPT | retained for archaeology; latest wave above wins on conflict |

## 6. Serial-test mandates (binding on verifier/CI)

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
  impl deterministic (zero spawns). Parallel default stays flaky; serial 30/30
  GREEN. Treat parallel `left/right in 1..5` line-175 failures as harness noise.
- Do NOT run either binary `--test-threads=N>1`; do NOT parallelize with heavy
  jobs (8 GiB budget, one validation at a time). Do NOT edit frozen tests.
- `env_lock` proposal TURN-STREAM-GATE §6, T05 options EXT002-T05 §5: NOT applied —
  verifier/test-owner authority.

## 7. ralph flip table — all 82 non-accepted IDs (EVIDENCE ONLY — do NOT edit)

ready = behavior-RED receipt complete + GREEN on disk (wiring-uncommitted does not
block evidence readiness). `caveat` = acceptance still blocked (see note).
Counts: **ready-y 34 / partial 3 / not-ready 45** (total 82 = 44 in-progress + 38 not-started).

| # | Story | ralph now | ready | Evidence path / note |
|---|---|---|---|---|
| 1 | AUTO-004 | in-progress | y-caveat | AUTO3 xA RED 3/2 + GREEN 5/5; CAVEAT tokio-mechanism (card amendment or integration lane, AUTO-TOKIO-BROKER) |
| 2 | AUTO-005 | in-progress | y | AUTO3 xA RED 0/5 + GREEN 5/5 + tool PASS/mutated-exit-2 |
| 3 | AUTO-006 | in-progress | y-caveat | AUTO3 xA RED 4/1 + GREEN 5/5; same tokio-pool caveat as AUTO-004 |
| 4 | EXT-001 | in-progress | y | EXT1C xC RED 4/1 + GREEN 5/5 |
| 5 | EXT-002 | in-progress | y | EXT1C xC RED 4/1 + GREEN 5/5 |
| 6 | EXT-004 | in-progress | y | EXT1C xC RED 3/2 + GREEN 5/5 |
| 7 | EXT-005 | in-progress | y | EXT1C xC RED 3/2 + GREEN 5/5 |
| 8 | EXT-006 | in-progress | y | EXT1C xC RED 4/1 + GREEN 5/5 |
| 9 | EXT-008 | in-progress | y | EXT1C xC RED 4/1 + GREEN 5/5 |
| 10 | EXT-009 | in-progress | y | EXT2C xD RED 0/5 + GREEN 5/5 |
| 11 | EXT-010 | in-progress | y | EXT2C xD RED 1/4 + GREEN 5/5 |
| 12 | EXT-011 | in-progress | y | EXT2C xD RED 0/5 + GREEN 5/5 |
| 13 | EXT-012 | in-progress | y | EXT2C xD RED 0/5 + GREEN 5/5 |
| 14 | INT-001 | in-progress | n | INT-FINAL uA RED 2/3 + GREEN 5/5 BUT dirty-tree caveat — re-run on quiesced tree before accept-flip |
| 15 | INT-002 | in-progress | n | INT-FINAL uA RED 0/5 + GREEN 5/5 BUT dirty-tree caveat, same |
| 16 | INT-003 | in-progress | n | INT-FINAL uA RED 3/2 + GREEN 5/5 BUT dirty-tree caveat, same |
| 17 | INT-005 | in-progress | n | INT-FINAL uA RED 3/2 + GREEN 5/5 BUT dirty-tree caveat, same |
| 18 | INT-006 | in-progress | n | INT-FINAL uA RED 4/1 + GREEN 5/5 BUT dirty-tree caveat, same |
| 19 | INT-007 | in-progress | n | INT-FINAL uA RED 4/1 + GREEN 5/5 BUT dirty-tree caveat, same |
| 20 | INT-009 | in-progress | n | INT-FINAL uA RED 2/3 + GREEN 5/5 BUT dirty-tree caveat, same |
| 21 | INT-010 | in-progress | n | INT-FINAL uA RED 0/5 + GREEN 5/5 BUT dirty-tree caveat, same |
| 22 | OPS-001 | in-progress | n | OPS-FINAL uB RED 3/2 + GREEN 5/5 BUT dirty-tree caveat — re-run on quiesced tree before accept-flip |
| 23 | OPS-002 | in-progress | n | OPS-FINAL uB RED 0/5 + GREEN 5/5 BUT dirty-tree caveat, same |
| 24 | OPS-003 | in-progress | n | OPS-FINAL uB RED 2/3 + GREEN 5/5 BUT dirty-tree caveat, same |
| 25 | OPS-004 | in-progress | n | OPS-FINAL uB RED 0/5 + GREEN 5/5 BUT dirty-tree caveat, same |
| 26 | OPS-005 | in-progress | n | OPS-FINAL uB RED 0/5 + GREEN 5/5 BUT dirty-tree caveat, same |
| 27 | OPS-006 | in-progress | n | OPS-FINAL uB RED 0/5 + GREEN 5/5 BUT dirty-tree caveat, same |
| 28 | OPS-007 | in-progress | n | OPS-FINAL uB RED 0/5 + GREEN 5/5 BUT dirty-tree caveat, same |
| 29 | OPS-008 | in-progress | n | OPS-FINAL uB RED 0/5 + GREEN 5/5 BUT dirty-tree caveat, same |
| 30 | OPS-009 | in-progress | n | OPS-FINAL uB RED 0/5 + GREEN 5/5 BUT dirty-tree caveat, same |
| 31 | REL-001 | in-progress | n | validator-only, lane GREEN, verifier re-run pending (INTEGRATION-5 §3) |
| 32 | REL-002 | in-progress | n | same as REL-001 |
| 33 | REL-003 | in-progress | n | same as REL-001 |
| 34 | SHARE-001 | in-progress | n | survey only (SHARE-TWINS-DISPOSITION); no RED receipt; redaction writer owns merge/queue |
| 35 | SHARE-002 | in-progress | n | same as SHARE-001 |
| 36 | SHARE-003 | in-progress | n | same as SHARE-001 |
| 37 | SHARE-004 | in-progress | n | same as SHARE-001 |
| 38 | SHARE-005 | in-progress | n | same as SHARE-001 |
| 39 | WEB-001 | in-progress | n | prior worklogs; no RED receipt this wave |
| 40 | WEB-002 | in-progress | n | same as WEB-001 |
| 41 | WEB-003 | in-progress | n | same as WEB-001 |
| 42 | WEB-004 | in-progress | n | same as WEB-001 |
| 43 | WEB-005 | in-progress | n | same as WEB-001 |
| 44 | WEB-006 | in-progress | n | prior worklogs; unchanged |
| 45 | WEB-007 | not-started | n | GREEN-only HOLD (WEB-007-012-ACCEPT); write-paths disabled, no RED by design |
| 46 | WEB-008 | not-started | n | same as WEB-007 |
| 47 | WEB-009 | not-started | n | same as WEB-007 |
| 48 | WEB-010 | not-started | n | same as WEB-007 |
| 49 | WEB-011 | not-started | n | same as WEB-007 |
| 50 | WEB-012 | not-started | n | same as WEB-007 |
| 51 | WEB-013 | not-started | partial | entry-point RED-VALID (s3 0/5) + server boundary WEAK-RED (uC: flags/keys pin, reason-strings + full T01–T05 do NOT); controller decides flip to in-progress |
| 52 | WEB-014 | not-started | y | entry-point RED-VALID (s3 3/2) + GREEN 5/5; controller decides flip to in-progress |
| 53 | WEB-015 | not-started | partial | entry-point RED-VALID (s3 0/5) + server boundary WEAK-RED (uC 3/3 registry-metadata pins, full contract does NOT); controller decides flip |
| 54 | WEB-016 | not-started | y | entry-point RED-VALID (s3 1/4) + GREEN 5/5; controller decides flip |
| 55 | WEB-017 | not-started | y | entry-point RED-VALID (s3 1/4) + GREEN 5/5; controller decides flip |
| 56 | PROV-015 | not-started | y | RED-VALID strong (uD A1/A2/A3 all bite) + GREEN 5/5; controller decides flip |
| 57 | PROV-016 | not-started | partial | lifecycle pins (uD B1/B2) BUT boundary validators non-pinning (§3b); T06–T08 tests needed before full flip |
| 58 | PROV-017 | not-started | n | HOLD — one-line fix in worktree (uncommitted); re-run `prov_017` (+018..024) first |
| 59 | PROV-018 | not-started | n | same gate as PROV-017 |
| 60 | PROV-019 | not-started | n | same gate as PROV-017 |
| 61 | PROV-020 | not-started | n | same gate as PROV-017 |
| 62 | PROV-021 | not-started | n | same gate as PROV-017 |
| 63 | PROV-022 | not-started | n | same gate as PROV-017 |
| 64 | PROV-023 | not-started | n | same gate as PROV-017 |
| 65 | PROV-024 | not-started | n | same gate as PROV-017 |
| 66 | UI-019 | not-started | y | TOOL3 xJ RED 1/4 + GREEN 5/5; review → in-progress? |
| 67 | TOOL-016 | not-started | y | TOOL3 xJ RED 1/4 + GREEN 5/5; review → in-progress? |
| 68 | TOOL-017 | not-started | y | TOOL3 xJ RED 1/4 + GREEN 5/5; review → in-progress? |
| 69 | TOOL-018 | not-started | y | TOOL3 xJ RED 4/1 + GREEN 5/5; review → in-progress? |
| 70 | TOOL-019 | not-started | y | TOOL3 xJ RED 1/4 + GREEN 5/5; review → in-progress? |
| 71 | TOOL-020 | not-started | y | TOOL3 xJ RED 3/2 + GREEN 5/5; review → in-progress? |
| 72 | SYNC-001 | not-started | y | TOOL3 xJ RED 0/5 full-fail + GREEN 5/5; review → in-progress? |
| 73 | SYNC-002 | not-started | y | TOOL3 xJ RED 3/2 + GREEN 5/5; review → in-progress? |
| 74 | RUN-001 | not-started | y | ACPSDK2 xB RED 0/5 + GREEN 5/5, WIRED-uncommitted §3, gate pending |
| 75 | ACP-001 | not-started | y | ACPSDK2 xB RED 3/2 + GREEN 5/5, WIRED-uncommitted §3 |
| 76 | ACP-002 | not-started | y | ACPSDK2 xB RED 3/2 + GREEN 5/5, WIRED-uncommitted §3 |
| 77 | WSX-001 | not-started | y | ACPSDK2 xB RED 4/1 + GREEN 5/5, WIRED-uncommitted §3 |
| 78 | WSX-002 | not-started | y | ACPSDK2 xB RED 4/1 + GREEN 5/5, WIRED-uncommitted §3 |
| 79 | SDK-001 | not-started | y | ACPSDK2 xB RED 6/6 + GREEN 12/12, WIRED-uncommitted §3 |
| 80 | SDK-002 | not-started | y | ACPSDK2 xB RED 1/10 + GREEN 11/11, WIRED-uncommitted §3 |
| 81 | HEAD-001 | not-started | y | ACPSDK2 xB RED 2/6 + GREEN 8/8, NO wiring needed §3 |
| 82 | HEAD-002 | not-started | y | ACPSDK2 xB RED 7/1 + GREEN 8/8, NO wiring needed §3 |

Plus (not stories, controller-owned): unknown prefixes SYNC/RUN/ACP/WSX/SDK/HEAD →
allowlist-or-map + reconcile accounting (plan validator 11 extra); FEATURES stale-accepted
set → FEATURES.md sync only (guard log 122 lines: 69 stale-mirror + 53 ledger/gap).
EXT twins pairs 2–8 → integrator executes drops per §3a AFTER pair-1 GREEN re-run.

## 8. Ranked blockers top 5 (owner lanes)

1. Wired-but-uncommitted + ungated + unattributed lib.rs drift + LANDED guard-port
   un-gated (owner: wiring lanes + verifier + integrator).
   Server +20, sessions +3, foundation +3 mods in worktree, zero committed.
   security/storage/tools lib.rs reorder/fmt-only diffs still no owning lane
   (storage +18/-9) — attribute or revert before the integration commit.
   `plugin_transform` guard-port (+9/-2) needs canonical 5/5 re-run, then twin drop.
   Action: serial lane-gate GREEN per suite (§6 mandates for
   `session_turn_stream_api` + `ext_builtins_lane`), then ONE integration commit.
2. Guard FAIL backlog exhaustion (owner: controller). 122 errors exit 1,
   byte-identical sets vs TRIAGE-13. 69 FEATURES sync + 53 ledger/gap; no lane edits ralph.json.
3. Dirty-tree RED caveat on INT + OPS + frozen-test reformats (owner: INT/OPS lanes + verifier).
   Behavior RED now FINAL-receipted for both (OPS-FINAL 9/9 + INT-FINAL 8/8),
   but logged against CURRENT dirty bytes, not pristine commit; frozen tests carry
   uncommitted rewraps. Neither slice accept-flippable until quiesced-tree re-run.
   Verifier must read FINAL files as the receipts, not wait on further files.
4. Codex-bounds + WEB-013/015 boundary gaps OPEN (owner: providers/server lanes + test owner).
   PROV-016 B3-class (device-code / URL-length / zero-expiry) non-pinning; WEB-013
   reason-string + full T01–T05, WEB-015 full contract non-pinning by absence.
   Additive T06–T08 / gap-probe patches proposed but NOT applied (frozen-test waiver needed).
   Lifecycle-only flips possible; boundary-hardening flips blocked.
5. PROV-017 fix gate + fmt/untracked drift + AUTO-004 tokio gap (owner: providers lane +
   verifier + integrator + controller).
   One-line worktree fix per INTEGRATION-5 §2; re-run `prov_017` (+018..024) first.
   `cargo fmt --check` 48 (flat vs v10/v11/v12); status 409→420 (+12 untracked worklogs, CORR).
   AUTO-004 needs card amendment (sync state machine) or tokio integration lane —
   controller dep approval either way.

Merge order: (i) attribute/revert §3 unattributed lib.rs drift; (ii) canonical
`plugin_transform` 5/5 re-run on ported bytes → drop `ext_replay_lane` same commit
(§3a); (iii) quiesce INT/OPS owned paths + frozen tests → INT/OPS re-run (§4);
(iv) T06–T08 codex-bounds tests + gap probes ONLY with test-owner waiver (§3b/§5);
(v) lane gates GREEN per wired suite (serial mandates §6); (vi) refresh stale status
files; (vii) ONE integration commit (tracked fixes + lib.rs wirings, fmt at commit);
(viii) untracked modules only with passing gates; (ix) controller syncs
FEATURES.md/accounting + flips per §7.
