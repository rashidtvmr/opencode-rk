# INTEGRATION-14 — per-slice GREEN matrix + RED receipt inventory + wiring + flip table (v14)

Rev: `248f519` HEAD. Date: 2026-09-16.
Bounds: READ-ONLY except this file. No ralph.json/controller/product edits. No fmt run. No commit.
Supersedes INTEGRATION-13 where disk disagrees (marked CORR).
Inputs: `worklog/INTEGRATION-13.md`, `worklog/GUARD-TRIAGE-14.md` (latest on disk),
`worklog/TURN-STREAM-GATE.md`, y-wave worklogs `RED-VALIDITY-AUTO4.md`,
`ACPSDK-VERIFY3.md`, `EXT1-VERIFY4.md`, `EXT2-VERIFY4.md`, `INT-VERIFY4.md`,
`OPS-VERIFY4.md`, `PROV-VERIFY4.md`, `TOOL-VERIFY4.md`, `WEB-VERIFY4.md`,
`SHARE-DEDUP4.md`, `worklog/EXT-TWINS-DISPOSITION.md`,
`worklog/SHARE-TWINS-DISPOSITION.md`, on-disk diffs, `ralph.json` (read-only).

Guard (GUARD-TRIAGE-14, no re-run this lane): validate_repository exit 1,
122 errors (122 bullets); validate_plan exit 1, 133 = 122 + 11 unknown prefixes
(ACP/HEAD/RUN/SDK/SYNC/WSX); `git diff --check` exit 0; status 421 lines
(204 M + 217 ??). Forbidden paths untouched. GUARD RED — no integration commit
until backlog-exhaustion ledger reconciled.
`ralph.json`: 258 stories, 176 accepted / 44 in-progress / 38 not-started
(re-verified this lane via python count on `userStories.status`; unchanged vs
v8–v13). NOT edited.

## 1. Stub / ignore scan (this lane)

- Strict grep: `grep -rn --include='*.rs' -e 'todo!' -e 'unimplemented!' -e '#\[ignore\]' crates/`
  → **0 hits** (exit 1, empty). Log: `/tmp/opencode/zO-stub.log` (0 lines). CLEAN.
- (First invocation exited 0 only via `rtk|tee` pipe masking; direct rerun exit 1, 0 lines.)
- Loose `ignore` matches remain only known-benign (per v8–v13 §1). No action.

## 2. Fmt + status (this lane)

- `cargo fmt --check 2>&1 | grep -c 'Diff in'` → **48** (flat vs v10–v13 = 48).
- `git status --porcelain`: **204 M + 229 ?? (wc -l 432; awk TOT 433, filename-space rounding)**.
  CORR vs INTEGRATION-13 (204 M + 217 ?? ≈ 420) = **0 tracked, +12 untracked**.
  CORR vs GUARD-TRIAGE-14 (204 M + 217 ?? = 421) = same +12 untracked drift.
  Drift source: worklogs only; no new product files this wave; no controller files.
- `git diff HEAD --stat -- crates/`: 203 files, +2070/-1232 (fmt churn + landed ports/fixes).
- No `cargo fmt` executed (write-shaped op banned this lane).

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
| tools | `crates/tools/src/plugin_transform.rs` | GUARD-PORT-landed, uncommitted (unchanged) | +9/-2: unknown-scope guard + invariant docs (§3a) | EXT-TWINS-DISPOSITION pair-1 | EXT2-VERIFY4 hash-check PASS (b4c517e3); canonical 5/5 re-run still mandated §6 |
| providers | `crates/providers/src/claude_oauth.rs` | FIX-landed, uncommitted (PROV-017 gate CLEARED §3b) | +1/-1: raw `{LOOPBACK_REDIRECT_URI}` at :462 (was percent-encoded literal) | PROV-VERIFY4 | 11 suites 56/56 GREEN |
| providers | `crates/providers/src/codex_oauth.rs` | clean | `git diff` empty (unchanged) — no bounds fix, no test addition landed (§3c) | none | none |

- 6 lib.rs files differ (same set as v8–v13 §3). Storage hunk still +18/-9
  pure rewrap; still unattributed: single-writer unverified.
  Integrator must attribute or revert before commit.
- Providers 9 unwired left to integrator on acceptance (YAGNI, no consumer).

### 3a. Guard-port status (EXT pair-1: twin → canonical)

- Disposition order (EXT-TWINS-DISPOSITION §pair-1): port twin's 1-line
  unknown-scope guard + invariant docs into `plugin_transform` first, then drop twin.
- Status: **PORT STILL LANDED in worktree, uncommitted, UNCHANGED vs v10–v13.**
  EXT2-VERIFY4 confirms hash `b4c517e3` = guard-port present (`set_scope_disabled`
  unknown-scope no-op guard + invariant doc) and 5/5 GREEN on ported bytes.
