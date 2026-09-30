# Protected ledger reconciliation proposal

Status: review-only; no controller or claim state changed.

## Base and evidence

- Base revision: `81a06296898d62cd9e6a638dfea3dc2ecfb587ee`.
- `python3 tools/convergence_gate.py`: blocked with 58 findings.
- Four active-plan rows are terminally inconsistent:
  - `APP-010` is `completed`, but its note explicitly records identity failure
    as `exit74+rm bad bin`. The frozen SHIP-001 RED proves that this deletes a
    pre-existing installed generation instead of restoring it.
  - `AUD-017` is `completed`, but its note records `test_auto*` 16 run / 6 fail,
    lease validation 18 run / 2 fail, and explicitly says `no acceptance`.
  - `AUD-020` is `completed`, but its note says `requiresRevalidation` for all
    258 legacy stories and `verdict no acceptance`.
  - `TUI-011` is `completed` as a clean-target native distribution, but the
    integrated tree contains only
    `crates/opentui-bridge/native/lib/x86_64-unknown-linux-gnu/libopentui.so`.
    APP-012 cannot compile on `aarch64-apple-darwin` because its required native
    library is absent.
- 56 rows are `completed` in `tasks/completion/claims.json` but have no task ID
  in the 109-story completion plan. `tools/convergence_gate.py` correctly
  rejects all 56; off-plan evidence cannot complete a parent.
- Exact machine-readable input and all 56 rows are preserved outside the
  repository in `PHASE1-LEDGER-RECONCILIATION-PROPOSAL.json`, SHA-256
  `a6225aa88c30f9a6385cbeebd57aa63ff8ea25c49c7e95438a5437f3ea53b770`.

## Proposed owner-reviewed migration

1. Add an immutable historical-evidence ledger with a schema that preserves
   every off-plan row, including task ID, status, notes, session and scratchpad.
2. Add an executable migration validator proving exact set equality and field
   equality between the 56 source rows and the historical ledger.
3. Through a repository-owner controller operation—not a worker edit—reject
   `APP-010`, `AUD-017`, `AUD-020` and `TUI-011` back to `blocked`, retaining
   their existing notes as historical evidence and adding the exact failed
   commands/missing artifacts as blockers. APP-010 remains blocked on the
   proposed `DISC-120` repair.
4. Through the same reviewed migration, remove the 56 migrated off-plan IDs
   from the active claim namespace. Do not delete their worklogs or commits.
5. Require active claim IDs to be a subset of loaded plan IDs. Historical rows
   must never satisfy dependencies or release acceptance.
6. Run, without exemptions or assertion changes:
   - `python3 tools/completion_plan.py --check`
   - `python3 tools/convergence_gate.py`
   - `python3 tools/validate_repository.py`
   - the new migration validator and existing frozen bootstrap tests.

## Non-actions

- Do not weaken `tools/convergence_gate.py`.
- Do not rewrite `completedNote` text to hide failed tests.
- Do not delete off-plan evidence.
- Do not use the coordination ledger as acceptance authority.
- Do not merge this proposal as if it were the migration itself.

## Current external blocker

`tools/validate_repository.py` still reports 51 pre-existing backlog-exhaustion
errors. The reviewed migration must reconcile those protected source ledgers as
one atomic owner-reviewed change; this proposal does not modify them.
