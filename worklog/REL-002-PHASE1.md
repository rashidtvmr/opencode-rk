# REL-002 phase 1

## Claim

- Task: REL-002
- Branch: `lane/REL-002-phase1`
- Base: `9ded2b901b2376a4a5ee1815a41c7a1234ece157`
- Owned file: `tests/bootstrap/test_rel002_release_tdd.py`
- Orchestrator reclaimed; re-claimed by continuation session `ses_f3335dbc5ffect45yr3f1Ll8N8-rel002`.
  Worktree: `/Users/mymac/Projects/opencode-rk-rel002`.

## Source evidence

- `tasks/REL-002.md:34-63` defines the validator CLI, four checks, deterministic
  output, bounds, duplicate-owner rejection, and REL-002-T01..T05.
- `tools/check_release_tdd.py:89-272` is a real stdlib validator. It hashes the
  frozen files, checks RED/GREEN/verifier receipts, rejects worker-only evidence,
  checks gate outputs, and writes the bounded report.
- `fixtures/release-tdd/{frozen.json,receipts,gates}/` contains complete and
  negative fixtures. The complete fixture pins revision
  `863a0019fc6f0b9779d867792fe00c4a9ab84c87`.
- `.github/workflows/ci.yml:15-60` has planning and Rust jobs but no invocation
  of `tools/check_release_tdd.py`. `grep` found no CI caller.
- `REVIEW_ITERATION_1.md:116-125,165-169` identifies release verifiers as
  unwired and calls for CI integration.

## Contract covered

`test_rel002_release_tdd.py` exercises exact-revision full proof, missing RED,
import-error-only RED, mutated frozen bytes, wrong verifier revision, worker
self-report rejection, AUTO-005 duplicate ownership, deterministic bounded
reports, fixture immutability, and the missing CI caller.

## RED evidence

Command:

```text
python3 -m unittest tests.bootstrap.test_rel002_release_tdd
```

Result (8 tests, 7 ok / 1 FAIL):

```text
test_auto005_material_is_rejected_as_duplicate_owner ... ok
test_ci_calls_release_validator ... FAIL
  (RED: 'tools/check_release_tdd.py' not found in ci.yml)
test_deterministic_and_fixture_read_only ... ok
test_full_exact_revision_proof_passes ... ok
test_missing_or_import_only_red_proof_fails ... ok
test_mutated_frozen_hash_fails ... ok
test_worker_self_report_is_not_independent_evidence ... ok
test_wrong_verifier_revision_fails_closed ... ok
FAILED (failures=1)
```

Semantic RED established ONLY at the missing CI caller
(`test_ci_calls_release_validator`). All 7 behavior tests pass against the
existing validator; the single failure is the missing `.github/workflows/ci.yml`
invocation of `tools/check_release_tdd.py`.

Controller-observed candidate hash (sha256 of
`tests/bootstrap/test_rel002_release_tdd.py`):
`2c98f6728be7eec12002b72f904389689fbad7bed7fdcdb3b30ce1d3e413bd34`.

The worker-reported hash did not match the committed bytes. The controller
independently reran the committed file: 8 tests executed, 7 passed, and only
`test_ci_calls_release_validator` remained RED. Treat this as a candidate RED
pending independent freeze approval, not as an already frozen receipt.

Status: BLOCKED. Out-of-scope blocker (cannot edit `.github/workflows/ci.yml`;
CI-caller lane is a separate one-file owner). No production validator,
controller state, policy, manifest, or other frozen workflow edits.

## Boundary

No production validator, controller state, policy, manifest, frozen unrelated
test, or workflow was edited. Next owner: one-file CI integration owner for
`.github/workflows/ci.yml`, with release-fixture/revision inputs supplied by the
trusted controller. Verifier rerun remains independent.

## Remaining unknowns

- Current static fixture revision is the old source pin; a real CI caller must
  obtain the candidate revision and trusted frozen evidence without mutating
  fixtures.
- No acceptance or release receipt is claimed here.
