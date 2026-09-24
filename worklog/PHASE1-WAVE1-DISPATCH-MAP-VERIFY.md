# PHASE1-WAVE1-DISPATCH-MAP-VERIFY

## Claim
- Task: PHASE1-WAVE1-DISPATCH-MAP-VERIFY, task type verification.
- Session: `ses_f2dbd2ce8ffe1GjyBSZYwJYwS2`; scratchpad `worklog/PHASE1-WAVE1-DISPATCH-MAP-VERIFY.md`.
- Role: independent execution-manifest verifier. No manifest edit, no product/test/Cargo change, no lane launch, no controller reconciliation, no acceptance.
- Candidate: commit `0e6675bd2f7b263ac6b05a1643f8e03bb7ff847c` on `plan/phase1-wave1-dispatch`, file `worklog/PHASE1-WAVE1-DISPATCH-MAP.md`.

## Verdict
`ACCEPT WITH CORRECTIONS`. The compact map is mostly evidence-grounded and its
parent-open warning is correct, but one launch-next row (Lane 1) is stale
(already authored and independently verified), and every launch command carries a
non-consumable `timeout` token plus a truncated revision literal on row 2.

## Immutable evidence refs re-verified
- Source evidence `29ec0b3` file present; base `5d666830bc9e5b716b2ea1d239d9d0ccbe6e067b` exists as a commit.
- Shell verifier `8c514e0` on `origin/verify/TOOL-RED-SHELL` exists; worklog `worklog/TOOL-RED-SHELL-VERIFY.md` verdict `ACCEPT WITH SPLIT`.
- Security verifier `632a71e` on `origin/plan/security-acceptance` exists; worklog `worklog/PHASE1-SECURITY-MAP-VERIFY-RETRY.md` verdict `ACCEPT WITH CORRECTIONS`.
- `rtk git ls-remote origin` confirms `plan/phase1-wave1-dispatch=0e6675b`, `verify/TOOL-RED-SHELL=8c514e0`, `plan/security-acceptance=632a71e`, `lane/TOOL-RED-SHELL=2cd9ca9` are all remote references.

## Row-by-row launch table check

### Lane 1 - Tools shell broker RED (RED authoring) -> DEFECT: not launch-now (already authored + verified)
- Owned file `crates/tools/tests/phase1_shell_broker.rs` is NOT new. It exists on
  `origin/lane/TOOL-RED-SHELL` (commit `2cd9ca9987bed99adcff0d1bee0038f7f1b2c3c5`)
  and on `origin/verify/TOOL-RED-SHELL`.
- Re-hashed the artifact bytes: `2853960b7ad711d88384b136db5b37eca1dd827e82c4339990c120267a4e11ed`,
  byte-identical to the verifier's claimed frozen hash. Verified `ACCEPT WITH SPLIT`.
- Manifest row says "new" and "Author compiling RED first. Freeze hash." Both are
  stale relative to immutable evidence ref row 1 / shell verifier. Re-authoring
  would duplicate frozen bytes and collide with the existing lane artifact.
- Classification: already-authored / verification-only. Shell *implementation*
  remains blocked per the split verdict; the manifest is correct that shell impl
  is not launchable, but wrong to present RED authoring as launch-next.

### Lane 2 - Installed entrypoint verify (verification) -> launch-now WITH command corrections
- Frozen test hash recomputed `fec2fdb94c74df2493eb8eb2f2098732d813096f0e01928f00a8cb8c621bf317` = source-evidence row 4. PASS.
- Read-only target; `#![cfg(target_os = "macos")]` confirmed at line 1. Package
  `opencode-rk-cli` / target `installed_default_entrypoint`, `--features native`
  confirmed in `crates/cli/Cargo.toml`.
- DEFECT command: literal `env OC2_BUILD_REVISION=5d66683...` is a truncated
  placeholder (10 chars). `crates/cli/build.rs:50-60` `valid_revision` requires
  exactly 40 lowercase-hex or `fail_invalid_revision()` exits 1. Must be full
  `5d666830bc9e5b716b2ea1d239d9d0ccbe6e067b` or omitted (build.rs auto-detects git
  revision when env unset).
