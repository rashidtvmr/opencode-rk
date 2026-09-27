# Wave 6 - Verify final artifacts and make the Phase 1 release decision

Status: DRAFT. Entry: `P1-G5` independently passed.

## Demonstration that closes this wave
Relocated signed/approved artifacts pass each declared platform, browser, live-provider, migration/recovery and independent release-decision gate on one exact source revision.

## Work packages
- **P1-W6-01 - Produce reproducible signed release artifacts with native closure**. The downloaded artifact is the tested application and includes everything needed without a developer runtime.
- **P1-W6-02 - Prove installation, upgrade, rollback and safe uninstall**. Users can install and update the product without Cargo/Zig/Node and without risking existing OpenCode history.
- **P1-W6-03 - Pass the installed native application matrix on real platforms**. Each declared native platform independently passes the required installed user journeys.
- **P1-W6-04 - Pass browser release acceptance against the packaged daemon**. The shipped embedded web UI, not the development build, passes the approved full local feature matrix.
- **P1-W6-05 - Run small authorized live-provider and service canaries**. Deterministic fixtures are complemented by real approved service evidence, without leaking secrets or spending an unbounded budget.
- **P1-W6-06 - Obtain independent security and supply-chain release review**. A different authority verifies that the actual release meets the security promises made in the UI and documentation.
- **P1-W6-07 - Prove data migration, backup recovery and release soak**. Release installation and repeated use preserve data and stay inside frozen resource budgets.
- **P1-W6-08 - Write verified user setup, operation and recovery documentation**. A fresh user can follow the documented release workflow without implementation-only commands or false feature promises.
- **P1-W6-09 - Issue an independent Phase 1 completion decision**. Only the approved local Phase 1 scope may be certified, and only on the exact integrated release revision.
- **P1-W6-10 - Reconcile handoff records and request release approval**. Finish with an auditable handoff, not a celebratory percentage or automatic publication.

## Gate criteria
- Source, native library, web bundle and installed artifact hashes match the receipts.
- All scoped parents and required surface tests are independently accepted; no self-certified task completion.
- Later-phase full-release work remains open; a Phase 1 certificate cannot assert whole-project completion.

## Capacity and sequencing
Freeze shared contracts first; then dispatch only non-conflicting one-file candidates. Reserve integration and independent review capacity. Use one heavy validation token across the wave, not one build per worker. A wave may contain multiple bounded dispatch rounds; wave count is not a concurrency or time estimate. Keep repair capacity rather than filling every slot with new breadth.
