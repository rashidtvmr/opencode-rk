# LEDGER-CONVERGENCE-VERIFY

## Claim
- Task: LEDGER-CONVERGENCE-VERIFY
- Session: ses_f2e217617ffenDJZbnchx5Jkjr
- Status: in-progress -> completed
- Branch: plan/ledger-convergence
- Owned files: worklog/LEDGER-CONVERGENCE-VERIFY.md + own ledger row only
- Did NOT edit: proposal, product, tests, PLAN, task manifests, prior claim rows,
  verifier, controller files. Simulation done in-memory/disposable copies only.

## Verdict: FAIL (do not apply the proposal as written)

Independent reproduction confirms the *raw counts* the proposal claims, but the
proposal's 54-row alias set is unsound: it is not the authoritative 78-row retire
set, its fold-target table contradicts the authoritative DISC-003 mapping on 15 of
27 IDs, three of its 54 rows cite worklog evidence that does not exist in git, and
its Stage-1/Stage-2 commands depend on an API path that is not authorized.

## Reproduced facts (base commit 1f4a9e6)

`python3 tools/convergence_gate.py` at 1f4a9e6, recomputed via the same
`ledger_errors()` logic:
- findings = 86 (83 off-plan-completed lines + 3 bad-note lines)
- distinct tids producing findings = 85
- INSTALLED-DEFAULT-CONTRACT-INTEGRATION produces BOTH an off-plan line and a
  bad-note line (the 86-vs-85 delta). CONFIRMED.
- bad-note tids: AUD-017 ("no acceptance"), AUD-020 ("no acceptance"),
  INSTALLED-DEFAULT-CONTRACT-INTEGRATION ("missing"). CONFIRMED.
- plan story ids = 109. CONFIRMED.

At proposal commit d93927e the ledger gains one row (LEDGER-CONVERGENCE-PROPOSAL)
so the gate reports 87 findings / 86 tids. The proposal states this. CONFIRMED.

## Defect 1: the 54-row alias set is wrong; authoritative set is 78

The proposal's `retire` = A1 (27 DISC-003 new) UNION A2 (27 IDs), total 54.
- A1 exactly equals the 27-ID list in
  `worklog/DISC-003-INTEGRATED-REMAP-ADDENDUM.md:18-45`. CONFIRMED.
- A2 is NOT new evidence. All 27 A2 IDs are already members of the 51-ID
  authoritative retire set in `worklog/DISC-003-AUTHORITY-REMAP-PROPOSAL.md`
  (`retire`, assert len(retire) == 51). `A2 - orig51 == []`.
- Therefore proposal retire = A1 UNION (27 of the 51) = 54. The authoritative set
  is orig51 (51) UNION A1 (27) = 78. The proposal duplicates 27 already-retired
  IDs and omits 24 authoritative alias rows:
  ACP-001, LANE-TOOL-PERM, LANE-TRANSCRIPT-LAND, LANE-TUI-HOST, LANE-TURN-SETTLE,
  LANE-WEB-HONEST, OPS-009, PROV-018..PROV-022, REL-003, RUN-001, SDK-001,
  SDK-002, SYNC-001, SYNC-002, TOOL-012, TOOL-018, TOOL-019, WEB-004..WEB-006.
- Shortfall = 78 - 54 = 24.

Consequence: the proposal's own Group B classifies those 24 as "truly orphaned
implementation completions needing canonicalization", but the DISC-003 authority
already assigned every one of them a canonical parent
(ACP-001=AUD-009, OPS-009=AUD-018, PROV-018..022=AUD-002, REL-003=AUD-018,
RUN-001=AUD-010, SDK-001/002=AUD-010, SYNC-001/002=AUD-010, TOOL-012/018/019=AUD-005,
WEB-004/005/006=AUD-014, LANE-TOOL-PERM=DISC-105, LANE-TRANSCRIPT-LAND=AUD-013,
LANE-TUI-HOST=AUD-011, LANE-TURN-SETTLE=APP-004, LANE-WEB-HONEST=AUD-014).
They are aliases, not orphans.

