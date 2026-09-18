# COORD-004 — Trusted frozen-test evidence and zero-test rejection

Claim: implement `tools/completion_verification.py` as a stdlib-only
frozen-test evidence checker (RED/GREEN/integrated-green kinds, sha256
`file_hash` with 128 MiB cap, `safe_path` traversal/symlink reject,
zero-test/edited-flag/self-verify reject, denial-no-side-effect +
reclaim assertions).

Source evidence (HEAD 5af7884):
- `tools/completion_plan.py:194-201` — `file_hash` sha256 chunked 1 MiB,
  128 MiB cap raises `InvalidPlan`.
- `tools/completion_plan.py:33-46` — `safe_path`: rejects backslash,
  absolute, `.`/`..` segments, escape-from-root, any symlink component.
- `tools/completion_plan.py:204-231` — `proof_errors`: accepted status on
  exact revision, testCount >= max(1, len(required)), testObligations
  superset, 64-hex frozen hash, verifier != implementer, 1..64 artifacts,
  RED/GREEN/integrated-green kinds when obligations required, per-artifact
  `safe_path` + sha256-vs-bytes check.
- `tools/completion_scheduler.py:143-156` — `validate_proof`: passed True,
  task/revision match, frozen hash equality, verifier != worker, int
  test_count >= obligations, obligations subset. Denials raised before
  `integrate` (pre-merge, `:191-192`), so zero/edited/self cases never
  merge.
- `tasks/completion/delivery.json:7` — COORD-004 card: journey/tests demand
  real compiling RED with test/command hashes, fail-closed on changed
  frozen/zero/ignored/missing, worker PASS text + edited flags rejected,
  denial/cancellation asserts absent effects + reclaimed resources,
  verifier outside implementer write authority, bounded redacted evidence
  tied to candidate revision.
- `docs/TDD.md:43-56,91` — RED compiles and fails for missing behavior;
  denial asserts absence of side effects; cancellation asserts reclaimed
  tasks/FDs.
- `docs/SECURITY.md:54-56` — blocked write leaves no file; blocked process
  does not start.

Observed scenario: file did not exist (`git log --all -- tools/...` empty).
Created it; verified compile + existing 38 completion tests still green
(no new test files — owned file only; frozen tests untouched).

Target boundary: owned file ONLY `tools/completion_verification.py`.
No edits to tests, plan, scheduler, contracts, config, or other tools.

Tests:
- Required gate: `timeout 60 python3 -m compileall -q
  tools/completion_verification.py && timeout 60 python3 -m unittest
  discover -s tests/completion -p 'test_*.py'` → 38 tests OK
  (test_plan.py + test_scheduler.py unchanged).
- Disposable self-probe (tempdir, not committed): valid 3-kind proof →
  no errors, record kinds sorted; zero-test, edited status, self-verifier,
  ignored suites, empty obligations, missing kind, frozen change,
  malformed command hash, tampered bytes, `../` traversal, symlink
  artifact, >128 MiB file all rejected; denial left dir listing
  unchanged, FDs reclaimed, absent path asserted. → "all edge cases OK".

Decisions:
- Mirrored `completion_plan.safe_path/file_hash/proof_errors` semantics
  (same caps, same symlink walk, same kind set) so scheduler + plan +
  verification agree; added `expected_frozen_sha256` equality check,
  `require_command_hash` opt-in, RED compiling/missing-behavior shape,
  ignored/skipped rejection, `passes:true`-without-acceptance rejection.
- `validate_proof` raises `InvalidEvidence` writing nothing (denial has no
  side effects by construction) and returns a <=64 KiB allowlisted record
  (no contents/secrets). Helpers `assert_absent`,
  `assert_no_side_effects`, `assert_fds_reclaimed`, `open_fd_count` give
  tests the TDD §3 denial/cancellation assertions.
- Stdlib only (`hashlib/json/os/pathlib/re`); single process, no threads,
  no network, no subprocess, no env/secret access; 1 MiB chunks; max 64
  artifacts.

Remaining unknowns: none for this slice. Wiring into scheduler/integrator
belongs to COORD-005/COORD-008 lanes, not this file.
