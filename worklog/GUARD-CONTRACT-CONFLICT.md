# Repository guard contract conflict — 2026-09-23

## Exact candidate

- Branch: `lane/GUARD-wave1-integrate-20260923`
- Revision before this receipt: `a464c8b`
- `python3 tools/convergence_gate.py`: blocked with the unchanged 80 findings.
- `python3 tools/validate_repository.py`: protection fixtures pass, then backlog
  exhaustion fails with 51 errors.

## Conflict

The remaining repository-guard failures cannot be repaired by changing only the
source-gap or exhaustion ledgers:

- `tools/validate_backlog_exhaustion.py:733-736` requires `ROUTE-009/010` to
  remain `not-started` in `ralph.json`.
- `:1031-1047` requires the OPS gap stories to remain `not-started` and requires
  their task cards/worklogs to be absent.
- `:1251-1264` applies the same requirements to REL-001..003.
- `:1400-1417` requires EXT-001/002 to remain `in-progress` and requires their
  task cards/worklogs to be absent.
- The same frozen-validator pattern produces the remaining SHARE, EXT, and INT
  errors reported by `validate_repository.py`.

The live integrated tree instead contains accepted Ralph rows and real task and
worklog evidence for these stories. The worker contract forbids weakening an
accepted task flag, deleting existing evidence, or modifying frozen verifier
logic/config to obtain GREEN. Updating the gap records to acknowledge the live
files would still fail because the validator directly tests for their absence.

Therefore no honest patch can satisfy all three immutable constraints:

1. preserve accepted task flags;
2. preserve existing task/worklog evidence;
3. preserve the frozen validator's old status/absence assertions.

## Required authority decision

This is a blocked contract review. A controller/human must explicitly authorize
one of the following before the 51-error guard can become green:

1. a new frozen-validator contract that accepts deliberately reconciled live
   task/worklog bindings while retaining null ownership and parent blockers; or
2. an explicit status migration that changes the accepted Ralph rows, contrary
   to the current no-weakening rule.

Deleting task/worklog files, silently changing accepted flags, or editing the
validator merely to suppress failures is not proposed. Until authority resolves
the conflict, canonical source-gap and exhaustion files remain unchanged and
repository acceptance remains false.
