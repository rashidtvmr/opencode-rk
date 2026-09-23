# REL-002 RED review

## Claim

- Task: REL-002
- Branch: `lane/REL-002-phase1`
- Review session: `ses_f331843f8ffed8wEMoFzV9siFz`
- Owned writes: this scratchpad and the REL-002 ledger row only.
- No test, validator, fixture, workflow, policy, or controller edits.

## Candidate identity

- `git rev-parse HEAD`: `2217bcc319c44b965d0620c24f897abcf8fcb230`.
- Supplied `2c98f6728be7eec12002b72f904389689fbad7bed7fdcdb3b30ce1d3e413bd34` matches SHA-256 of `tests/bootstrap/test_rel002_release_tdd.py`.
- Supplied value is not a Git object: `git cat-file -t 2c98...` returned `fatal: Not a valid object name`.
- `tools/check_release_tdd.py` SHA-256: `79be6ef1e7702f3224b378e2864f588a8eed5db1600fbd6917de8bf4071b9c04`.
- Frozen fixture test SHA-256: `c894d98107791ddfbb8ca5a42d4c371a4cfcbcd90ab9dc5db32788cad97f2fa0`.

## Source evidence

- `tasks/REL-002.md:34-63`: CLI contract, four checks, exact revision, deterministic/read-only/bounded behavior, T01-T05 obligations.
- `tools/check_release_tdd.py:182-228`: receipt revision and frozen test hashes are checked against CLI `--revision`; gate receipts must pass; worker reports do not count.
- `tests/bootstrap/test_rel002_release_tdd.py:76-179`: seven validator/fixture behavior tests pass; `:181-186` checks only a workflow source substring.
- `.github/workflows/ci.yml:15-60`: no `tools/check_release_tdd.py` caller.
- `.github/CODEOWNERS:4-6`: catch-all plus `/.github/` ownership by `@rashidtvmr`.
- `.github/protection-policy.json:7-11,20-26`: workflow protected; `planning` required; `tools/validate_repository.py` canonical.
- `tools/validate_repository.py:56-70,89-95`: `planning` block exact protected contract; inserting release call without updating protected validator is invalid.

## Verification

`python3 -m unittest tests.bootstrap.test_rel002_release_tdd`: 8 tests, 7 passed, 1 failed. Sole failure is intentional RED:

```text
test_ci_calls_release_validator ... FAIL
AssertionError: 'tools/check_release_tdd.py' not found in ci.yml
```

Seven exercised cases are real disposable subprocess runs against copied fixtures. Candidate bytes are compared before/after; reports are bounded; output is deterministic in the two-run case. `python3 tools/convergence_gate.py` failed on pre-existing 53 known findings, including off-plan completed ledger rows. Blocker, not acceptance.

## Review decision: BLOCKED

The exact artifact hash is verified, but this RED suite is not admissible for freezing as a release contract yet.

1. `test_ci_calls_release_validator` is only `assertIn` on raw YAML. A comment, dead scalar, or unrelated string passes without a runnable caller, correct arguments, failure propagation, or output handling. It does not prove CI invokes the validator.
2. Fixtures pin historical revision `863a0019fc6f0b9779d867792fe00c4a9ab84c87` (`frozen.json:14`, all receipts), while candidate commit is `2217bcc319c44b965d0620c24f897abcf8fcb230`. Workflow must derive checked-out candidate revision dynamically, preferably `git rev-parse HEAD` after checkout, and pass it as `--revision`. It must not hardcode the fixture revision. Receipt/manifest evidence must be candidate-revision-bound.
3. Card requires malformed-input/tool-error and timeout behavior (`tasks/REL-002.md:49,72`), but frozen reviewer suite has no malformed or timeout test. T01-T05 map to named obligations; additional failure contracts remain unreviewed.
4. T05 checks fixture bytes and temporary directory, but does not observe arbitrary absolute-path writes or assert captured output contains no secret bytes. Resource bounds beyond report size are not exercised.

Workflow is protected infrastructure. Implementation requires `.github/workflows/ci.yml` owner plus CODEOWNER/protection review; this lane must not edit it. Preserve exact `planning` block, or obtain authorized protection-contract update first. Set REL-002 `blocked` pending protected CI implementation and contract/test-author review. No acceptance claim.

## Required implementation contract for next owner

- Add a real executable CI step/job, not a substring/comment, with checked candidate revision and explicit `--manifest`, `--receipts`, `--gates`, `--out` arguments.
- Derive revision from checked tree (`git rev-parse HEAD`, or equivalent exact checkout ref). Do not use `863a0019...` in workflow as release revision.
- Supply receipts and frozen manifest generated or selected for that exact candidate. Do not rewrite static fixture receipts in CI to manufacture a pass. Nonzero validator exit must fail the job.
- Keep `.github/workflows/ci.yml:56-70` canonical `planning` unchanged unless protected validator/policy owner approves coordinated change.
- Add independent assertions for actual invocation and dynamic revision in a newly authorized RED review if existing frozen test remains unchanged; never weaken/edit frozen test in implementation lane.

## Ledger state

Blocked pending protected CI implementation/approval and RED contract repair. No acceptance claimed.
