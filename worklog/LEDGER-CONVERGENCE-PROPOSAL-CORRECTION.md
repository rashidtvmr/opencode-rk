# LEDGER-CONVERGENCE-PROPOSAL-CORRECTION

## Claim

- Task: `LEDGER-CONVERGENCE-PROPOSAL-CORRECTION`
- Type: research/proposal correction
- Session: `ses_f2df0db24ffeURdCksWFAmHJFm`
- Owned product path: `worklog/LEDGER-CONVERGENCE-PROPOSAL.md`
- Scratchpad: this file
- Controller state: not modified; only own in-progress claim row added

## Source evidence and decisions

| Finding | Evidence | Decision |
|---|---|---|
| Old proposal unsafe | `d93927e`, `worklog/LEDGER-CONVERGENCE-VERIFY-RETRY.md:12-28,60-233` | Supersede; do not reuse 54-row set, 15 bad targets, commands, or 28 residual claim |
| Authority set | `worklog/DISC-003-AUTHORITY-REMAP-PROPOSAL.md:22-81,100-127`; `worklog/DISC-003-INTEGRATED-REMAP-ADDENDUM.md:11-69` | R51 union R27, disjoint, 78 rows; mappings transcribed exactly |
| Dead evidence | `worklog/LEDGER-CONVERGENCE-VERIFY-RETRY.md:69-73,109-118,221-223`; current ledger rows | Re-home three completed notes before removal; proposed durable destination named |
| Legal API | `tools/completion_claims.py:32-38,89-92,144-192,243-247` | No worker delete/retire API; completed has no outgoing transition; no raw mutation command offered |
| Current gate | `tools/convergence_gate.py:126-148`; current run | `total=93`; distinguish base, added rows, and this in-progress row |
| Backlog gate | `worklog/LEDGER-CONVERGENCE-VERIFY-RETRY.md:310-314,348-349` | Keep `validate_repository.py` 51-error backlog finding separate |

## Count reconciliation

Read-only recomputation on current tree:

- `1f4a9e6`: ledger SHA `25bb55306e0f3f4650a2b4430d68a277627c15a3fc1d3c9a833109ab6985fca3`, 83 off-plan + 3 bad-note = 86 lines, 85 IDs.
- `d93927e`: proposal row adds one off-plan finding, 84 + 3 = 87 lines, 86 IDs.
- `b16e71b`: APP-010 frozen/integration rows plus prior verifier add drift, 88 + 4 = 92 lines, 90 IDs.
- `4e5175e` and `056a210`: retry verifier adds one completed off-plan row, 89 + 4 = 93 lines, 91 IDs.
- Current correction row is `in-progress`, therefore does not add a gate finding. If later marked completed, it becomes an additional off-plan finding unless separately dispositioned by controller authority.

Disposable simulation evidence from verifier: R78 plus three demotions leaves
4 lines at `1f4a9e6`, 10 lines at `b16e71b`, and 11 lines at current
`4e5175e`/`056a210` because the retry row was added after `b16e71b`.

## Required residual matrix

The corrected proposal classifies the current-base residual 11 lines:

- `APP-010-FROZEN-INTEGRITY`: investigate; keep its out-of-scope bad-note open.
- `APP-010-REVISION-RECEIPT`, `APP-010-REVISION-RECEIPT-INTEGRATION`: canonicalize only after controller confirms parent ownership and preserves receipt hashes; otherwise keep open.
- Four `APP-012-*` sub-lanes: keep open under in-progress `APP-012`; no feature reversal.
- `LEDGER-CONVERGENCE-PROPOSAL`, `LEDGER-CONVERGENCE-VERIFY`,
  `LEDGER-CONVERGENCE-VERIFY-RETRY`: retire or canonicalize only under explicit controller authority while preserving worklogs/receipts.

No residual is automatically demoted. `AUD-017`, `AUD-020`, and
`INSTALLED-DEFAULT-CONTRACT-INTEGRATION` are proposed demotions only under
explicit controller authority, with notes preserved and dependency impact
recorded.

## Requirement matrix

| Requirement | Proposal evidence |
|---|---|
| 1. Mark 54/28 unsafe | Status section explicitly supersedes `d93927e` |
| 2. Derive/enumerate 78 | R51/R27 derivation, counts, complete mapping blocks |
| 3. Resolve 15 targets | Contradiction table uses authority document only |
| 4. Re-home three dead rows | Destination, exact notes, row hashes, source refs, output hashes specified |
| 5. Separate legal/authority | API table; no delete/demote worker operation |
| 6. Safe transaction | Preconditions, disposable simulation, backup, atomic write, schema/repo/gate/verifier receipts |
| 7. Named bases/count drift | Four immutable bases, SHA-256, 86/87/92/93, current residual 11 |
| 8. Residual classification | Current-base matrix: investigate, canonicalize, keep open, controller-retire |
| 9. Evidence preservation | Explicit no-reversal rule, hashes, backup, durable worklog, frozen receipts |
| 10. Authorization/lane boundary | Controller authorization list, separate implementation/controller and verifier lanes |
| 11. Backlog separate | Dedicated section; `validate_repository.py` remains independent |

## Validation and limits

Commands, all read-only or disposable as required:

- `rtk python3 tools/convergence_gate.py` -> `CONVERGENCE BLOCKED`, `total=93`.
- `rtk python3` count/hash scripts -> verified base counts, current SHA,
  R51=51, R27=27, disjoint union=78, all 78 currently completed off-plan.
- `rtk git log --all -- worklog/LANE-CI-EXT.md worklog/WEB-EVENT-STREAM.md worklog/WEB-HINT.md` -> no commits; dead scratchpads confirmed.
- `rtk git diff --check` -> clean (exit 0) after final edits.
- Author checks: both owned Markdown files contain zero U+2014/U+2013; proposal
  contains 51 R51 mappings plus 27 R27 mappings; own claim row validates as
  `in-progress` with the required scratchpad; no `delete`, `remove`, or `retire`
  API exists and `completed -> blocked` is rejected.
- No Cargo commands run. No canonical reconciliation simulation run. No
  controller state, product, test, plan, verifier, or protection file edited.

Remaining unknowns: controller disposition for proposal/verifier/retry rows;
whether controller chooses a new sanctioned API or a reviewed bespoke
transaction; exact fresh pre-operation SHA at application time. These are
authority questions, not fabricated here.
