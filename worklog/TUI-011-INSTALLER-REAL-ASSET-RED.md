# TUI-011-INSTALLER-REAL-ASSET-RED

## Claim
- **Task ID**: TUI-011-INSTALLER-REAL-ASSET-RED
- **Session**: ses_f21984612ffeQx7yaOWDgyvMEB
- **Branch**: red/TUI-011-INSTALLER-REAL-ASSET (worktree at /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/red-tui011-real-installer)
- **Base commit**: 9cf998a5512f86e5f88c2f57280fc79d1f22c19a
- **Role**: Test author (RED only; no product code edits)

## Source Evidence (verified at HEAD 9cf998a)
- `scripts/install-oc2.sh:16` — `MAX_ARCHIVE_PAYLOAD=1048576` (1 MB hard cap)
- `scripts/install-oc2.sh:224-246` — per-member payload probe via `tar -xOf | head -c $((MAX_ARCHIVE_PAYLOAD + 1)) | wc -c`; checks `[ -s "$probe_err" ]` at line 230, then `[ "$probe_bytes" -gt "$MAX_ARCHIVE_PAYLOAD" ]` at line 237
- `scripts/install-oc2.sh:308-310` — installs `$BIN` to `$INSTALL_DIR/$BIN` and native lib to `$INSTALL_DIR/../lib/$NATIVE_NAME`
- Archive fixture: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/real-native-install-9cf998a/oc2-macos-arm64.tar.gz`
- Archive SHA-256: `4d7a0ded5f3e0610b7cf8eb8b168404225f11827d81be7c9d2ce614379989131` (verified via `shasum -a 256`)
- Archive members: `oc2` (45,447,176 bytes), `native/lib/macos-arm64/libopentui.dylib` (26,367,408 bytes)
- Binary LC_RPATH: `@loader_path/../lib` (verified via `otool -l`)
- Binary `--version` output: `oc2 0.1.0-alpha.1` (exit 0 with dylib at `bin/../lib/`)

## Observed Scenario (RED — verified)
Running `sh scripts/install-oc2.sh --archive <real-archive> --checksum <real-sha256> --install-dir <fresh>/bin` exits 65:
- The probe at line 229 runs `tar -xOf "$ARCHIVE" "$member" 2>"$probe_err" | head -c $((MAX_ARCHIVE_PAYLOAD + 1)) | wc -c`
- `head -c` closes the pipe early, causing `tar` to emit a broken-pipe error to `probe_err`
- `[ -s "$probe_err" ]` at line 230 is true (non-empty error), causing `exit 65` with message "invalid archive: cannot read oc2"

## Contract Under Test (desired GREEN)
1. Accept the real native archive via `--archive` + `--checksum`
2. Exit 0 on success
3. Install `bin/oc2` and `../lib/libopentui.dylib` into disposable install dir
4. `bin/oc2 --version` exits 0 with NO `DYLD_*` env vars (relies on LC_RPATH `@loader_path/../lib`)
5. Sibling `../lib/libopentui.dylib` exists post-install

## Observable Failure State (RED — verified)
- Script exits 65 at the probe (line 230 broken-pipe check, or line 237 size comparison)
- No files are written to the install dir
- `bin/oc2 --version` cannot run (no install)

## Safety Constraints (fixed before freeze)
- `TempDir::new`: uses `create_dir` (not `create_dir_all`) with retry on AlreadyExists; unique name per pid+id; never deletes pre-existing path
- `TMPDIR` env for BOTH child commands set to owned `tmp_dir.path()`, not inherited generic `/tmp`
- `env_clear()` children with ONLY `PATH`, `HOME`, `TMPDIR`, `TERM` exported
- NO `DYLD_*` env vars
- Child bounded by 30-second timeout via `wait_or_kill`; kill+wait owned child on timeout
- stdout/stderr NULL (exit status only)
- No user DB or real home access (disposable HOME)

## Test Plan
- File: `tests/e2e/native_archive_install.rs`
- Compiles via `rustc --edition 2021 --test` (std-only, no Cargo build)
- 1 test: `native_archive_install_succeeds`
- Frozen SHA-256 (post-safety-fix): `0da0e11a1fa0e562e6057f1fdb2568c7a3ea7dee20680ceb68e3f34c516e7932`
- Fixture paths via env: `OC2_REAL_ARCHIVE`, `OC2_REAL_CHECKSUM`, `OC2_INSTALLER_SCRIPT` (all absolute defaults)

## RED Evidence (verified at freeze)
```
rtk rustc --edition 2021 --test tests/e2e/native_archive_install.rs -o /tmp/tui011-approved/test
  -> compiled cleanly, 0 errors

/tmp/tui011-approved/test --list
  -> native_archive_install_succeeds: test
  -> 1 test, 0 benchmarks

env -i PATH=... OC2_REAL_ARCHIVE=... OC2_REAL_CHECKSUM=... OC2_INSTALLER_SCRIPT=... /tmp/tui011-approved/test
  -> test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 filtered out
  -> panicked at native_archive_install.rs:217:5:
     install-oc2.sh must exit 0 (success); got status exit status: 65;
     current RED: MAX_ARCHIVE_PAYLOAD=1048576 rejects real 45MB binary (exit 65 at size probe)
  -> EXIT: 101
```

The test FAILS because the installer exits 65 (genuine RED on the missing behavior — the installer script's size probe rejects the real archive). This is NOT a syntax error, NOT a compile error, and NOT a missing fixture.

## Status
- RED verified: compiles, lists 1 test, runs and fails on `status.success()` assertion (installer exit 65)
- Frozen SHA-256: 0da0e11a1fa0e562e6057f1fdb2568c7a3ea7dee20680ceb68e3f34c516e7932
- RED-only cannot complete (test author lane only; LC_RPATH fix is a separate implementer lane)
- Ledger: blocked with exact note (RED-only, frozen, genuine failure captured)