- DEFECT command: `timeout 180` token. No `timeout`/`gtimeout` binary on this
  macOS host (`command -v timeout` -> exit 127). The literal command is not
  consumable as written.

### Lane 3 - APP-012 journey verify (verification) -> launch-now WITH timeout correction
- Frozen test hash recomputed `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50` = source-evidence row 5. PASS.
- Read-only target; package `opencode-rk-server` / `app012_tool_journey_red` confirmed.
- DEFECT command: `timeout 180` absent on host. Otherwise consumable.

### Lane 4 - Native artifact manifest verify (verification) -> launch-now WITH timeout correction
- Frozen test hash recomputed `cb7d4cde7c3aed1916814c9aab747b68aa48015518c444e90bd8a58e66bd5c8b` = source-evidence row 6. PASS.
- Read-only, std-only target; `opencode-rk-opentui-bridge` / `native_artifact_manifest` confirmed.
- DEFECT command: `timeout 180` absent on host. Otherwise consumable.

## Collision matrix recreation
- Launch table (section 3) has exactly 4 rows; scope the parser to that table
  (naive `^| [0-9] |` also matches the section 7 resource table and yields 8
  false rows). Within section 3, four launch rows own four distinct files; no
  duplicate among them. PASS on literal ownership.
- BUT `crates/tools/tests/phase1_shell_broker.rs` is not distinct from the already
  landed lane artifact. The map's "None (new file)" justification is false against
  `origin/lane/TOOL-RED-SHELL`. Reclassifying Lane 1 to verification-only removes
  the collision claim entirely.
- `tasks/completion/claims.json` shared by cc.claim fail-closed: PASS.

## Duplicate ownership as launchable (active work duplicated)
- No live fenced ledger row exists for these tasks in this branch's
  `tasks/completion/claims.json` (`TOOL-RED-SHELL`, `TOOL-RED-SHELL-VERIFY`,
  `SEC-RED-OS` all absent here). So no double-claim will be fenced. But Lane 1
  duplicates *already-landed* work on a remote branch, which the map fails to
  detect. That is the duplicate-active defect.

## Queued blockers and shell split status
- Queued table rows match source-evidence rows 2,3,7,8,9,10,11 accurately. PASS.
- Shell split: verifier `8c514e0` `ACCEPT WITH SPLIT`; t01-t03 broker contract vs
  t04 cancellation. Manifest section 6 says "Implementation blocked pending shell
  split/refreeze authorization" and section 3 "Shell implementation is NOT
  launchable per ACCEPT WITH SPLIT verdict." CORRECT.
- `plan/TOOL-RED-SHELL-SPLIT` exists only locally; NOT on origin. Manifest does
  not claim it as landed, so no defect, but the split authorization is not
  remotely persisted.

## SEC-RED-OS current state (no polling)
- `lane/SEC-RED-OS` local HEAD `62f43ce` = `SEC-RED-OS-VERIFY: verdict INCOMPLETE, candidate artifact absent`.
- `worklog/SEC-RED-OS-VERIFY.md`: candidate `crates/security/tests/phase1_os_isolation.rs` absent; author row `in-progress` session `ses_f2dc11dd3ffeWw31mT4dyuJYOM`; not on origin.
- This branch's ledger has no `SEC-RED-OS` row. Manifest line 44/97 "SEC-RED-OS is
  active" is defensible only as "a claim exists on an unmerged local branch". It is
  not launchable here and is correctly listed BLOCKED/QUEUED. PASS on not launchable.
  Correction: qualify "active" as unmerged/INCOMPLETE-verified, not live.

## Gate count reconciliation
- Re-ran `rtk python3 tools/convergence_gate.py`: `CONVERGENCE BLOCKED`, `total=92`.
- Manifest section 2 says `total=90`, then "Current gate blocked at 91". Brief says 92.
- DEFECT: internal inconsistency (90 vs 91) and stale against current 92. Count drift, non-blocking for launch safety but must be corrected.