- Remaining before drop: serial-mandate re-run of canonical `plugin_transform`
  5/5 logged by verifier (§6); then integrator drops `ext_replay_lane`
  src+test in same commit (`#[path]` coupling: twin test breaks iff src deleted).
- Pairs 2–8: NO port applied this lane (drops pending integrator; pair 5/6/7 need
  1-line `#![forbid(unsafe_code)]` port at drop time per disposition).

### 3b. PROV-017 fix status (GATE CLEARED this wave)

- PROV-VERIFY4: fix PRESENT in worktree (`claude_oauth.rs:462` emits raw
  `{LOOPBACK_REDIRECT_URI}`, passes own `validate_consent_url`); 11 suites
  56/56 GREEN (`/tmp/opencode/yG-prov.log`), hashes match GATE3.
- PROV-017..024 flip gate from INTEGRATION-13 §7 (re-run `prov_017` +018..024)
  is SATISFIED. Zero edits by verify lane; fix pre-existing from prior lane.

### 3c. Codex-bounds status (PROV-016 B3 validators: OPEN, no fix applied)

- Open gaps per RED-VALIDITY-PROV015-016 §PROV-016: `validate_device_code`
  (empty + `>MAX_DEVICE_CODE_BYTES`), `validate_consent_url` length cap
  (`>MAX_CONSENT_URL_BYTES`), zero-expiry rejection (`complete_login` + `refresh`)
  are dead enforcement from the suite's view — B3/B3alt/B3exp stubs stay 5/5 GREEN.
- Additive proposal (T06 device-code bounds / T07 URL length cap / T08 zero-expiry)
  NOT applied — verifier/test-owner authority. Slice stays flippable for lifecycle
  only; boundary-hardening flip needs new frozen tests.

## 4. Per-slice GREEN matrix — this wave (latest receipt per slice wins)

Serial discipline all waves: one cargo cmd at a time, `timeout 120`, `rtk` prefix,
`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`, `--test-threads=1` unless noted, restores
`cmp`/`diff -q` identical, frozen tests + lib.rs + ralph.json untouched.
No cargo runs by THIS lane (read-only rollup). y-wave = verify-only reruns at
rev `248f519` on current worktree bytes.

