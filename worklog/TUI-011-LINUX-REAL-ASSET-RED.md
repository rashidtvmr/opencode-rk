# Worklog: TUI-011-LINUX-REAL-ASSET-RED (test-author, RED-only)

## Claim
- Task: TUI-011-LINUX-REAL-ASSET-RED, role test-author.
- Session: ses_f216fb1d9ffeNe31FO5CzQwbw9; scratchpad: worklog/TUI-011-LINUX-REAL-ASSET-RED.md.
- Ledger: claimed in-progress before touching any file; final status blocked
  (see below). No other lane files touched.
- Route/allowlist: delegation names user-requested Muse Spark 1.3 Contributor
  via available OpenCode provider with no user allowlist (canonical N/A);
  assigned route matches delegation, no restriction applies.

## Source evidence (repo commit 2f169298, branch red/TUI-011-LINUX-REAL-ASSET)
- `scripts/install-oc2.sh:15` `MAX_ARCHIVE_MEMBERS=4`;
  `:18` `MAX_ARCHIVE_PAYLOAD=134217728` (128 MiB cap, Linux path).
- `scripts/install-oc2.sh:149-154`: linux-* -> `LIB_SUFFIX=.so`,
  `EXPECTED_NATIVE=native/lib/$PLATFORM/libopentui.so`.
- `scripts/install-oc2.sh:229-246`: per-member payload probe; exit 65.
- `scripts/install-oc2.sh:261-269`: staged payload sum check; exit 65.
- `scripts/install-oc2.sh:271-301`: stage bin/../lib mirror, staged
  `oc2 --version` identity (`*oc2*`, reject `*opencode-rk*`); exit 74.
- `scripts/install-oc2.sh:321-323`: `$BIN` to `INSTALL_DIR/oc2`, native lib
  to `INSTALL_DIR/../lib/libopentui.so`; `:381-382` print identity, exit 0.
- Frozen Mac reference `tests/e2e/native_archive_install.rs` (behavior only,
  not edited): env-prereq pattern, env_clear + fresh HOME/TMPDIR, owned
  child with deadline, exit-0 + lib + `--version` assertions.

## Observed scenario (untrusted artifact, not authority)
- Real Linux RED receipt
  `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/linux-native-2f16929/nomad-1d6bf109-receipt.txt`
  SHA256 2a02745041d8b3b35335c55a2079837beac715ec34dbc6c7b850d16ceadee810
  (verified locally): source-built oc2 118773880 B (ELF RUNPATH
  `$ORIGIN/../lib`, `--version` ok from disposable bin/../lib) +
  libopentui.so 26583552 B = 145357432 B > 134217728 B cap; rebuilt archive
  SHA256 8dc056db07e4b4e1778b590831b5e984edc6ea1ea4fec1efea3a29ffcf1e3d40;
  checksum-valid install exits 65 (`expanded payload exceeds 134217728`).
- Artifact lived only inside the expired Nomad allocation; no fixture is
  claimed to exist now. Missing fixture is a panic blocker, not RED.

## Target boundary
- Owned file: `tests/e2e/linux_native_archive_install.rs` (new, 160 lines,
  std-only, Linux-only via `#[cfg(target_os = "linux")]` on the test fn).
- Contract: real archive/csum/script via `OC2_LINUX_ARCHIVE` /
  `OC2_LINUX_CHECKSUM` / `OC2_INSTALLER_SCRIPT`; run actual install-oc2.sh
  with fresh disposable HOME/TMPDIR and `<root>/bin` prefix; 45 s owned-child
  deadline (kill+reap only owned child); stdout/stderr bounded (null for
  installer, piped + 8192 B cap for `--version`); assert exit 0, installed
  `lib/libopentui.so`, `bin/oc2 --version` exit 0 under env_clear with no
  `LD_LIBRARY_PATH`, identity contains `oc2`.

## Tests
- Mac syntax check: `rustc --edition 2021 --test
  tests/e2e/linux_native_archive_install.rs` -> exit 0 (only expected
  dead-code warnings: Linux-gated test fn compiled out on Mac; helpers fully
  type-checked). Binary `--list` -> `0 tests, 0 benchmarks` (expected: host
  is darwin, test is Linux-only).
- DRAFT sha256 2b89898b32f1d4b78752d09161a4e39e813f2dfb8620bbd8ac1707cf3750cfc9.
  NOT frozen: freeze requires an actual Linux run with >=1 test discovered.
- Linux behavioral RED is orchestrator-owned next step on the bounded Lenovo
  Nomad job that rebuilds the real archive. No Nomad/Cargo launched here.

## Decisions
- `#[cfg]` on the test fn (not whole file) so Mac still type-checks helpers.
- Defaults point at expected rebuild paths; receipt archive SHA kept as the
  checksum default (orchestrator overrides via env regardless).
- No Cargo.toml/registration edit: e2e files are standalone `rustc --test`
  targets like the Mac precedent; Cargo.toml is out of lane authority.

## Remaining unknowns / open RED gate
- Blocked: `Linux RED pending real archive` — needs orchestrator-coordinated
  Linux run: rebuild real archive, set env, `rustc --edition 2021 --test`
  + run, expect 1 test discovered and FAILING (exit 65) until installer cap
  is fixed by the implementer lane.
- Convergence gate baseline (pre-existing, not caused by this lane):
  BLOCKED, 84 ledger findings (`python3 tools/convergence_gate.py` exit 1).
