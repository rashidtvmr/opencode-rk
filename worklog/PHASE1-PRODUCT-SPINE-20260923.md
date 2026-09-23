# Phase 1 product-spine integration branch

## Boundary

- Branch: `lane/PHASE1-product-spine-20260923`.
- Base: `06ed486`, the pushed Phase 1 candidate before audit-only waves.
- Purpose: serialize verified product and frozen-RED commits without importing
  audit-only ledger history or the dirty original integration worktree.

## Admission rules

1. Source lane branch is pushed and retained.
2. Exact changed files and claim row are verified from Git, not chat output.
3. New tests have a compiling RED, frozen SHA-256, and bounded command manifest.
4. Product commits do not edit frozen tests.
5. Focused GREEN and regressions rerun on this exact integrated branch.
6. `validate_repository.py` and convergence failures remain explicit blockers;
   no accepted flag, verifier, policy, or safeguard is weakened.

## Pending candidates

- UI-014 renderer-independent composer prewire: under semantic review; not
  accepted, and native artifact/PTY/owned-worker behavior remains blocked.
- WEB-009 durable tool/reference + accessibility RED: test-author lane active.
- PROV-023 runtime catalog RED integrated at `896103a`: frozen test SHA-256
  `f74dba4c600c89f59d011fff0361ef6d5be1925d113ac750b7897889b1d57e95`.
  Its public request-path case compiles and fails because Google is documented
  but `profile_for` returns `UnknownProvider`; malformed/version/cap loader seam
  remains absent, so the task stays blocked after this RED eventually turns green.
  Exact integrated rerun (bounded through Python because macOS has no `timeout`
  binary): 4 tests, T02–T04 pass, sole T01 failure is `UnknownProvider`; exit 101.

No candidate is integrated merely because it compiles or has isolated GREEN
tests. The hard installed `oc2` journey in `docs/CONVERGENCE.md` remains the
parent acceptance boundary.
