# V2 Ubuntu installer rollback contract

Independent test owner contract for the concrete WIP rollback defect.

- Script under test: `scripts/install-oc2.sh` (read-only for this package).
- Historical authority: `2f1692987ed0e27d928b290d8f2168ea37b81185`, whose
  installer has paired binary/native backups, rollback, and signal traps.
- The fixture creates a disposable archive with exactly two regular tar members:
  `oc2` and `native/lib/<host-profile>/libopentui.<suffix>`; no recursive
  directory entries are included in this first rollback probe.
- Existing binary and native library bytes are staged only below a disposable
  temporary HOME. A PATH `mv` wrapper fails only for the exact final `oc2`
  target; all other moves delegate to the real system `mv`.
- The frozen assertion requires nonzero exit and byte-identical preservation of
  both pre-existing files after the native replacement succeeds and binary
  replacement fails. This is the semantic RED: current WIP removes/replaces
  the old native library and does not restore it.
- The original installer frozen five-test artifact is untouched.

## RED command manifest

```text
rtk /usr/bin/arch -arm64 /usr/bin/python3 -m unittest tests.bootstrap.test_tui011_installer_rollback
```

The test is bounded by a 15-second subprocess timeout and captures combined
installer output.

Frozen test source SHA-256:
`de493cfd0515ba66be7f9ba1780334bc1761da46824f1d4c00330cf91f191d6c`.

Exact RED command/result against the current WIP script:

```text
rtk /usr/bin/arch -arm64 /usr/bin/python3 tests/bootstrap/test_tui011_installer_rollback.py
=> FAIL (1 test, 0.110s)
AssertionError: b'new native library bytes' != b'OLD-NATIVE-BYTES'
installer output:
platform: macos-arm64
upgrade: preserving existing install (history lives in user data dir, untouched)
FAIL: cannot install binary; install unchanged
```

This is a clean semantic RED: the forced second rename fails, the command is
nonzero, the old binary remains intact, and the old native library is replaced.
The WIP script SHA-256 used by the probe is:
`6962df6e9998dcf86c7b7ba2793f4e9d6fe3289e36833483ed6e5757246bdf97`.
