# LEDGER-CONVERGENCE-TOOL-RED

## Claim

- Task: `LEDGER-CONVERGENCE-TOOL-RED`
- Type: RED authoring
- Session: `ses_f2d27018cffeqjKbFHUPWzxOjn`
- Branch/worktree: `controller/LEDGER-CONVERGENCE-TOOL`
- Owned file: `tests/bootstrap/test_ledger_convergence_controller.py`
- Scratchpad: this file

## Source evidence

- `worklog/LEDGER-CONVERGENCE-TOOL-TEST-BOUNDARY.md:64-165` defines the sole
  Python API, CLI, marker, output, and apply-disabled contract.
- `worklog/LEDGER-CONVERGENCE-TOOL-TEST-BOUNDARY.md:197-280` defines T01-T21,
  including JCS/hash domains, exact R78, CAS/lock, durable publication,
  recovery, bounds, and guarded rollback.
- `worklog/DISC-003-AUTHORITY-REMAP-PROPOSAL.md:100-122` and
  `worklog/DISC-003-INTEGRATED-REMAP-ADDENDUM.md:13-63` define the fixed 78-ID
  retirement set and three demotions.
- `worklog/LEDGER-CONVERGENCE-PROPOSAL.md:241-255` defines hash preimages and
  `:260-423` defines phases, lock, journal, recovery, and rollback invariants.

## Scenario matrix

| Group | Test coverage |
|---|---|
| T01 | JCS UTF-8/sorted arrays, duplicate/NaN rejection |
| T02 | row/file/ledger fixed SHA-256 vectors |
| T03 | manifest/receipt non-circular hash domains |
| T04 | Phase A manifest fields and excluded dependent fields |
| T05 | exact R78 plus three demotions, byte-preserving candidate |
| T06 | deterministic Phase C-shaped simulation receipt fields |
| T07 | repeat simulation and canonical byte preservation |
| T08 | `save_ledger` refusal and no raw publication helper |
| T09 | apply always exit 2, `apply-disabled`, no token bypass |
| T10 | marker, path, txid, JSON, secret, and output bounds |
| T11 | common-dir lock, mode, contention, malformed/stale owner refusal |
| T12 | initial CAS failures |
| T13 | final CAS failure before publication |
| T14 | remote advancement refusal |
| T15 | txid idempotency and sidecar ownership |
| T16 | write/flush/file-fsync/replace/dir-fsync observability |
| T17 | each durable-write failure leaves canonical bytes unchanged |
| T18 | all documented crash points |
| T19 | state-driven recovery, corruption and changed-branch refusal |
| T20 | journal/sidecar/backup bounds |
| T21 | guarded pre/post rollback refusal on mismatch |

## Boundary and safety

All fixtures use `TemporaryDirectory`, fixed bytes, local disposable Git only,
and no network or real ledger. Every simulation/refusal snapshots canonical
ledger bytes, source worklog/plan/mapping bytes, and the fixture tree. Apply is
never expected to succeed. No test calls `save_ledger()`.

## RED evidence

Initial implementation is intentionally absent at authoring time. The focused
suite must collect successfully, then fail assertions for the missing module or
behavior. Compile/import/collection failure is invalid RED. Exact command and
output are recorded after the final test file is frozen.

## Frozen artifact

- Test SHA-256: `056754e65755411cb12fc8b590e53ca89ecca9f4288cfd41d9acd82bf640f6fe`
- Command manifest: `PYTHONHASHSEED=0 python3 -m unittest tests.bootstrap.test_ledger_convergence_controller -v`
- Canonical before/after hashes: every protected fixture byte snapshot is equal
  before/after refusal and deterministic simulation; no canonical checkout bytes
  changed.
- RED result: `Ran 21 tests in 2.835s`, `FAILED (failures=50)`; failures are
  assertion failures for absent `tools.ledger_convergence_controller`, not
  collection/import failures.

## Unknowns

- Product implementation must choose private lock/sidecar internals while
  preserving the public API and exact observable contract.
- Independent verifier reruns this frozen RED and decides GREEN/acceptance.