| Slice | Wave receipt | GREEN after restore | RED bite shape |
|---|---|---|---|
| AUTO-004 (delegation_lane) | RED-VALIDITY-AUTO4 (yA) | 5/5 (`yA-delegation_lane.log`) + full crate 35/35 | prior xA: 3p/2f (T02,T03 owner-detach) |
| AUTO-006 (driver_lane) | RED-VALIDITY-AUTO4 (yA) | 5/5 (`yA-driver_lane.log`) | prior xA: 4p/1f (T02 non-owner release) |
| AUTO-005 (delegation_gated + tool) | RED-VALIDITY-AUTO4 (yA) | 5/5 + tool PASS exit 0 + 3 mutated exit 2 | prior xA: 0p/5f (inverted broker) |
| agents full crate | RED-VALIDITY-AUTO4 (yA) | 35/35 (lib 15 + gated 5 + lane 5 + driver 5 + turn 5) | — |
| ACP-001 | ACPSDK-VERIFY3 (yB) | 5/5 (`yB-acp_bridge.log`) | prior xB: 3p/2f (T01,T02 frame newline) |
| ACP-002 | ACPSDK-VERIFY3 (yB) | 5/5 (`yB-acp_files.log`) | prior xB: 3p/2f (T03,T05 Terminal gate) |
| WSX-001 | ACPSDK-VERIFY3 (yB) | 5/5 (`yB-workspace_proxy.log`) | prior xB: 4p/1f (T02 BadEndpoint) |
| WSX-002 | ACPSDK-VERIFY3 (yB) | 5/5 (`yB-remote_sync.log`) | prior xB: 4p/1f (T02 backoff table) |
| SDK-001 | ACPSDK-VERIFY3 (yB) | 12/12 (`yB-sdk_client.log`) | prior xB: 6p/6f |
| SDK-002 | ACPSDK-VERIFY3 (yB) | 11/11 (`yB-sdk_spawns.log`) | prior xB: 1p/10f (spawn short-circuit) |
| HEAD-001 | ACPSDK-VERIFY3 (yB) | 8/8 (`yB-run_headless.log`) | prior xB: 2p/6f (exit code) |
| HEAD-002 | ACPSDK-VERIFY3 (yB) | 8/8 (`yB-session_export.log`) | prior xB: 7p/1f (T04 secret leak) |
| RUN-001 | ACPSDK-VERIFY3 (yB) | 5/5 (`yB-runner.log`) | prior xB: 0p/5f (start short-circuit) |
| ACPSDK total | ACPSDK-VERIFY3 (yB) | **64/64, 0 failed** | — |
| EXT-001 | EXT1-VERIFY4 (yC) | 5/5 (`yC-ext1.log`, 6 suites 30/30) | prior xC: 4p/1f (T03 dup guard) |
| EXT-002 | EXT1-VERIFY4 (yC) | 5/5 | prior xC: 4p/1f (T01 caps) |
| EXT-004 | EXT1-VERIFY4 (yC) | 5/5 | prior xC: 3p/2f (T01,T04 reload-kind) |
| EXT-005 | EXT1-VERIFY4 (yC) | 5/5 struct-shape ONLY — **CONTRACT MISMATCH CAVEAT** (see §4a) | prior xC: 3p/2f (T01,T04 byte cap) |
| EXT-006 | EXT1-VERIFY4 (yC) | 5/5 | prior xC: 4p/1f (T03 broker-deny) |
| EXT-008 | EXT1-VERIFY4 (yC) | 5/5 | prior xC: 4p/1f (T03 dup guard) |
| EXT-009 | EXT2-VERIFY4 (yD) | 5/5 (`yD-ext2.log`, 4 suites 20/20) | prior xD: 0p/5f full-fail |
| EXT-010 | EXT2-VERIFY4 (yD) | 5/5 | prior xD: 1p/4f (T04 negative-path survives) |
| EXT-011 | EXT2-VERIFY4 (yD) | 5/5 on PORTED bytes (guard-port hash PASS) | prior xD: 0p/5f full-fail |
| EXT-012 | EXT2-VERIFY4 (yD) | 5/5 | prior xD: 0p/5f full-fail |
| TOOL-016 | TOOL-VERIFY4 (yJ) | 5/5 (`yJ-tool.log`, 8 suites 40/40) | prior xJ: 1p/4f (T05 empty-path survives) |
| TOOL-017 | TOOL-VERIFY4 (yJ) | 5/5 | prior xJ: 1p/4f (T02 selection-only survives) |
| TOOL-018 | TOOL-VERIFY4 (yJ) | 5/5 | prior xJ: 4p/1f (T01 ready gate) |
| TOOL-019 | TOOL-VERIFY4 (yJ) | 5/5 | prior xJ: 1p/4f (T04 validation-only survives) |
| TOOL-020 | TOOL-VERIFY4 (yJ) | 5/5 | prior xJ: 3p/2f (T01,T04) |
| UI-019 | TOOL-VERIFY4 (yJ) | 5/5 | prior xJ: 1p/4f (T04 gate fires pre-stub) |
| SYNC-002 | TOOL-VERIFY4 (yJ) | 5/5 | prior xJ: 3p/2f (T01,T04) |
| SYNC-001 | TOOL-VERIFY4 (yJ) | 5/5 | prior xJ: 0p/5f full-fail |
| WEB-013 (sessions entry-point) | RED-VALIDITY-WEB-PROV (s3) | 5/5 | 0p/5f (adapter gate) |
| WEB-014 | RED-VALIDITY-WEB-PROV (s3) | 5/5 | 3p/2f (T01,T04) |
| WEB-015 (sessions entry-point) | RED-VALIDITY-WEB-PROV (s3) | 5/5 | 0p/5f (ghost id) |
| WEB-016 | RED-VALIDITY-WEB-PROV (s3) | 5/5 | 1p/4f (T02 neg-path survives) |
| WEB-017 | RED-VALIDITY-WEB-PROV (s3) | 5/5 | 1p/4f (T02 gate survives) |
| WEB-001..006 suites | WEB-VERIFY4 (yK) | GREEN in 107-test sweep (control/route/asset/transcript/composer lanes) | prior worklogs; no fresh RED this wave |
| WEB-007..012 suites | WEB-VERIFY4 (yK) | GREEN in 107-test sweep (chat_nav/web_008/capabilities/workspace lanes) | none — GREEN-only HOLD stands |
| WEB total | WEB-VERIFY4 (yK) | **107 passed, 0 failed** (`yK-web.log`); `cargo check -p opencode-rk-server` 0 errors; stub/audio/write-path scans clean | — |
| PROV-015 | PROV-VERIFY4 (yG) | 5/5 (`yG-prov.log`, 11 suites 56/56) | uD A1/A2/A3 all bite (strong) |
| PROV-016 lifecycle | PROV-VERIFY4 (yG) | 5/5 + bounds 6/6 | uD B1/B2 pins |
| PROV-016 boundary | PROV-VERIFY4 (yG) | 5/5 GREEN-ON-STUB | **NO BITE 5/5 ×3 — OPEN (§3c)** |
| PROV-017..024 | PROV-VERIFY4 (yG) | 5/5 each (10×5 + bounds 6 = 56 total) WITH fix present | RED history in per-lane worklogs |
| WEB-013 server boundary | RED-VALIDITY-WEB013-015 (uC) | 1/1 | flag-flip FAIL, missing-key FAIL; reason-text-only PASSES |
| WEB-015 server boundary | RED-VALIDITY-WEB013-015 (uC) | 2/2 | 3/3 stub strategies bite |
| OPS-001..009 | OPS-VERIFY4 (yF) | 5/5 each, 45/45 (`yF-ops.log`); `cargo check` clean (1 pre-existing warning) | prior FINAL uB: FAILED ×9 (2× partial: OPS-001 3/2, OPS-003 2/3) |
| INT-001/002/003/005/006/007/009/010 | INT-VERIFY4 (yE) | 5/5 each, 40/40 (`yE-int.log`) | prior FINAL uA: 2/3, 0/5, 3/2, 3/2, 4/1, 4/1, 2/3, 0/5 |
| SHARE-001..005 (10 suites) | SHARE-DEDUP4 (yI) | 5/5 each, 50/50 (`yI-share.log`); redaction probe PASS, TOTAL_LEAK_BYTES=0 (`yI-probe.log`); `cargo check -p opencode-rk-sessions` exit 0 | none — GREEN+probe by design, no behavior-RED |
| session_turn_stream_api | TURN-STREAM-GATE (3x serial) + yK re-confirm | 2/2 ×3 runs + 2/2 in yK sweep | n/a (harness race, not behavior RED) |
| EXT002-T05 | EXT002-T05-DETERMINISM | serial 30/30 | flaky-by-construction; NO-FIX |
| EXT twins (8 pairs) | EXT-TWINS-DISPOSITION (uM 35/35) | 35/35 7-suite | pair-1 guard PORTED (§3a); pairs 2–8 drop-pending |
| SHARE twins (5 pairs) | SHARE-TWINS-DISPOSITION | survey only | keep-both everywhere |
| REL-001..003 | lane worklogs only | lane GREEN | none — validator-only, verifier re-run pending |

