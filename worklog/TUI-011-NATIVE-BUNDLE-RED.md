# TUI-011 native bundle RED

Status: blocked. Test-author lane only. Product files untouched.

## Claim

- Task: TUI-011
- Session: `ses_f3318a12affe4ygqfER51PhtXn`
- Candidate start: `85b7cc4719c7e79bb631870bf0568bdbcd6b73ac`
- Owned test: `tests/bootstrap/test_tui011_installer_native_closure.py`

## Source evidence

- `scripts/install-oc2.sh:72-80`: checksum gate runs before writes; mismatch exits 65.
- `scripts/install-oc2.sh:88-98`: archive extracts to a temporary stage, then copies only `oc2` into `INSTALL_DIR`.
- `scripts/install-oc2.sh:99-121`: installed `--version` identity gate removes `oc2` on failure; legacy identity exits 74.
- `worklog/TUI-011-PROD-AUDIT.md:97-108`: current installer installs no native closure; relocatable Unix target is `$INSTALL_DIR/../lib`.
- `tasks/completion/tui.json:14`: TUI-011 requires clean-target native-library resolution and reproducible distribution.

## Observable contract

Archive members are bounded and contain executable `oc2` plus
`native/lib/<host-platform>/libopentui.so` on Linux or `.dylib` on macOS.
Installer places the binary at `INSTALL_DIR/oc2` and the selected native library
at `INSTALL_DIR/../lib/libopentui.{so,dylib}`. This sibling path is relocatable
for an executable with `$ORIGIN/../lib` or `@executable_path/../lib` loader data.

Checksum and identity failures remain fail-closed. Malformed archives, missing
native closure, and traversal members install no partial binary. Fixture paths
contain spaces. The subprocess receives only fixture `HOME`, `TMPDIR`, `PATH`,
and `LC_ALL`; no network or existing OpenCode DB is used.

## RED plan

Run:

```sh
python3 tests/bootstrap/test_tui011_installer_native_closure.py
```

Observed RED: `python3 tests/bootstrap/test_tui011_installer_native_closure.py`
ran 5 tests, 1 failed. Only
`test_native_bundle_and_missing_library_are_atomic` failed. Its three failures
are the same missing installer-native-closure behavior: valid archive omitted
`libopentui.dylib`, archive without native library was accepted, and that
accepted archive left `oc2` installed. Checksum, identity, malformed archive,
and traversal cases passed.

Frozen test SHA-256: `fe5543977bdfb3dd435d240f131b4e2174466225339e536824b21f273a12f1d5`

## Remaining

- Capture final test hash after this scratchpad update.
- Ledger status `blocked`, pending `scripts/install-oc2.sh` implementation.
- Commit only this test, this scratchpad, and own ledger row; push
  `origin/lane/TUI-011-prod`.