## Defect 2: A2 fold-target table contradicts the authoritative mapping

The proposal's Group A2 "Proposed canonical fold targets" disagree with
`DISC-003-AUTHORITY-REMAP-PROPOSAL.md` on 15 of 27 IDs, including swap/rollback
cases that would misroute evidence:

| alias | proposal target | authoritative target |
|---|---|---|
| BASE-004 | AUD-010 | AUD-001 |
| FIX-LOGROTATE | AUD-001 | AUD-018 |
| FIX-LOOP-RULES | AUD-005 | AUD-017 |
| FIX-PACKAGING | DISC-102 | AUD-018 |
| G6-CHAT-DATADIR | AUD-011 | AUD-001 |
| HEAD-001 | AUD-011 | AUD-001 |
| HEAD-002 | AUD-003 | AUD-001 |
| LANE-AUTODRIVE-CLAMP | COORD-001 | AUD-017 |
| LANE-CHAT-ORIGIN | AUD-015 | APP-011 |
| LANE-CI-CAPS | COORD-005 | AUD-018 |
| LANE-FILE-AUTHZ | DISC-105 | AUD-005 |
| LANE-PROV-FALLBACK | PROV-014 | AUD-002 |
| LANE-RALPH-MAX2 | COORD-002 | AUD-017 |
| LANE-SHELL-AUTHZ | DISC-105 | AUD-005 |
| LANE-SRV-ROUTER | AUD-018 | AUD-001 |

The proposal itself marks these as "not this lane's authority", which is
acceptable as a disclaimer but means the table must not be used for reconciliation.

## Defect 3: three of the 54 rows cite nonexistent worklog evidence