### 4a. EXT-005 contract mismatch (must-flag, NEW this wave)

- EXT1-VERIFY4 §contract-mismatch: card `tasks/EXT-005.md` demands
  `validate_manifest_bytes(bytes:&[u8])` JSON pipeline (length→UTF-8→JSON→fields),
  `MAX_MANIFEST_BYTES=16384`, `MAX_NAME_LEN=128`, `MAX_CAPABILITIES=16`,
  `MAX_CAP_LEN=64`, `Manifest{name,contract_version:u32,capabilities}`,
  `SUPPORTED_CONTRACT_VERSION=1`, T04 byte-layer (TooLarge/InvalidEncoding/
  InvalidJson/1MiB prompt). Actual `plugin_manifest.rs` (55L): struct-only
  `PluginManifest{name:String,version:String(X.Y.Z),permissions:Vec<String>}`,
  `MAX_PERMISSIONS=32`, errors EmptyName/BadVersion/EmptyPermission/
  TooManyPermissions. Tests named `manifest_t01..t05`, struct shape only.
- Effect: EXT-005 GREEN 5/5 proves struct-shape only, NOT the frozen
  EXT-005-T01..T05 contract. Flip row 8 downgraded y → **y-caveat**
  (owning lane implements bytes pipeline to card OR moves card through
  gap-closure with verifier approval). Ready-count unchanged (y-caveat counts
  in ready-y bucket).

## 5. RED receipt inventory (which waves cover which IDs)

In-repo receipt lives in `worklog/`; per-test logs in `/tmp/opencode` (ephemeral).
No RED receipts live in `crates/` (by design — frozen tests untouched).

