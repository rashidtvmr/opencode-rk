# REL-002 strong RED

## Claim

- Task: REL-002
- Session: `ses_f32d2ba72ffeZ7tOBYK7YE5rto`
- Branch: `lane/REL-002-phase1`
- Base: `aafec3aa5ad0bed4a0e02d63600797a7f16f6cad`
- Owned files: `tests/bootstrap/test_rel002_release_tdd.py`, this scratchpad, own ledger row.

## Source evidence

- `tasks/REL-002.md:34-63,69-73`: validator CLI, exact five arguments, four proof checks, deterministic output, malformed/timeout tool errors, T01-T05.
- `tools/check_release_tdd.py:40-55`: required CLI arguments and `--timeout`.
- `tools/check_release_tdd.py:60-87,167-228`: malformed receipt/tool-error, timeout, exact revision, frozen hash, gate, independent verifier checks.
- `.github/workflows/ci.yml:15-28`: protected `planning` job currently has checkout, canonical repository validator, bootstrap suite; no release validator caller.
- `tools/validate_repository.py:56-96`: protected canonical planning block; workflow implementation requires owner/guard review and cannot be changed in this lane.
- `.github/protection-policy.json:20-76`, `.github/CODEOWNERS:4-6`: workflow is protected infrastructure with code-owner authority.
- `docs/REPOSITORY_PROTECTION.md`: read as required protection context.

## Contract tested

`test_ci_calls_release_validator` now parses only the `planning` job and executable `run` steps, ignores comments/dead strings, requires one Python invocation of `tools/check_release_tdd.py`, exact `--revision`, `--manifest`, `--receipts`, `--gates`, `--out` argv values, dynamic `git rev-parse HEAD` revision, positive job timeout, `set -e`/`pipefail`, and no `continue-on-error`/`||` bypass.

Additional tests:

- Dead comment/scalar, missing argument, historical revision, swallowed exit, ignored step all reject.
- Extracted valid step executes in a disposable git repository; captured argv proves revision equals that repo's `git rev-parse HEAD` and exact argument order/values. Replaced validator exits 23; shell step returns 23.
- Existing validator behavior remains covered: full proof, missing/import-only RED, mutated frozen bytes, wrong verifier revision, worker self-report, AUTO-005 duplicate, determinism/read-only.
- Malformed receipt returns exit 1, stderr tool error, no report.
- Tiny `--timeout` returns exit 1, stderr timeout, no report.

## RED evidence

Command:

```text
python3 -m py_compile tests/bootstrap/test_rel002_release_tdd.py
python3 -m unittest tests.bootstrap.test_rel002_release_tdd
```

Result: 12 tests executed; 11 pass. Sole failure is `test_ci_calls_release_validator`, with `AssertionError: planning job must contain exactly one executable release-validator invocation; found 0`. All validator/fixture cases and strong CI negative/execution tests pass against current implementation. Failure is the missing real executable CI step only.

`python3 tools/convergence_gate.py` remains blocked by the pre-existing 53 repository findings, including off-plan completed claims. Not acceptance evidence.

## Boundary and blocker

No workflow, validator, fixture, policy, protection, or frozen unrelated test edited. CI implementation belongs to protected workflow owner and requires canonical guard/code-owner review. REL-002 ledger status set `blocked` pending protected workflow implementation and review. No acceptance claim.

## Candidate hash

Frozen commit: `27516eb3590a697ff79135dbcad8341a97f30b52`.
Frozen test SHA-256: `5c8375613afc11a4e7a11427404c60e663615ea19cb3514ccb316bd6241262ae`.
Controller independently hashes this file and reruns the focused command. Expected: one real missing executable CI step failure; no post-freeze test edits.
