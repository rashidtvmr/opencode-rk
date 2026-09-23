# TUI-011 installer security implementation

Status: blocked; implementation green; parent release gaps remain.

Task/session: `TUI-011` / `ses_f32880ed8ffeQpIqYmzt0MQj`

Owned product file: `scripts/install-oc2.sh`.

Frozen hashes: security RED `c1c8fa57017612db43e6a7aaafa4945f1bba077405ee8bc541e0c50abf2a50a4`; native closure `fe5543977bdfb3dd435d240f131b4e2174466225339e536824b21f273a12f1d5`.

Changes: reject destination symlink directories before creation/writes; uninstall binary and platform native sibling without following links; cleanup destination temporary/backup paths on exit; validate staged `--version` and `--help`, preserving exit 74.

Verification commands: `sh -n scripts/install-oc2.sh`; `python3 tests/bootstrap/test_tui011_installer_security.py`; `python3 tests/bootstrap/test_tui011_installer_native_closure.py`; hash checks.

Blocked parent gaps: rpath, cross-architecture validation/production, SBOM/license closure, checksum provenance, signing/notarization.

Review correction: staged help invocation is quoted as `"$src"`; `cleanup()` always removes destination temps, removes unused backup placeholders only when the matching `*_backed=0`, and preserves backed rollback files if restoration fails after `rollback_install` sets `transaction_active=0`.