| Wave file | Covers | Verdict |
|---|---|---|
| `RED-VALIDITY-AUTO4.md` (yA-*.log, agents-full) | AUTO-004/006/005 | **GREEN re-confirm 35/35. RED bite carried from AUTO3 (xA). MECHANISM CAVEAT persists: AUTO-TOKIO-BROKER finds tokio REQUIRED by card but NOT MET (sync bool-flag cancel passes T04 vacuously); broker mirror SUFFICES. Card amendment OR integration lane + controller dep approval.** |
| `ACPSDK-VERIFY3.md` (yB-*.log) | ACP-001/002, WSX-001/002, SDK-001/002, HEAD-001/002, RUN-001 | **GREEN re-confirm 64/64. RED bite carried from ACPSDK2 (xB, strong behavior).** |
| `EXT1-VERIFY4.md` (yC-ext1.log) | EXT-001/002/004/005/006/008 | **GREEN re-confirm 30/30. RED bite carried from EXT1C (xC, strong). EXT-005 contract-mismatch caveat (§4a).** |
| `EXT2-VERIFY4.md` (yD-ext2.log) | EXT-009/010/011/012 | **GREEN re-confirm 20/20. RED bite carried from EXT2C (xD, strong). Guard-port hash CHECK PASS.** |
| `TOOL-VERIFY4.md` (yJ-tool.log) | TOOL-016..020, UI-019, SYNC-001/002 | **GREEN re-confirm 40/40. RED bite carried from TOOL3 (xJ, strong).** |
| `WEB-VERIFY4.md` (yK-web.log) | WEB-001..017 suites | **GREEN sweep 107/107 on current bytes (no fresh RED; RED for 013–017 entry-point carried from s3). Browser DOM/focus/reflow (vitest/tsc) unexecuted — verifier scope.** |
| `PROV-VERIFY4.md` (yG-prov.log) | PROV-015..024 | **GREEN 56/56 WITH fix present. RED bite: PROV-015 strong (uD A1/A2/A3), PROV-016 lifecycle pins (uD B1/B2), boundary OPEN (§3c). PROV-017 gate SATISFIED.** |
| `INT-VERIFY4.md` (yE-int.log) | INT-001/002/003/005/006/007/009/010 | **GREEN 40/40 on current bytes. RED bite carried from INT-FINAL (uA retry). Dirty-on-arrival caveat persists — evidence-at-verify-time, not quiesced-tree validity.** |
| `OPS-VERIFY4.md` (yF-ops.log) | OPS-001..009 | **GREEN 45/45 on current bytes. RED bite carried from OPS-FINAL (uB epoch). Same dirty-tree caveat as INT.** |
| `SHARE-DEDUP4.md` (yI-share/probe.log) | SHARE-001..005 | **GREEN 50/50 + redaction probe PASS (0 leak bytes). No behavior-RED by design (keep-both disposition).** |
| `RED-VALIDITY-WEB-PROV.md` (s3-*-red/green.log, bak-*.rs) | WEB-013/014/015/016/017 + PROV-015/016 entry-point | **RED-VALID (strong, entry-point). QUALIFIED by uC/uD rows — entry-point verdict stands, boundary precision does NOT extend.** |
| `RED-VALIDITY-WEB013-015.md` (uC-attempt logs) | WEB-013/015 server boundary | **WEAK-RED (partial pinning). Unavailable/registry-metadata = YES pinned; reason-string precision + full T01–T05 = NOT pinned. Additive gap probes proposed (NEW-FILE, not applied).** |
| `RED-VALIDITY-PROV015-016.md` (uD-a/b logs) | PROV-015 (strong) / PROV-016 (lifecycle pins, boundary OPEN) | **PROV-015 RED-VALID; PROV-016 WEAK-RED. Codex-bounds OPEN (§3c).** |
| `RED-VALIDITY-OPS-FINAL.md` (PRIMARY RED; uB-*-red.log 9/9) | OPS-001..009 | **RED-VALID (strong, behavior). Caveat: tree dirty on entry — wiring proven on CURRENT content, NOT pristine-commit validity. yF GREEN re-confirms current bytes; quiesced re-run still required before accept-flip.** |
| `RED-VALIDITY-INT-FINAL.md` (PRIMARY RED; uA-INT*-red.log 8/8 + green1/2) | INT-001/002/003/005/006/007/009/010 | **RED-VALID (strong, behavior). Same dirt caveat as OPS; yE GREEN re-confirms current bytes.** |
| `TURN-STREAM-GATE.md` (stream_serial{1,2,3}) | session_turn_stream_api | serial-mandate only (§6) |
| Superseded epochs (detail only, NOT primary) | EXT1/EXT1B, EXT2, AUTO/AUTO2/AUTO3, TOOL/TOOL2, ACPSDK/ACPSDK2, OPS/OPS2, INT/INT2/INT3, WEB-007-012-ACCEPT | retained for archaeology; latest wave above wins on conflict |

## 6. Serial-test mandates (binding on verifier/CI)

```
# stream binary: env race (TURN-STREAM-GATE §2)
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-server --test session_turn_stream_api -- --test-threads=1
# ext_builtins_lane binary: T05 thread-count race (EXT002-T05-DETERMINISM §2-3)
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 cargo test -p opencode-rk-tools --test ext_builtins_lane -- --test-threads=1
```

- Stream root cause: process-global env race (`OPENAI_BASE_URL`/`KEY` via
  unlocked `EnvGuard`, 2 tests same binary). 3x serial GREEN logged + yK
  re-confirm 2/2. No server bug; `TURN_PERMITS` cap does not fix test parallelism.
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
Counts: **ready-y 70 / partial 3 / not-ready 9** (total 82 = 44 in-progress + 38 not-started).

y-evidence lifts vs v13 (34 → 70, +36):
- PROV (+8): PROV-017..024 n → y. PROV-VERIFY4 proves fix present
  (`claude_oauth.rs:462` raw const interpolation) + 56/56 GREEN matching GATE3
  hashes. v13 HOLD gate (re-run prov_017+018..024) SATISFIED. PROV-015 already y.
