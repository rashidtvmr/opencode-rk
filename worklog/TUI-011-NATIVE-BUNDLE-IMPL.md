# TUI-011 native bundle implementation

Status: completed candidate; parent acceptance remains blocked.

## Claim

- Task: TUI-011
- Session: `ses_f32f0afd0ffeXqSxD78n7xuyZs`
- Candidate revision: `e25006d` plus local implementation changes; final commit reported below.
- Owned product file: `scripts/install-oc2.sh`
- Frozen test SHA-256: `fe5543977bdfb3dd435d240f131b4e2174466225339e536824b21f273a12f1d5`

## Source evidence

- `scripts/install-oc2.sh:72-121`: checksum, archive extraction, binary copy, identity gate, exit contracts.
- `worklog/TUI-011-PROD-AUDIT.md:97-108`: installed native library belongs at `$INSTALL_DIR/../lib`; current installer omitted it.
- `tests/bootstrap/test_tui011_installer_native_closure.py:13-25`: supported profiles, four-member bound, 1 MiB payload, suffix contract.
- `tests/bootstrap/test_tui011_installer_native_closure.py:107-153`: valid native closure and missing-library atomicity.

## Contract implemented

- Detects only `linux-x64`, `linux-arm64`, `macos-x64`, `macos-arm64`.
- Validates tar integrity, member count, regular-file types, exact required regular files, safe relative names, and 1 MiB expanded payload before extraction.
- Stages and identity-validates `oc2` before destination writes.
- Installs binary and sibling native library with same-filesystem backups and rollback. No `eval`.

## Verification

- RED receipt: `python3 tests/bootstrap/test_tui011_installer_native_closure.py` 1 failed / 5, frozen SHA above.
- GREEN: `sh -n scripts/install-oc2.sh`; `python3 tests/bootstrap/test_tui011_installer_native_closure.py` -> 5/5.
- Frozen test SHA rechecked: `fe5543977bdfb3dd435d240f131b4e2174466225339e536824b21f273a12f1d5`.
- Oversize disposable fixture rejected with exit 65 before install directory creation.
- Parent remains blocked for rpath, cross-architecture native artifacts, SBOM, and signing per `worklog/TUI-011-PROD-AUDIT.md:184-193`.

## Remaining

- Focused existing Rust packaging test is not runnable in this checkout: frozen `packaging_identity.rs` hardcodes `/home/rashid/projects/opencode-rk/scripts/install-oc2.sh` and cannot address this lane's script.
- Commit/push owned files plus ledger.
