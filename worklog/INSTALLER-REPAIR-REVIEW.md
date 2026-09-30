# Installer repair review and required lease

Read-only design review against integrated `d382fdf`; no implementation or
frozen-test edits. Muse Spark review independently confirms destructive writes
at `scripts/install-oc2.sh:97-98` followed by deletion on identity failures at
lines 102-139.

## Verified ownership

Current `tasks/completion/local.json:13` APP-010 owns
`scripts/install-opencode2.sh` and `.ps1`, not `scripts/install-oc2.sh`.
Historical scratchpad ownership cannot expand the current approved card.
SHIP-001 owns `release` and `.github/workflows/release.yml`.
DISC-120 on proposal branch `9bae192` explicitly proposes ownership of
`scripts/install-oc2.sh`, but it is not integrated or leased.

Both Windows installers exist: `scripts/install-oc2.ps1` and
`scripts/install-opencode2.ps1`. The worker's contrary statement is rejected.

## Required implementation properties after lease

- Keep checksum validation before any installation changes.
- Extract to a uniquely created staging directory on the installation
  filesystem. Inspect executable format/CPU before running the candidate;
  execution failure alone is not a complete architecture check.
- Bound archive expansion and identity-command output/time. Prevent archive
  traversal and reject symlink/non-regular candidate files.
- Serialize installation and uninstall operations through an owned lock;
  two simultaneous installers must not corrupt rollback material.
- Preserve prior bytes in collision-safe rollback material before replacing
  the live path. Do not use a predictable PID filename with `rm -f`.
- Publish by same-filesystem rename over the live path. Moving the live path
  away before publishing creates an observable missing-executable interval.
- On failed post-publication verification, atomically restore the prior bytes
  even while a failed replacement exists. For a first install, remove only the
  failed newly installed executable.
- Explicit state flags distinguish prepared, published and committed phases.
  Traps preserve original failure status, do not target undefined paths, and
  retain recovery material if restoration fails.
- Define signal exits separately and record crash-recovery behavior. POSIX
  shell traps cannot make multiple filesystem operations power-loss atomic.
- User history and unrelated files are never part of rollback cleanup.

## Verification boundary

Frozen release suite SHA-256 remains
`2decf1d282a8975343183fec934b75ccd4355c83b1c98d576ddd110083345a6e`.
Installer changes can address dynamic T04/T05 failures but cannot make missing
workflow/signing/SBOM assertions pass. Those residuals must remain explicit.
Run shell syntax validation and the unchanged focused frozen suite after a
valid lease. No candidate is GREEN based on this design review.