- INT-GREEN (+8): INT-001/002/003/005/006/007/009/010 n → y-caveat. INT-VERIFY4
  gives fresh 40/40 GREEN on CURRENT bytes (`yE-int.log`) where v13 had only the
  stale FINAL-epoch receipt. Caveat persists (dirty-on-arrival, hashes
  evidence-at-verify-time) → acceptance still needs quiesced-tree re-run.
- OPS-GREEN (+9): OPS-001..009 n → y-caveat. OPS-VERIFY4 gives fresh 45/45
  GREEN on current bytes (`yF-ops.log`) + clean `cargo check`. Same dirty-tree
  caveat as INT → y-caveat, not full y.
- SHARE (+5): SHARE-001..005 n → y-caveat. SHARE-DEDUP4 adds what v13 lacked
  entirely: 10-suite 50/50 GREEN + redaction probe PASS (0 leak bytes) +
  sessions `cargo check` exit 0. y-caveat (not full y) because no behavior-RED
  by design (keep-both disposition) and merge/queue owned by redaction writer.
- WEB (+6): WEB-001..006 n → y-caveat. WEB-VERIFY4 107-test sweep GREENs their
  suites on current bytes where v13 had "prior worklogs; no RED receipt this
  wave". y-caveat (not full y): GREEN-only evidence, entry-point RED receipts
  cover only 013–017, browser vitest/tsc unexecuted.
- WEB-007..012 stay n (GREEN-only HOLD by design, write-paths disabled).
- PROV-016 stays partial (boundary OPEN §3c). WEB-013/015 stay partial (uC WEAK-RED).
- EXT-005 y → y-caveat (§4a contract mismatch; count-neutral).

