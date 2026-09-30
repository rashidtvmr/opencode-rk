# V2-UBUNTU-INSTALLER: native-library closure and transaction recovery

The original implementer report below is historical and incomplete. Main took
over the source repair after the independent second-rename contract exposed
replacement of the old native library despite retaining the old executable.
The follow-up section records the current transaction implementation; the older
claim that rollback/signals were unnecessary is superseded.

Package: installer closure (G7 slice, archive contract only).
Base: b323e2b. Frozen test: tests/bootstrap/test_tui011_installer_native_closure.py
from 1024754 (223 lines, sha256 fe554397...), committed f1070d1. No test edits.

## Failure observed
`python3 -m unittest tests.bootstrap.test_tui011_installer_native_closure -v`
on base: 4 pass, 1 fail
(`test_native_bundle_and_missing_library_are_atomic`: valid bundle installed no
`bin/../lib/libopentui.*`; native-missing archive accepted with partial binary).

## Source evidence (salvage, not invention)
- 2f1692987ed0e27d928b290d8f2168ea37b81185 (parent cc3ebaa): +16/-3 delta adds
  staged `bin/../lib` mirror before `--version` identity so a dynamic oc2
  resolves its relative loader path during the staged check (exit 74 preserved).
- cc3ebaa baseline already held: exact `oc2` + `native/lib/<platform>/libopentui.{so,dylib}`
  member gate, 4-member bound, traversal/type/probe gates, staged identity.
- 2f87242 (receipt consumer, NOT ancestor of 2f16929) deliberately NOT salvaged:
  it adds `--manifest`/receipt verification, a producer-absent contract this
  slice must not require. This lane stays an unsigned integrity consumer.
- tests/e2e/macos_loader.rs:17 cites `scripts/install-oc2.sh:309` as the
  `bin/`+`../lib/` layout authority; this change makes that line reference true
  again (native_target now at INSTALL_DIR/../lib).

## Change (scripts/install-oc2.sh only, 144 -> 318 lines)
Salvaged minimal closure from cc3ebaa/2f16929, preserving later canonical checks:
- `MAX_ARCHIVE_MEMBERS=4`, `MAX_ARCHIVE_PAYLOAD=134217728` (admits ~72 MiB real
  assets; frozen test uses 1 MiB self-bound fixtures, unaffected).
- `EXPECTED_NATIVE=native/lib/$PLATFORM/$NATIVE_NAME` (.so linux, .dylib macos).
- Listing/type/probe gates (exit 65), symlink-member refusal, post-extract
  payload sum check; checksum gate still before any write (exit 65);
  legacy `opencode` refusal (exit 73) kept.
- Staged identity (`--version`/`--help`, `opencode-rk` rejection) now runs on
  `$stage/bin/oc2` with `$stage/lib` sibling, exit 74, install untouched.
  Deliberate delta vs 2f16929: no `mv` of payload into `$stage/bin|lib` before
  identity; `cp` to temp files at install time instead, same observable layout.
- Atomic swap: tmp files beside targets, native installed BEFORE binary (a
  binary-swap failure cannot orphan a new binary without its lib; fresh-install
  binary failure removes the new lib). Upgrade preserves history notice.
  Deliberate delta vs 2f16929: no backup/restore, no signal traps; failure
  before either rename leaves old files untouched, which is all the frozen
  contract requires.
- `uninstall` removes both binary and sibling lib; refuses symlinked dest dirs
  (exit 74). No manifest/receipt flags added.

## Verification
- `sh -n scripts/install-oc2.sh` -> SYNTAX-OK.
- Frozen gate: `python3 -m unittest
  tests.bootstrap.test_tui011_installer_native_closure -v` -> 5/5 OK.
- Extra disposable probe (temp dirs only, spaces in names, no Cargo/Docker):
  symlink-native rejected, duplicate-bin rejected, bad-upgrade (exit 3 binary)
  returns 74 with old binary+lib byte-preserved, uninstall removes both,
  symlink-dest uninstall refused 74 -> PASS.
- `git diff --check` -> clean.
- No other installer Python gates exist in-tree (no test_tui011_installer_security
  module, no rel002/packaging python gate referencing install-oc2.sh).

## Not claimed
Full G7/G8: real Ubuntu archive journey (real oc2 + real libopentui.so,
`readelf/ldd` RUNPATH `$ORIGIN/../lib`, golden path) still required on a Linux
runner. No Cargo/Docker builds run here.

## Main transaction follow-up

- Role/package: installer source implementer, G7 archive/installation transaction
  slice. Canonical integration and acceptance are separate subsequent steps.
- Source repair base: `33a934f` (independent rollback contract frozen after its
  clean RED). The immutable pre-transaction script is preserved externally with
  SHA-256 `6962df6e9998dcf86c7b7ba2793f4e9d6fe3289e36833483ed6e5757246bdf97`.
- Product source path: `scripts/install-oc2.sh`. The independent contract owner
  owns signal/layout assertions and its own contract worklog.
- Current implementation makes copies of both old files before any replacement,
  stages both new files next to their destinations, and arms rollback before
  each replacement syscall. Failed installation or HUP/INT/TERM exits through
  cleanup, restoring both old files. If restoring by rename fails, a copy
  fallback is attempted; an unrestorable backup is retained and reported.
- The archive gate admits only the two exact regular payloads and their three
  optional directory ancestors, bounded to five members and 128 MiB total
  expanded payload. Traversal, link, duplicate, wrong-platform and missing-native
  gates remain fail-closed. Staging directory permissions are repaired from
  parent to child before traversal, including producer-shaped mode-000 directory
  fixtures.
- Authority: historical producer `2f1692987ed0e27d928b290d8f2168ea37b81185`,
  real native loader layout, and the independently frozen rollback requirement.
  No receipt/manifest requirement is added to the unsigned integrity consumer.
- Independent test-owner correction committed as
  `13616a440808d40947192126454140d955161986`; frozen signal/layout SHA-256:
  `db1b32d5472cf5c2b23bac079cf7eca7ad622667fe21b80335e3f94efefb237a`.
  The same fixed assertions now fail against the immutable baseline (signal
  returns 0 rather than 143 after proven replacement; nested layout exits 65),
  and pass against this candidate. Source-hash-dependent assertions and
  contradictory final-byte checks in the previous fixture are superseded.
- Main rehashed actual source as
  `92525d449da6b5f2737e26708851d268c6741280e4bf7ce6e814697c5cdc08fe`
  and ran:

  ```text
  rtk /usr/bin/arch -arm64 /usr/bin/python3 -m unittest tests.bootstrap.test_tui011_installer_native_closure tests.bootstrap.test_tui011_installer_rollback tests.bootstrap.test_tui011_installer_signal_layout
  /bin/sh -n scripts/install-oc2.sh
  git diff --check
  ```

  Results: **8/8 passed**, shell syntax GREEN, diff check GREEN.
- State: source **CANDIDATE** over the frozen test revision `13616a4`.
  Independent verification on committed source is required before integration;
  only the exact integrated gate can confer acceptance. The real release-built
  Ubuntu archive/golden journey and full G7/G8 remain pending.
