# V2 controller backpressure test package

- Scope: independent tests only; scheduler implementation remains owned by main.
- Base SHA: `fc2d201f450b6020b1dce12268a550053d65235d`.
- Source: `tests/bootstrap/test_convergence_v2_backpressure.py`.
- Contract: V2 candidate high-water is a hard ceiling of four, including the
  number of adapter executions admitted while four candidates are retained for
  preverification. Cancellation must join workers waiting for candidate
  capacity and workers executing adapter work.
- The fixtures use events and bounded `wait_for` calls for synchronization and
  do not use timing sleeps as assertions.
- V1 scheduler tests that intentionally pass values above four are historical
  semantics and are not modified by this package.

## RED evidence

Command:

```text
rtk /usr/bin/arch -arm64 /usr/bin/python3 -m unittest tests.bootstrap.test_convergence_v2_backpressure
```

Expected current result before the main scheduler repair: the hard-cap test
fails because `run_rolling` accepts `max_unverified=5` and `20`; the bounded
admission test fails because `execute()` runs before candidate-slot admission
and starts more than four workers while four candidates are retained. The
cancellation test records that controller cancellation is joined.

Frozen test source SHA-256 after the compiling RED run:
`73f693fb0695e9261c0fa5a6ae14701d552595985a9c65f85b673eca1de7dd17`.

## Integrated implementation and verification

The scheduler now rejects a candidate cap above four, reserves capacity before
calling the implementation adapter, and counts all running/queued/verifying/
integration-ready/integrating packages when admitting another implementation.
The candidate reservation lasts until rejection or post-integration verification.

Main independently reproduced all three RED failures before changing the scheduler.
After the repair, the frozen suite and the existing eight V2 controller tests pass:

```text
rtk /usr/bin/arch -arm64 /usr/bin/python3 -m unittest tests.bootstrap.test_convergence_v2_backpressure tests.bootstrap.test_convergence_v2_controller
Ran 11 tests; OK
rtk /usr/bin/arch -arm64 /usr/bin/python3 tools/validate_repository.py
validate_repository: OK
rtk /usr/bin/arch -arm64 /usr/bin/git diff --check
exit 0
```

Frozen source hashes remained unchanged. This is controller verification only;
it does not constitute acceptance of G0-G8 product gates.