| # | Story | ralph now | ready | Evidence path / note |
|---|---|---|---|---|
| 1 | AUTO-004 | in-progress | y-caveat | yA GREEN 5/5 + full 35/35; RED bite via xA 3/2; CAVEAT tokio-mechanism (card amendment or integration lane, AUTO-TOKIO-BROKER) |
| 2 | AUTO-005 | in-progress | y | yA GREEN 5/5 + tool PASS/mutated-exit-2; RED bite via xA 0/5 |
| 3 | AUTO-006 | in-progress | y-caveat | yA GREEN 5/5; RED bite via xA 4/1; same tokio-pool caveat as AUTO-004 |
| 4 | EXT-001 | in-progress | y | yC GREEN 5/5 (30/30 wave); RED bite via xC 4/1 |
| 5 | EXT-002 | in-progress | y | yC GREEN 5/5; RED bite via xC 4/1 |
| 6 | EXT-004 | in-progress | y | yC GREEN 5/5; RED bite via xC 3/2 |
| 7 | EXT-005 | in-progress | y-caveat | yC GREEN 5/5 BUT struct-shape only — CONTRACT MISMATCH §4a (bytes pipeline to card or gap-closure) |
| 8 | EXT-006 | in-progress | y | yC GREEN 5/5; RED bite via xC 4/1 |
| 9 | EXT-008 | in-progress | y | yC GREEN 5/5; RED bite via xC 4/1 |
| 10 | EXT-009 | in-progress | y | yD GREEN 5/5 (20/20 wave); RED bite via xD 0/5 |
| 11 | EXT-010 | in-progress | y | yD GREEN 5/5; RED bite via xD 1/4 |
| 12 | EXT-011 | in-progress | y | yD GREEN 5/5 on PORTED bytes (hash b4c517e3 PASS); RED bite via xD 0/5 |
| 13 | EXT-012 | in-progress | y | yD GREEN 5/5; RED bite via xD 0/5 |
| 14 | INT-001 | in-progress | y-caveat | LIFT vs v13 (was n): yE GREEN 5/5 on current bytes; RED bite via uA 2/3; CAVEAT dirty-tree — quiesced re-run before accept-flip |
| 15 | INT-002 | in-progress | y-caveat | LIFT vs v13: yE GREEN 5/5; RED bite via uA 0/5; same quiesced-tree caveat |
| 16 | INT-003 | in-progress | y-caveat | LIFT vs v13: yE GREEN 5/5; RED bite via uA 3/2; same caveat |
| 17 | INT-005 | in-progress | y-caveat | LIFT vs v13: yE GREEN 5/5; RED bite via uA 3/2; same caveat |
| 18 | INT-006 | in-progress | y-caveat | LIFT vs v13: yE GREEN 5/5; RED bite via uA 4/1; same caveat |
| 19 | INT-007 | in-progress | y-caveat | LIFT vs v13: yE GREEN 5/5; RED bite via uA 4/1; same caveat |
| 20 | INT-009 | in-progress | y-caveat | LIFT vs v13: yE GREEN 5/5; RED bite via uA 2/3; same caveat |
| 21 | INT-010 | in-progress | y-caveat | LIFT vs v13: yE GREEN 5/5; RED bite via uA 0/5; same caveat |
| 22 | OPS-001 | in-progress | y-caveat | LIFT vs v13 (was n): yF GREEN 5/5 on current bytes; RED bite via uB 3/2; CAVEAT dirty-tree — quiesced re-run before accept-flip |
| 23 | OPS-002 | in-progress | y-caveat | LIFT vs v13: yF GREEN 5/5; RED bite via uB 0/5; same caveat |
| 24 | OPS-003 | in-progress | y-caveat | LIFT vs v13: yF GREEN 5/5; RED bite via uB 2/3; same caveat |
| 25 | OPS-004 | in-progress | y-caveat | LIFT vs v13: yF GREEN 5/5; RED bite via uB 0/5; same caveat |
| 26 | OPS-005 | in-progress | y-caveat | LIFT vs v13: yF GREEN 5/5; RED bite via uB 0/5; same caveat |
| 27 | OPS-006 | in-progress | y-caveat | LIFT vs v13: yF GREEN 5/5; RED bite via uB 0/5; same caveat |
| 28 | OPS-007 | in-progress | y-caveat | LIFT vs v13: yF GREEN 5/5; RED bite via uB 0/5; same caveat |
| 29 | OPS-008 | in-progress | y-caveat | LIFT vs v13: yF GREEN 5/5; RED bite via uB 0/5; same caveat |
| 30 | OPS-009 | in-progress | y-caveat | LIFT vs v13: yF GREEN 5/5; RED bite via uB 0/5; same caveat |
| 31 | REL-001 | in-progress | n | validator-only, lane GREEN, verifier re-run pending (INTEGRATION-5 §3) |
| 32 | REL-002 | in-progress | n | same as REL-001 |
| 33 | REL-003 | in-progress | n | same as REL-001 |
| 34 | SHARE-001 | in-progress | y-caveat | LIFT vs v13 (was n): yI 10-suite 50/50 GREEN + probe PASS 0 leak bytes; CAVEAT no behavior-RED by design, merge/queue writer-owned |
| 35 | SHARE-002 | in-progress | y-caveat | LIFT vs v13: same yI evidence; same caveat |
| 36 | SHARE-003 | in-progress | y-caveat | LIFT vs v13: same yI evidence; same caveat |
| 37 | SHARE-004 | in-progress | y-caveat | LIFT vs v13: same yI evidence; same caveat |
| 38 | SHARE-005 | in-progress | y-caveat | LIFT vs v13: same yI evidence; same caveat |
| 39 | WEB-001 | in-progress | y-caveat | LIFT vs v13 (was n): yK 107-sweep GREENs suite on current bytes; CAVEAT GREEN-only, no entry-point RED |
| 40 | WEB-002 | in-progress | y-caveat | LIFT vs v13: same yK evidence; same caveat |
| 41 | WEB-003 | in-progress | y-caveat | LIFT vs v13: same yK evidence; same caveat |
| 42 | WEB-004 | in-progress | y-caveat | LIFT vs v13: same yK evidence; same caveat |
| 43 | WEB-005 | in-progress | y-caveat | LIFT vs v13: same yK evidence; same caveat |
| 44 | WEB-006 | in-progress | y-caveat | LIFT vs v13: same yK evidence; same caveat |
| 45 | WEB-007 | not-started | n | GREEN-only HOLD (WEB-007-012-ACCEPT + yK re-confirm); write-paths disabled, no RED by design |
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
| 56 | PROV-015 | not-started | y | RED-VALID strong (uD A1/A2/A3 all bite) + yG GREEN 5/5; controller decides flip |
| 57 | PROV-016 | not-started | partial | lifecycle pins (uD B1/B2) BUT boundary validators non-pinning (§3c); T06–T08 tests needed before full flip |
| 58 | PROV-017 | not-started | y | LIFT vs v13 (was n): fix PRESENT + yG GREEN 5/5 matching GATE3; controller decides flip |
| 59 | PROV-018 | not-started | y | LIFT vs v13: same yG evidence |
| 60 | PROV-019 | not-started | y | LIFT vs v13: same yG evidence |
| 61 | PROV-020 | not-started | y | LIFT vs v13: same yG evidence |
| 62 | PROV-021 | not-started | y | LIFT vs v13: same yG evidence |
| 63 | PROV-022 | not-started | y | LIFT vs v13: same yG evidence |
| 64 | PROV-023 | not-started | y | LIFT vs v13: same yG evidence |
| 65 | PROV-024 | not-started | y | LIFT vs v13: same yG evidence |
| 66 | UI-019 | not-started | y | yJ GREEN 5/5; RED bite via xJ 1/4; review → in-progress? |
| 67 | TOOL-016 | not-started | y | yJ GREEN 5/5; RED bite via xJ 1/4; review → in-progress? |
| 68 | TOOL-017 | not-started | y | yJ GREEN 5/5; RED bite via xJ 1/4; review → in-progress? |
| 69 | TOOL-018 | not-started | y | yJ GREEN 5/5; RED bite via xJ 4/1; review → in-progress? |
| 70 | TOOL-019 | not-started | y | yJ GREEN 5/5; RED bite via xJ 1/4; review → in-progress? |
| 71 | TOOL-020 | not-started | y | yJ GREEN 5/5; RED bite via xJ 3/2; review → in-progress? |
| 72 | SYNC-001 | not-started | y | yJ GREEN 5/5; RED bite via xJ 0/5 full-fail; review → in-progress? |
| 73 | SYNC-002 | not-started | y | yJ GREEN 5/5; RED bite via xJ 3/2; review → in-progress? |
| 74 | RUN-001 | not-started | y | yB GREEN 5/5 (64/64 wave); RED bite via xB 0/5, WIRED-uncommitted §3, gate pending |
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
set → FEATURES.md sync only (guard log 122 lines: 69 stale-mirror + 53 ledger/gap).
EXT twins pairs 2–8 → integrator executes drops per §3a AFTER pair-1 GREEN re-run.
EXT-005 card-vs-impl gap → owning lane or gap-closure (§4a).

