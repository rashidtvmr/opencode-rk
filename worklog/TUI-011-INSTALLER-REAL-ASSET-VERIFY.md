# TUI-011-INSTALLER-REAL-ASSET-VERIFY

## Claim
- Task ID: TUI-011-INSTALLER-REAL-ASSET-VERIFY
- Session: ses_f217abd2bfferQRgAbC0R3kY92
- Role: independent verifier (no implementation, no test edits)
- Route/model: tokenharbor/qwen3.8-flash:free (requested 9router/th alias unavailable; delegation authorized tokenharbor provider)
- Commit under verification: 2f1692987ed0e27d928b290d8f2168ea37b81185
- Tree: /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/verify-tui011-real-installer (branch verify/TUI-011-INSTALLER-REAL-ASSET)
- Owned files: worklog/TUI-011-INSTALLER-REAL-ASSET-VERIFY.md + own ledger row only

## Intake
- Read .agents/WORKER.md (full protocol), AGENTS.md (contract in system context), PLAN.md, docs/TDD.md, docs/SECURITY.md, docs/CONVERGENCE.md — summarized below.
- Prior lanes: TUI-011 (blocked, installer security impl), TUI-011-INSTALLER-REAL-ASSET-RED (blocked, frozen test author), TUI-011-INSTALLER-REAL-ASSET (blocked, prior real-asset GREEN on a different base). Ledger confirms no prior row for this VERIFY task; claim succeeded.

## Scope of verification
1. Frozen test hash: tests/e2e/native_archive_install.rs == 0da0e11a1fa0e562e6057f1fdb2568c7a3ea7dee20680ceb68e3f34c516e7932
2. Real archive at /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/real-native-install-9cf998a/oc2-macos-arm64.tar.gz == sha256 4d7a0ded5f3e0610b7cf8eb8b168404225f11827d81be7c9d2ce614379989131
3. rustc --test (std-only) compiles; --list shows exactly 1 test; run with explicit absolute fixture env, disposable HOME/TMPDIR -> expect GREEN 1/1, real installed native binary runs with no DYLD_* env
4. Existing immutable Python native/security installer suites: expect 8/8
5. sh -n on installer script(s); inspect aggregate 128 MiB cap and stage bin+lib ordering before identity check
6. Canonical guard: record baseline (expected 51 errors), change nothing
7. Status: blocked (parent unintegrated + guard baseline) — never completed
8. Land: commit+push scratchpad + own claim only, verifier branch over canonical SSH; no main merge

## Findings (all at exact commit 2f169298)

1. **Frozen test integrity**: `shasum -a 256 tests/e2e/native_archive_install.rs` =
   `0da0e11a1fa0e562e6057f1fdb2568c7a3ea7dee20680ceb68e3f34c516e7932` — MATCHES freeze hash. No test edits (git status shows only claims.json + this scratchpad).
2. **Real archive integrity**: `shasum -a 256 .../real-native-install-9cf998a/oc2-macos-arm64.tar.gz` =
   `4d7a0ded5f3e0610b7cf8eb8b168404225f11827d81be7c9d2ce614379989131` — MATCHES approved SHA. Members: `oc2` (45,447,176 B), `native/lib/macos-arm64/libopentui.dylib`; extracted `oc2` has LC_RPATH `@loader_path/../lib` (otool -l).
3. **Compile/list**: `rustc --edition 2021 --test tests/e2e/native_archive_install.rs -o <approved-temp>/tui011_frozen_test` exit 0 (std-only, no Cargo); `--list` → exactly **1** test (`native_archive_install_succeeds`).
4. **GREEN run**: `env -i PATH=/usr/bin:/bin:/usr/sbin:/sbin HOME=<disposable> TMPDIR=<disposable> OC2_REAL_ARCHIVE=<abs archive> OC2_REAL_CHECKSUM=4d7a0ded... OC2_INSTALLER_SCRIPT=<abs commit script> <test>` → **1 passed; 0 failed** in 0.52 s, exit 0. No `DYLD_*` in the stripped env; installed `bin/oc2 --version` runs via LC_RPATH against sibling `lib/libopentui.dylib`. Script used is the commit-tree script (`scripts/install-oc2.sh`, sha256 `146ae357414908da15eb40b17dccefdd8406e2e969006263ecb6a279b49c97c9`), overriding the default `red-tui011-real-installer` worktree path in the frozen test via explicit env — per task instruction "explicit absolute fixture env".
5. **Existing immutable Python suites**: `python3 -m unittest tests.bootstrap.test_tui011_installer_security tests.bootstrap.test_tui011_installer_native_closure` → **8/8 OK** (0.485 s), zero test edits.
6. **Shell syntax**: `sh -n scripts/install-oc2.sh` and `sh -n scripts/install-opencode2.sh` → exit 0.
7. **Script inspection** (`scripts/install-oc2.sh` @2f16929):
   - `MAX_ARCHIVE_PAYLOAD=134217728` (line 18) = 128 MiB cap; per-member probe (lines 229–243) and post-extraction aggregate re-check `expanded_payload <= MAX_ARCHIVE_PAYLOAD` (lines 260–267).
   - Stage-then-identity order: extraction (line 252) → member regular-file/symlink checks → **staged into `$stage/bin` + `$stage/lib` mirroring installed layout (lines 274–282) BEFORE identity gate** (`--version` lines 285–299, `--help` lines 301–316) → destination transaction (lines 318+) with backup/rollback, symlink-refusal, atomic mv. Confirmed: identity is checked before anything touches the install dir.
   - Independent oversize probe: fabricated 130 MiB single-member archive → installer exits **65**, `bin/` left empty (fail-closed, no partial install).
   - Standalone extracted `oc2` (no adjacent dylib) fails exec (exit 127) — proves the staged bin+lib layout is load-bearing, not decoration.
8. **Canonical guard baseline**: `python3 tools/validate_repository.py` → `validate_backlog_exhaustion: 51 error(s)`, FAIL exit=1 — exactly the recorded baseline of **51 errors**, unchanged; nothing modified to fix it. Output captured to approved-temp `verify-tui011-real-test-2f16929/guard-output.txt`. `tools/convergence_gate.py` also reports off-plan-completed ledger findings (total=84) — parent remains unintegrated.

## Verdict and boundary
- Slice evidence: frozen-test GREEN 1/1 against the real installed native binary at exact commit 2f169298, 8/8 immutable installer suites, syntax and cap/order checks pass.
- This is a **candidate receipt, not acceptance** (AGENTS.md: verifier decides, worker never claims `passes:true` as proof). Parent TUI-011-INSTALLER-REAL-ASSET remains open: canonical guard baseline 51 errors, Linux installed journey, provenance, and integration into `main` remain unresolved; branch `verify/TUI-011-INSTALLER-REAL-ASSET` is ~109 commits ahead of `main` per prior lane note.
- Ledger status set to **blocked** (verification complete but parent/gate blocked), with bounded note. Committed scratchpad + own claim only to the verifier branch over canonical SSH remote (`git@github.com:rashidtvmr/opencode-rk.git`); no merge to `main`, no release.

## Resources
- Test run 0.52 s; Python suites 0.485 s; rustc compile ~5 s; guard ~2 s. Single heavy command at a time, `CARGO_*` untouched (no Cargo invoked). System memory free 58 % before/after (16 GB pages host, well within 8 GiB interactive budget). All artifacts under approved temp `.../T/opencode/verify-tui011-real-test-2f16929`.
