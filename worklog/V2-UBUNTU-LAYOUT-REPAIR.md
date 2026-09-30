# Ubuntu installer directory-header repair

Package: G7 installer nested-layout portability.

Base integrated revision: `0b00ae430c0ddddfe8cc70efa6707c1ac46d51b1`.

## Observable failure and frozen contract

The independent Ubuntu verifier ran the same ten identity/installer controls
against the installed arm64 native release and integrated installer. Nine passed;
`test_historical_nested_directory_entries_are_accepted` failed with exit 65,
`invalid archive: extraction failed`. The frozen signal/layout test SHA-256 is
`db1b32d5472cf5c2b23bac079cf7eca7ad622667fe21b80335e3f94efefb237a`.
No test assertions were changed.

The test's historical producer-shaped archive contains two regular payloads and
three directory headers with mode 000. A direct GNU tar reproduction in the
disposable Ubuntu fixture returned exit 2 with `Cannot mkdir: Permission denied`
and `Cannot open: Permission denied`. Evidence is retained in
`/work/ubuntu-layout-reproduction/extract.log` in container
`oc2-v2-ubuntu-build` (approved temporary host bind mount).

Authority: the approved cross-platform installer contract and preserved producer
`2f1692987ed0e27d928b290d8f2168ea37b81185`; this is rk packaging behavior rather
than an upstream OpenCode API.

## Repair

Only `scripts/install-oc2.sh` changes product behavior. After the unchanged safe
member/type and expansion-budget gates, extract the two explicitly named regular
payloads. Tar creates private staging ancestors without applying the optional
directory headers' restrictive modes. Existing checks, relative native layout,
identity validation, paired replacement and rollback remain exercised by the
frozen contracts.

Baseline script SHA-256:
`92525d449da6b5f2737e26708851d268c6741280e4bf7ce6e814697c5cdc08fe`.
Candidate script SHA-256:
`9b7f259dc02bb38e86cac52482e0ac01c2c09074c648533f3e6ec6d255c07f16`.
Both copies and frozen input hashes are preserved at
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-ubuntu-layout-repair-ygpjf5bx`.

## Candidate checks

```text
docker exec --workdir /work/source -e TUI011_INSTALLER_SCRIPT=/work/ubuntu-layout-candidate/install-oc2.sh -e OC2_TEST_BINARY=/work/ubuntu-installed-verified/install/bin/oc2 -e OC2_TEST_NATIVE_LIBRARY=/work/ubuntu-installed-verified/install/lib/libopentui.so -e TMPDIR=/work/verify-tmp oc2-v2-ubuntu-build python3 -m unittest tests.bootstrap.test_app010_packaged_cli_identity tests.bootstrap.test_tui011_installer_native_closure tests.bootstrap.test_tui011_installer_rollback tests.bootstrap.test_tui011_installer_signal_layout
rtk /usr/bin/arch -arm64 /usr/bin/python3 -m unittest tests.bootstrap.test_tui011_installer_native_closure tests.bootstrap.test_tui011_installer_rollback tests.bootstrap.test_tui011_installer_signal_layout
/bin/sh -n scripts/install-oc2.sh
```

Results: Ubuntu **10/10 passed**; Mac installer **8/8 passed**; shell syntax GREEN.
The Ubuntu release binary remains the recorded `0b00ae4` build; these results
verify the installer correction, not a new native release journey. Independent
verification on the exact integrated script revision remains required.
