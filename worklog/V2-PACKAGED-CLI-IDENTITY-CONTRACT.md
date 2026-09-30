# APP-010/G7 packaged CLI identity contract

## Scope

The frozen executable contract applies to an owned copy staged at
`install/bin/oc2`.  If supplied, `OC2_TEST_NATIVE_LIBRARY` must be an absolute,
readable regular `.dylib` or `.so`; the fixture copies it to
`install/lib/libopentui.<suffix>` for both inspections.  No library is required
for the development baseline.  It runs `--version` and `--help` with stdin disconnected,
an explicit disposable HOME/XDG/data/runtime/project/PATH environment, a
three-second timeout, and a 64 KiB retained-output bound.  Neither inspection
command may create a daemon descriptor (`backend.json`) or a runtime/session
database.  Output must identify `oc2`, must not contain `opencode-rk` or
`opencode2`, and help must contain `Usage: oc2`.

## Evidence and RED

- Base SHA: `9a8732a5b79e7123d3646b51d636a121016be61d`
- Baseline installed binary: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-integrated-f5cfb01-np94vwxp/bin/oc2`
- Baseline file SHA: `1870d7627875ae450d2dcea2691d7b00e2127a30860726efeb2b43cdab434321`
- Observed baseline `--version`: `opencode-rk 0.1.0-alpha.1`
- Observed baseline `--help` included `Usage: oc2` and did not contain either
  forbidden literal.  The help inspection is therefore a safe control and
  passes on the baseline; the version test fails on the stale product identity
  as required.

Exact RED command (no native companion is needed for this development
baseline):

```sh
OC2_TEST_BINARY=/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-integrated-f5cfb01-np94vwxp/bin/oc2 \
  python3 -m unittest tests.bootstrap.test_app010_packaged_cli_identity
```

Result: `Ran 2 tests ... FAILED` with one expected version-identity assertion
failure and one passing help control; no build, Cargo, Docker, native
dependency, or user database was used.  A release invocation may additionally
set `OC2_TEST_NATIVE_LIBRARY=/absolute/path/to/libopentui.dylib` (or `.so`),
which is copied into the disposable install layout without changing the fixed
runtime environment.

## Frozen artifact

The frozen test source is this file:
`tests/bootstrap/test_app010_packaged_cli_identity.py`.

Updated mechanical fixture candidate: the test source SHA-256 and commit are
recorded by the handoff.  This package is a test contract, not a product fix and
is neither PREVERIFIED nor ACCEPTED; the integrator must independently rerun
the frozen tests after selecting the historical CLI identity fix and building
the release artifact.
