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

## Signal/layout follow-up (superseded prior evidence)

The independent follow-up source is
`tests/bootstrap/test_tui011_installer_signal_layout.py`. It adds two bounded
contracts: a self-targeted TERM immediately after native replacement must
restore both old files, and a normal producer-shaped archive with the three
intermediate native directory members must install successfully. The
directory-entry shape is directly supported by the historical producer
revision `2f1692987ed0e27d928b290d8f2168ea37b81185` and is intentionally
separate from the exact two-member rollback probe above.

Follow-up source SHA-256:
`db1b32d5472cf5c2b23bac079cf7eca7ad622667fe21b80335e3f94efefb237a`.

The prior WIP script was snapshotted outside the checkout before probing. Run:

```text
TUI011_INSTALLER_SCRIPT=<external-snapshot>/install-oc2.sh \
  rtk /usr/bin/arch -arm64 /usr/bin/python3 \
  tests/bootstrap/test_tui011_installer_signal_layout.py
```

The previous `d2ec2a13...`, `f468ef3c...`, and `ea01fe4d...` signal results are
superseded; their fixtures did not establish correctly targeted signal
injection. The former hash-conditional test and its contradictory assertions
are also superseded. The frozen test below has one expectation on every
script: exact final native destination, `.libopentui.tmp.*` source basename,
successful real move, injected snapshot equal to `new-native`, exit 143, and
byte-identical old binary/native after rollback. Its wrapper delegates each
`mv`, bounds the trace, and injects TERM exactly once only after the successful
native replacement; rollback moves therefore cannot retrigger injection.

Against immutable baseline `6962df6e...` and candidate `92525d44...`, the
fixed command was:

```text
rtk /usr/bin/arch -arm64 env TUI011_INSTALLER_SCRIPT=/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-installer-before-rollback-osbxbydr/install-oc2.sh /usr/bin/python3 tests/bootstrap/test_tui011_installer_signal_layout.py
```

Baseline result (clean semantic RED; signal assertion fails because the old
script returns 0 after the injected TERM and does not rollback; nested archive
also remains rejected):

```text
Ran 2 tests in 0.236s
FAILED (failures=2)
signal: AssertionError: 0 != 143; trace shows the exact `.libopentui.tmp.*`
  source moved successfully to the exact final native target
nested: AssertionError: 65 != 0; invalid archive: expected bounded safe members
```

Injection is proven independently by the marker, source basename, successful
move trace, and pre-TERM evidence snapshot; no baseline hash is consulted.
Candidate result with this frozen test source: `Ran 2 tests in 0.308s`, `OK`.
This is candidate GREEN only; it is not source acceptance until main commits the
source and a separate verifier reruns the exact integrated revision.

An earlier non-baseline probe reported unguarded recursive `chmod` errors:

```text
chmod: .../native/lib: Permission denied
chmod: .../native/lib/macos-arm64: Permission denied
```

Earlier snapshot probing established the direct four-member archive-gate RED:
`invalid archive: expected bounded safe members`.

This confirms the current four-member gate rejects the historical five-member
directory-entry layout. The immutable baseline SHA is
`6962df6e9998dcf86c7b7ba2793f4e9d6fe3289e36833483ed6e5757246bdf97`.

The previous hash `d2ec2a13c0775405104230e8137f87b3c2ed235f5c56c217d72ed603bcfe6082`
is superseded: its signal wrapper returned status 1 for ordinary delegated
`mv` calls and compared an unnormalized `install/../lib` spelling, so its
reported signal GREEN was not valid evidence.
