# TUI-011 native bundle security review

Status: blocked; audit only. Owned files: this scratchpad, own ledger row.
Session: `ses_f32a24247ffeJc0c6ZUCBAwUTK`. Candidate: `0cb7b43`.

## Frozen verification

Command: `python3 tests/bootstrap/test_tui011_installer_native_closure.py`
Result: 5/5 passed. SHA-256: `fe5543977bdfb3dd435d240f131b4e2174466225339e536824b21f273a12f1d5`.
`sh -n scripts/install-oc2.sh`: passed.

## Evidence

- `scripts/install-oc2.sh:137-145`: caller-supplied SHA-256 is checked before writes. This authenticates bytes only when caller's checksum is trusted; it does not prove native format, ABI, architecture, signing, or provenance.
- `scripts/install-oc2.sh:158-204`: bounded archive name/type checks reject absolute, traversal, symlink, hardlink, FIFO, and duplicate required members. PAX-format ordinary required members accepted. Long/extra member rejected by four-member exact closure.
- `scripts/install-oc2.sh:230-249`: extraction and payload cap are staged; `:275-303` creates destination temp files before transaction.
- `scripts/install-oc2.sh:65-69`: exit cleanup removes only `stage`; it does not remove `binary_tmp`/`native_tmp` created before transaction.
- `scripts/install-oc2.sh:108-118`: uninstall removes only `$INSTALL_DIR/oc2`; installed sibling `../lib/libopentui.{so,dylib}` remains.
- `scripts/install-oc2.sh:275-276`: `mkdir -p "$INSTALL_DIR" "$native_dir"` follows pre-existing symlink directories; `:296-324` then writes through them.
- `scripts/install-oc2.sh:251-270`: staged binary identity only; no ELF/Mach-O/architecture/native ABI validation.

## Reproductions (disposable `/var/folders/.../T/tui011-review-sq603jao` only)

Generated tar fixtures; each used the exact archive SHA as `--checksum`, isolated HOME/TMPDIR, no real paths.

| fixture | result |
|---|---|
| symlink required member | exit 65, no install |
| hardlink required member | exit 65, no install |
| FIFO required member | exit 65, no install |
| long extra member | exit 65, no install |
| PAX ordinary required members | accepted (exit 0); expected, metadata is not a path/type escape |
| traversal member | exit 65; prior test reused destination, so no conclusion from stale `oc2`; fresh frozen traversal test passes |

Additional controlled checks (not frozen): pre-existing `INSTALL_DIR`/`bin` symlink to an outside disposable directory is followed by `mkdir -p`, allowing the installer to place `oc2` outside the requested root. A symlinked `INSTALL_DIR` parent similarly redirects both binary and sibling native library. Disposable uninstall check: exit 0, binary removed, sibling native library remained. Existing `oc2` and native-library symlink targets are moved as symlink objects (`:306-316`) before replacement; no dereference observed.

## Classification

### (A) Installer sub-contract defects; new frozen RED required

1. Destination symlink directories are followed (`:275-276`); install can write outside caller-selected root. Add frozen RED asserting bin/lib symlink rejection and no outside writes.
2. Uninstall leaves native library residue (`:108-118`). Add frozen RED asserting binary plus sibling native closure removal, while preserving unrelated user files.
3. Failure after `mktemp` setup can leave `.oc2.tmp.*` / `.libopentui.tmp.*` because `cleanup()` only removes `stage` (`:65-69`, `:279-303`). Add deterministic frozen RED using a controlled destination failure after temp creation; assert no temp residue and old files unchanged.

Wrong ELF/Mach-O/architecture bytes were accepted when the shell `oc2 --version` identity passed. Treat native compatibility as B below unless the installer contract is expanded to require format/arch validation; checksum trust does not establish compatibility.

### (B) Parent release gaps

Rpath/loader resolution, cross-architecture artifact production and validation, SBOM/license closure, release checksum provenance, and signing/notarization remain parent release work. Existing evidence: `worklog/TUI-011-PROD-AUDIT.md:97-147`. Wrong native bytes / mismatched architecture belong here absent an explicit installer validation contract.

## Archive mutation

No race mutation claim: checksum is computed before archive reads, so post-checksum archive mutation would be a TOCTOU test requiring a controlled tar/filesystem hook. Not attempted against host paths. Extraction remains staged, but checksum does not bind later reads if the archive can be replaced between checksum and tar operations.

## Decision

Blocked. Frozen five-test suite is green, but three installer defects require new trusted RED tests and implementation. No product/test/policy edits made. No acceptance claimed.