For every retire ID the base row carries a `scratchpad` path. Three point at files
that do not exist in the working tree AND have no commit history on any ref:
- LANE-CI-EXT -> worklog/LANE-CI-EXT.md  (note admits "t03 gated pre-existing
  STREAM regression, owner LANE-STREAM-FIX")
- WEB-EVENT-STREAM -> worklog/WEB-EVENT-STREAM.md
- WEB-HINT -> worklog/WEB-HINT.md

`git log --all -- <path>` returns no commits for all three. The proposal claims
"Git history and worklogs remain evidence" for retired aliases; for these three
rows the only surviving evidence is the `completedNote` string inside the ledger
row being deleted. Removing the row without first capturing that note into an
authored worklog destroys the evidence. This is a real evidence-loss hazard for
exactly the rows whose notes admit a gated regression.

## Defect 4: deletion is not supported by completion_claims APIs

`tools/completion_claims.py` exposes no delete/remove function:
- `hasattr(cc, 'delete') == False`, `hasattr(cc, 'remove') == False`.
- `TRANSITIONS['completed'] == set()` and `validate_transition('completed','blocked')`
  raises `ClaimError: illegal status transition 'completed' -> 'blocked'`.
  `completed -> not-started` likewise fails. `release()` is in-progress only.
- `save_ledger()` validates rows with `validate_row()` (shape) only; it does NOT
  run transition checks. So the proposal's direct `del doc['claims'][tid]` +
  `cc.save_ledger(root, doc)` DOES bypass the state machine and will write.
- The proposal's Stage 2 snippet also sets `status='blocked'` + `blockedNote` and
  pops `completedNote` via raw JSON, bypassing `update()`. Its own note admits
  "completed->blocked requires direct JSON or orchestrator release".

Conclusion: removal and demotion are only possible through an unauthorized
bespoke controller operation (direct dict mutation + `save_ledger`). There is no
sanctioned API. This is not a bug to patch in the verifier; it is a missing
controller authority. Flag unsafe: any rehearsal of `del claims[tid]` against the
canonical `tasks/completion/claims.json` is out of a worker's authority and, for
the three dead-evidence rows, is destructive.

## Defect 5: demotion of AUD-017 / AUD-020 changes plan dependency status

AUD-017 and AUD-020 are plan tasks with live dependents:
- PAR-001 deps [AUD-020, AUD-019]
- COORD-001 deps [AUD-017]
- SHIP-004 deps [PAR-010, AUD-020]
- DISC-119 deps [AUD-017, COORD-001]

Moving them `completed -> blocked` makes their dependents unready. That is a
legitimate consequence of honest status, but the proposal does not state it and
its "expected 28 findings remain" does not mention dependency ripple. It must be
recorded as a controller decision, not a silent side effect.

Note lengths (all <= MAX_NOTE=400, so preservation is feasible):
- AUD-017 note len 211, starts "audit-complete refresh at 1614754 ... no acceptance"
- AUD-020 note len 233, starts "AUD-020 refresh at 1614754 ... verdict no acceptance"
- INSTALLED-DEFAULT-CONTRACT-INTEGRATION note len 355, starts "INTEGRATED 15381e3..."

Both AUD notes literally contain "no acceptance"; demotion is semantically
correct. INSTALLED-DEFAULT-CONTRACT-INTEGRATION's parent
INSTALLED-DEFAULT-CONTRACT is already `blocked` with a matching "missing revision
receipt" note, so the child's completion is a genuine parent-child contradiction.
Demoting the child to blocked is correct.

## Predicted remaining count (simulations)

Two in-memory / disposable re-runs of `ledger_errors()`:

1. Proposal set (54 removals + 3 demotions): remaining findings = 28, all
   off-plan-completed. The 28 = 4 APP-012 sub-lanes + the 24 authoritative
   aliases the proposal wrongly left in place. CONFIRMED the proposal's "28".
   But that 28 is not 28 true orphans; only the 4 APP-012 sub-lanes are orphans.
2. Authoritative 78-row set (78 removals + 3 demotions): remaining findings = 4,
   and all 4 are the APP-012 sub-lane aliases whose parent APP-012 is still
   `in-progress`:
   APP-012-FILE-OPS, APP-012-READ-EXECUTOR, APP-012-READ-INTEGRATION,
   APP-012-SERVER-READ-WIRING.

CORRECTED PREDICTION: a correct controller reconciliation removes the 78-row
authoritative retire set and demotes the 3 bad-note rows, leaving 4 findings, not
28. The proposal's "28" overstates unresolved orphans by 24.

## Evidence-preservation and canonical accepted-task check

- No canonical accepted task/evidence is lost by retiring the 78 aliases: none of
  the 78 IDs is a plan task (`[t for t in retire78 if t in known] == []`), no plan
  story depends on any of them, and all 78 are `completed` alias rows whose
  canonical parents remain in the ledger untouched. The 66 in-plan completed rows
  (APP-001..011, AUD-001..019 except 017/020, DISC-102..117, PAR-001..010,
  TUI-001..010) are never touched.
- Only the 3 dead-scratchpad rows (LANE-CI-EXT, WEB-EVENT-STREAM, WEB-HINT) risk
  evidence loss; their `completedNote` must be re-homed to authored worklogs
  before removal.
- Demoting AUD-017/AUD-020 removes their completed status but preserves the note
  verbatim as `blockedNote`, which is the correct legal-status semantics
  (honest partial; plan semantics "completed -> blocked" is not a legal
  `completion_claims` transition, so it needs controller authority).

## Ledger hash controls

- `DISC-003-INTEGRATED-REMAP-ADDENDUM.md` pins SHA-256 `6c8cf2d8691bbd33...`.
  That hash matches commit `3ba4cf967c66925def3f6b53f496460a7c863db6`, verified:
  `sha256(git show 3ba4cf9:tasks/completion/claims.json) = 6c8cf2d8...`. It does
  NOT match base 1f4a9e6 (25bb5530...) nor the current tree (42fff5c6...). The
  proposal correctly says the hash-locked 53-row proposal "MUST NOT be applied to
  this rebased candidate", but then still relies on the DISC-003 addendum's
  27-ID delta as authoritative while dropping the 51-ID parent set.
- The proposal's Stage-1 header says "verify the ledger SHA" but its snippet does
  not include the SHA assertion present in the authoritative proposal. Missing
  guard.

## Unsafe commands flagged

1. `del doc['claims'][tid]` + `cc.save_ledger(root, doc)` on the canonical ledger:
   bypasses `validate_transition`; no controller authorization in-tree; destroys
   the ledger row that is sole evidence for LANE-CI-EXT/WEB-EVENT-STREAM/WEB-HINT.
   Also would delete LEDGER-CONVERGENCE-PROPOSAL? No: not in the 54/78 set, but it
   is off-plan and the proposal notes a controller must handle it during Stage 1.
2. Direct JSON `status='blocked'` write without `update()`/release: unauthorized
   completed->blocked transition, plus unrecorded dependency ripple on PAR-001,
   COORD-001, SHIP-004, DISC-119.
3. Applying the A2 fold-target table would misroute 15 aliases to the wrong
   canonical parent.
4. Asserting `len(retire) == 54` (proposal) versus the authoritative
   `assert len(retire) == 78` (addendum). The 54-row assert would fail the
   addendum's own algorithm.

## Exact authorization needed

A designated controller/integrator must, in one bespoke authorized operation
(not through worker APIs):
1. Read `worklog/DISC-003-AUTHORITY-REMAP-PROPOSAL.md` + addendum; use the 78-ID
   retire set (51 original UNION 27 addendum) and the *authoritative* fold-target
   table, not the proposal's 54-but-duplicated/24-omitted set nor its A2 table.
2. Author worklogs for LANE-CI-EXT, WEB-EVENT-STREAM, WEB-HINT preserving their
   `completedNote` text before any removal (or refuse to retire those 3 until
   evidence is re-homed).
3. Remove 78 alias rows and demote AUD-017, AUD-020,
   INSTALLED-DEFAULT-CONTRACT-INTEGRATION to blocked with note preserved.
4. Add a SHA-256 assertion on the exact pre-mutation ledger (the 6c8cf2d8 hash is
   for 3ba4cf9, not for the candidate; a fresh candidate hash must be pinned),
   write a candidate file, review diff, then replace.
5. Accept the plan-dependency consequence (AUD-017/AUD-020 become blocked) and
   re-run `tools/convergence_gate.py` (expect 4 findings) and
   `tools/validate_repository.py` (currently FAIL backlog exhaustion, rc 1).
6. Because no `completion_claims` API supports row deletion or
   completed->blocked, the controller must record that this is an intentional,
   reviewed ledger operation, or extend the module with an authorized
   `retire`/`demote` API landing under controller ownership. Neither is a
   worker-owned change.

## Commands run

- `python3 tools/convergence_gate.py` at 1f4a9e6 and in disposable copies.
- Re-implemented `ledger_errors()` in `/usr/bin/python3` scripts and reproduced
  86/85, 87/86, then simulated the 54-row and 78-row reconciliation in memory and
  in a `tempfile.mkdtemp` disposable copy (`lv5.py`, `lv8.py`, `lv9.py`).
- `git show 1f4a9e6:tasks/completion/claims.json | sha256sum` = 25bb5530...
- `git show 3ba4cf9:tasks/completion/claims.json | sha256sum` = 6c8cf2d8...
- `git log --all -- worklog/LANE-CI-EXT.md` etc: no commits.
- `python3 tools/validate_repository.py` -> rc 1, FAIL backlog exhaustion.

## Remaining unknowns

- Whether the controller intends to also retire LEDGER-CONVERGENCE-PROPOSAL and
  LEDGER-CONVERGENCE-VERIFY rows themselves (both off-plan, both self-authored by
  controller-directed lanes). Not resolvable from the ledger.
- The 51- vs 78- row accounting in the addendum says "the original 51 off-plan
  rows remain, and current origin/main contributes these 27" -> 78, yet the
  addendum also says the gate reports 80 findings (78 off-plan + 2 bad-note) at
  3ba4cf9. At base 1f4a9e6 the gate reports 86/83/3. The 5-row difference is
  pre-existing drift on the branch and is the controller's to explain.