## Breadth / capacity recreation
- Manifest: breadth 4 of 14; 4 integration + 2 verifier reserved; 6 slots held.
- 4 breadth + 6 held = 10 <= 14 per AGENTS.md convergence-first reservation
  (>=4 integration, >=2 verifier, <=14 breadth). PASS.
- Reclassifying Lane 1 to verification-only reduces breadth to 3; still valid.

## One-heavy-command resource schedule
- Schedule serializes lanes 1-4 with `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`,
  `timeout 180`, and a stop threshold of <1 GiB free. Ordering and serialization
  are sound and within the 8 GiB budget. PASS.
- DEFECT: "Record `rtk free -h` before each" is Linux-only; `free` is absent on
  macOS. Use `vm_stat`/`memory_pressure` or `ps`-based available-memory query.

## Platform boundaries
- Lane 2 macOS-only (`cfg(target_os="macos")`) confirmed. Lane 3 cross-platform. Lane 4 host-neutral metadata + macOS-arm64 artifact contract. PASS.
- Section 8 does not explicitly exclude Windows-MSVC, though source-evidence
  omitted-lanes does. `crates/opentui-bridge/build.rs:48-57` distinguishes
  `target_env=="msvc"` (opentui.lib) from GNU (libopentui.dll.a). Correction: add
  explicit "MSVC not a supported pair; do not relabel GNU artifacts".

## Parent-completion check
- APP-012 ledger row `in-progress` (session `ses_f3c4de578ffelQv59xDXmOs03B`) confirmed.
- Manifest section 9 states APP-012 remains OPEN and lanes 2-4 do not close it;
  section 2 COMPLETED=0. PASS. No isolated lane is presented as closing APP-012 or
  release parents.

## Corrections required (non-negotiable before consuming the map)
1. Reclassify Lane 1 from "RED authoring / new file / launch-now" to
   already-authored verification-only (artifact frozen at `origin/lane/TOOL-RED-SHELL` `2cd9ca9`, hash `2853960b...`, verified `ACCEPT WITH SPLIT` `8c514e0`). Do not re-author; do not implement.
2. Remove or explicitly mark `timeout 180` as unavailable on this macOS host in all four commands (no `timeout`/`gtimeout` binary; exit 127).
3. Replace truncated `5d66683...` on Lane 2 with the full 40-hex
   `5d666830bc9e5b716b2ea1d239d9d0ccbe6e067b`, or omit it (build.rs auto-detects).
4. Reconcile gate count: state `total=92` (currently says 90 and 91).
5. Replace `rtk free -h` with a macOS-valid available-memory query.
6. State the Windows-MSVC exclusion explicitly (GNU pair only).

## Consumability decision
- Not consumable as written (Lane 1 duplicate + non-runnable commands).
- Consumable after corrections 1-3: observations/read-only verification of lanes
  3 and 4 may run now (drop `timeout`); lane 2 may run with a full revision or no
  revision; shell implementation stays blocked; OS isolation stays blocked.
- Launch constraint: one heavy command at a time; no shell implementation; no
  OS-isolation dispatch; frozen tests read-only; no parent closure.

## Commands and evidence
- `rtk python3 tools/convergence_gate.py` -> `CONVERGENCE BLOCKED`, `total=92`.
- `rtk git ls-remote origin` -> four cited refs confirmed at cited hashes.
- `shasum -a 256` on `installed_default_entrypoint.rs`/`app012_tool_journey_red.rs`/`native_artifact_manifest.rs` -> match source-evidence rows 4/5/6.
- `rtk git show origin/lane/TOOL-RED-SHELL:crates/tools/tests/phase1_shell_broker.rs | shasum -a 256` -> `2853960b...` (frozen verified artifact exists).
- `command -v timeout` -> not found (exit 127).
- `rtk git diff --check` -> clean.

## No changes
- Only this scratchpad and own ledger row written. No manifest/product/test/Cargo edit.
