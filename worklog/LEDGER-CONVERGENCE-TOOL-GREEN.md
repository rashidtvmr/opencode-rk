# LEDGER-CONVERGENCE-TOOL-GREEN

## Claim

- Task: `LEDGER-CONVERGENCE-TOOL-GREEN`
- Session: `ses_f2d07e154ffeuPIAKvIcFwW37T`
- Branch/worktree: `controller/LEDGER-CONVERGENCE-TOOL`
- Owned file: `tools/ledger_convergence_controller.py`

## Source evidence

- Frozen contract: `worklog/LEDGER-CONVERGENCE-TOOL-TEST-BOUNDARY.md`.
- Frozen tests: `tests/bootstrap/test_ledger_convergence_controller.py`, SHA-256
  `056754e65755411cb12fc8b590e53ca89ecca9f4288cfd41d9acd82bf640f6fe`.
- Apply is permanently disabled. Simulation must not call `save_ledger` or mutate
  protected fixture files.

## Design

- Standard-library-only module. JCS-like UTF-8 canonical JSON for hash domains;
  legacy pretty JSON plus LF for ledger bytes.
- Pure candidate removes frozen exact IDs and performs three note-preserving
  demotions. Simulation validates disposable marker, root-contained paths, Git
  cleanliness/branch, source schema/membership/mapping, bounded lock and durable
  simulation evidence. Canonical ledger is never replaced.
- Crash/fault paths write only transaction state and return blocked JSON. Apply
  exits 2 before any root inspection.

## Verification

- Frozen suite GREEN: `PYTHONHASHSEED=0 python3 -m unittest
  tests.bootstrap.test_ledger_convergence_controller -v`, 21/21.
- Frozen test SHA-256 unchanged:
  `056754e65755411cb12fc8b590e53ca89ecca9f4288cfd41d9acd82bf640f6fe`.
- `python3 -m py_compile tools/ledger_convergence_controller.py` passed.
- `git diff --check` passed.
- No tests edited. Apply remains unconditional exit 2. Canonical fixture bytes
  remain unchanged in all frozen scenarios. Simulation sidecars are bounded.
- Resource: one bounded Python unittest process, subprocess Git calls timeout 10s;
  no network, no daemon, no real ledger mutation.

## Remaining unknowns

- Independent verifier must inspect implementation and rerun bootstrap and
  repository/lane gates. No real apply or acceptance claim made.
- No test edits.
