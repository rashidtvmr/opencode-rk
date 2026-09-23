# Phase 1 integration candidate — 2026-09-23

## Candidate

- Branch: `lane/PHASE1-integration-20260923`
- Base: `origin/main` at `8a91a7b`
- WEB spine merge: `2f3b994` from `origin/lane/WEB-009-test-replacement`
- Current evidence revision before this receipt: `adb3eae`
- `origin/main` is an ancestor; candidate divergence is `0 behind, 91 ahead`.
- This is a pushed candidate branch, not `main`, release acceptance, or a release
  certificate.

## Integrated lane evidence

- WEB-009 replacement and bounded verifier evidence through `8ecf295`.
- SESS-008 contract/test corrections and no-caller decision through `3856936`,
  replayed as `50598f7..b679dec`.
- PROV-023 catalog RED/review through `3a271c4`, replayed as
  `3d861c2..6213e68`.
- PROV-024 fixture RED evidence `1dcc0c8`, replayed as `4850ad3`.
- REL-002 strong RED through `aad73f0`, replayed as `2b28bd7..508b186`.
- DISC-003 53-row audit/proposal through `00e2c26`, replayed as
  `16d76cc..3ba4cf9`.
- Integrated ledger delta review at `adb3eae`: current main adds 27 aliases, so
  the live candidate has 80 convergence findings rather than the old 53.
- TUI-011 audit, native-closure RED/implementation, security RED, and reviewed
  repair are integrated through `dbf65a0`. The script conflict was resolved by
  preserving the bundle installer and applying current-main's `--help` identity
  contract while staged, alongside the staged `--version` gate.

## Lightweight exact-candidate checks

- `python3 -m json.tool tasks/completion/claims.json`: pass.
- `git diff --check`: pass.
- PROV-023 + PROV-024 Python suites: 13 passed, 0 failed.
- TUI-011 integrated shell verification: `sh -n` passes; security 3/3 and native
  closure 5/5 pass (8/8 total); integrated script and frozen tests are byte-equal
  to `origin/lane/TUI-011-prod`; frozen hashes remain `c1c8fa57...` and
  `fe554397...`.
- REL-002 suite: 12 total, exactly 1 intended RED
  (`test_ci_calls_release_validator`, no executable planning invocation).
- `python3 tools/convergence_gate.py`: blocked, 80 findings (78 off-plan aliases
  plus AUD-017/AUD-020 bad-note rows).
- Disposable authority simulation at pre-addendum `3ba4cf9`: retiring exactly
  those 78 aliases and demoting only AUD-017/AUD-020 makes convergence structure
  GREEN. Canonical ledger was not changed.
- `python3 tools/validate_repository.py`: protection fixtures/checks pass, then
  backlog exhaustion fails with 51 errors. No safeguard was disabled.

## Resource and heavy-test boundary

Latest macOS `vm_stat` showed 111,769 free 16-KiB pages (~1.71 GiB), below the
repository-required 2 GiB free-memory reserve. No Cargo command was started.
WEB-009's third exact-revision pass and focused Rust regressions remain deferred
until memory recovers. Earlier verifier evidence contains two passing serial runs
but is not complete.

## Remaining blockers

1. TUI-011 parent remains blocked on relocatable loader/rpath proof,
   cross-architecture native artifacts, SBOM/license/checksum provenance, and
   signing/notarization despite the installer sub-contract being GREEN.
2. Controller/integrator authority for 78 alias retirements, AUD-017/AUD-020
   demotions, and the separate 51-error backlog-exhaustion reconciliation.
3. Protected REL-002 workflow wiring after guard success and code-owner review.
4. WEB-009 exact integrated Rust verification when memory is safe, followed by
   durable reference persistence and accessibility work.
5. SESS-008 retirement/decomposition decision; do not wire its stale in-memory
   cache into SQLite-backed `SessionService`.
6. PROV-023 real bounded catalog consumer and public provider-path RED.
7. Signing/notarization, cross-platform native artifacts, devices, credentials,
   and hosting-platform rules remain external blockers.
