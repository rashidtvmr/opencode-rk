# TUI-011 installer security RED

Status: RED frozen; implementation blocked.

## Claim
- Task: TUI-011
- Session: `ses_f32a08b10ffelo27wIOmkq2CYq`
- Owned test file: `tests/bootstrap/test_tui011_installer_security.py`
- Base commit: 764087b
- Existing frozen test untouched: `tests/bootstrap/test_tui011_installer_native_closure.py`
  SHA-256 `fe5543977bdfb3dd435d240f131b4e2174466225339e536824b21f273a12f1d5`

## Source evidence (scripts/install-oc2.sh @ 764087b)

- :275-276 `mkdir -p "$INSTALL_DIR" "$native_dir"`: follows pre-existing symlink dirs; installs can write outside root.
- :279-286 `mktemp "$INSTALL_DIR/.oc2.tmp.XXXXXX"` / `mktemp "$native_dir/.libopentui.tmp.XXXXXX"`: temp files created in dest dirs before transaction.
- :65-69 `cleanup()`: only removes `stage`; does NOT remove `binary_tmp`/`native_tmp`.
- :108-118 `uninstall()`: removes only `$INSTALL_DIR/oc2`; sibling `../lib/libopentui.{so,dylib}` remains.
- :306-316 `mv "$target" "$binary_backup"` / `mv "$native_target" "$native_backup"`: backs up by moving the link object, not following it (good -- preserves symlink).

## Defects to reproduce (frozen RED)

1. Pre-existing symlinked `INSTALL_DIR` (bin) and/or sibling `lib` dir rejected before writes. Assert no file outside selected root; prior targets unchanged.
2. `--uninstall` removes both `oc2` and matching platform sibling `libopentui`, preserves unrelated files, does NOT follow target-file symlinks.
3. Controlled failure after dest temp creation: no `.oc2.tmp.*` / `.libopentui.tmp.*` left; old binary/library bytes restored.

## Frozen RED

- New test: `tests/bootstrap/test_tui011_installer_security.py`
- SHA-256: `c1c8fa57017612db43e6a7aaafa4945f1bba077405ee8bc541e0c50abf2a50a4`
- `python3 -m py_compile tests/bootstrap/test_tui011_installer_security.py`: pass.
- `python3 -m unittest -v tests.bootstrap.test_tui011_installer_security`:
  3 tests run, 4 expected failing assertions:
  - symlinked bin destination accepted and written through;
  - symlinked lib destination accepted and written through;
  - third `mktemp` failure leaves `.oc2.tmp.*` residue;
  - uninstall leaves `libopentui.dylib` on the tested macOS arm64 host.
- Fault injection is deterministic: a disposable PATH wrapper forwards to the
  fixed system `mktemp` and fails only its third invocation (after stage and
  binary temp creation). Old binary/library bytes remain unchanged.
- All paths are disposable `TemporaryDirectory` children; no real install path,
  user data, credentials, or network are used.

## Existing frozen regression

- Existing test SHA remains
  `fe5543977bdfb3dd435d240f131b4e2174466225339e536824b21f273a12f1d5`.
- `python3 -m unittest -v tests.bootstrap.test_tui011_installer_native_closure`:
  5 passed, 0 failed.

## Integration constraint

Current `origin/main` commit `767a86a` adds an installed `oc2 --help` identity
gate, while this lane's candidate validates staged `oc2 --version`. The product
repair and conflict resolution must preserve both contracts, preferably while
still staged so identity failure cannot partially replace an installation.

The first delegated attempt stopped with a 74-line zero-test file; it was not
frozen or committed. The main integrator completed the real behavioral RED above
without changing product code or the existing frozen test.