## 8. Ranked blockers top 5 (owner lanes)

1. Wired-but-uncommitted + ungated + unattributed lib.rs drift + LANDED guard-port
   un-gated (owner: wiring lanes + verifier + integrator).
   Server +20, sessions +3, foundation +3 mods in worktree, zero committed.
   security/storage/tools lib.rs reorder/fmt-only diffs still no owning lane
   (storage +18/-9) — attribute or revert before the integration commit.
   `plugin_transform` guard-port (+9/-2) hash-verified on ported bytes (yD) but
   still needs verifier-logged serial 5/5 re-run, then twin drop same commit.
   Action: serial lane-gate GREEN per suite (§6 mandates for
   `session_turn_stream_api` + `ext_builtins_lane`), then ONE integration commit.
2. Guard FAIL backlog exhaustion (owner: controller). 122 errors exit 1,
   byte-identical sets vs TRIAGE-13/14. 69 FEATURES sync + 53 ledger/gap; no lane edits ralph.json.
3. Dirty-tree caveat on INT + OPS y-caveat lifts + frozen-test reformats (owner: INT/OPS lanes + verifier).
   Fresh GREEN now on CURRENT bytes (yE 40/40 + yF 45/45), but logged against dirty
   worktree, not pristine commit; frozen tests carry uncommitted rewraps. Neither
   slice accept-flippable until quiesced-tree re-run. Verifier must read FINAL +
   yE/yF files as the receipts, not wait on further files.
4. Codex-bounds + WEB-013/015 boundary gaps OPEN + EXT-005 contract gap (owner:
   providers/server/EXT lanes + test owner).
   PROV-016 B3-class (device-code / URL-length / zero-expiry) non-pinning; WEB-013
   reason-string + full T01–T05, WEB-015 full contract non-pinning by absence.
   NEW: EXT-005 impl proves struct-shape only, not card bytes-pipeline (§4a).
   Additive T06–T08 / gap-probe / bytes-pipeline patches proposed but NOT applied
   (frozen-test waiver needed). Lifecycle-only flips possible; boundary-hardening flips blocked.
5. AUTO-004 tokio gap + fmt/untracked drift (owner: controller + integrator).
   AUTO-004/006 need card amendment (sync state machine) or tokio integration lane —
   controller dep approval either way (yA re-confirms GREEN, mechanism caveat stands).
   `cargo fmt --check` 48 (flat vs v10–v13); status 420→432 (+12 untracked worklogs, CORR).
   PROV-017 gate CLEARED this wave (§3b) — drops out of this blocker.

Merge order: (i) attribute/revert §3 unattributed lib.rs drift; (ii) verifier-logged
serial `plugin_transform` 5/5 on ported bytes → drop `ext_replay_lane` same commit
(§3a); (iii) quiesce INT/OPS owned paths + frozen tests → INT/OPS re-run (§4/§5);
(iv) T06–T08 codex-bounds tests + gap probes + EXT-005 bytes pipeline ONLY with
test-owner waiver (§3c/§4a/§5); (v) lane gates GREEN per wired suite (serial mandates §6);
(vi) refresh stale status files; (vii) ONE integration commit (tracked fixes + lib.rs
wirings incl. PROV-017 one-liner, fmt at commit); (viii) untracked modules only with
passing gates; (ix) controller syncs FEATURES.md/accounting + flips per §7.
