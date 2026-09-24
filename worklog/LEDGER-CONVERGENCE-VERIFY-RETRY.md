# LEDGER-CONVERGENCE-VERIFY-RETRY

## Claim
- Task: LEDGER-CONVERGENCE-VERIFY-RETRY
- Session: ses_verifier_retry_LEDGER_CONVERGENCE
- Status: in-progress -> completed (independent verification of proposal d93927e)
- Branch: plan/ledger-convergence
- Owned files: worklog/LEDGER-CONVERGENCE-VERIFY-RETRY.md + own ledger row only
- Did NOT edit: proposal, product, tests, PLAN, task manifests, prior claim rows,
  verifier, controller files. Simulation done in-memory/disposable copies only.

## Verdict: REJECT (proposal as written is unsafe and inaccurate)

The proposal at commit `d93927e` does not safely and accurately reduce convergence
findings. It proposes a 54-row retirement set that is both incomplete (missing 24
authoritative aliases) and redundant (27 IDs duplicated from the original 51), and
its Stage 1/Stage 2 controller commands bypass the `completion_claims` state machine
with no sanctioned API for row deletion or completed->blocked demotion. Three of the
54 rows cite worklogs that do not exist in git history. The proposal's predicted
residual of 28 fails to account for rows added after the proposal's base commit.

However, the underlying DISC-003 authoritative 78-row retire set (lines 1-76 of
DISC-003-INTEGRATED-REMAP-ADDENDUM.md) plus the three demotions (AUD-017, AUD-020,
INSTALLED-DEFAULT-CONTRACT-INTEGRATION) is valid and evidence-preserving for 75 of
78 rows. The three dead-scratchpad rows require pre-homering their completedNote
before removal. A correct controller application of the authoritative set reduces
the current 92 finding lines to 10, not 28 as the proposal predicts.

## Base and current finding counts

### At proposal base 1f4a9e6 (claims.json sha256 25bb5530...)
- `python3 tools/convergence_gate.py` reproduced by prior verifier: 86 finding lines
- 83 off-plan-completed lines + 3 bad-note lines
- INSTALLED-DEFAULT-CONTRACT-INTEGRATION produces BOTH an off-plan line and a bad-note
  line (the 86-vs-85 distinct-tid delta). CONFIRMED via ledger_errors() reimplementation.
- Bad-note tids: AUD-017 ("no acceptance"), AUD-020 ("no acceptance"),
  INSTALLED-DEFAULT-CONTRACT-INTEGRATION ("missing"). CONFIRMED.
- plan story ids = 109. CONFIRMED.

### At proposal commit d93927e (claims.json sha256 differs from 1f4a9e6)
- Ledger gains LEDGER-CONVERGENCE-PROPOSAL row (off-plan completed)
- Gate reports 87 finding lines / 86 distinct tids. CONFIRMED by proposal text.

### At current HEAD b16e71b (the tree this verifier runs against)
- `python3 tools/convergence_gate.py` output: total=92
- 88 off-plan-completed lines (88 distinct off-plan completed tids)
- 4 bad-note lines:
  - APP-010-FROZEN-INTEGRITY ("out of scope") -- off-plan AND bad-note (2 lines)
  - AUD-017 ("no acceptance") -- in-plan bad-note (1 line)
  - AUD-020 ("no acceptance") -- in-plan bad-note (1 line)
  - INSTALLED-DEFAULT-CONTRACT-INTEGRATION ("missing") -- off-plan AND bad-note (2 lines)
- 92 = 88 (off-plan lines) + 4 (bad-note lines). CONFIRMED.
- 4 additional off-plan completed rows were added AFTER d93927e:
  APP-010-FROZEN-INTEGRITY, APP-010-REVISION-RECEIPT,
  APP-010-REVISION-RECEIPT-INTEGRATION, and LEDGER-CONVERGENCE-VERIFY itself.
- These 4 rows are not in the proposal's Groups A, B, or C. The proposal's Stage 3
  prediction of "28 findings remain" assumes only the proposal base state, not the
  current HEAD state.

## Validated/rejected alias mappings

### Group A1 (27 enumerated aliases in DISC-003 addendum, lines 18-45)
The proposal's A1 set exactly equals the 27 IDs named in
DISC-003-INTEGRATED-REMAP-ADDENDUM.md:18-45. The fold-target mappings in the proposal's
A1 (line 47) are verified identical to the addendum's authoritative mapping.
A1 is a subset of the authoritative 78-row retire set.
- A1 count: 27
- All 27 exist as completed off-plan rows. CONFIRMED.
- 24 of 27 scratchpads exist on disk; 3 do NOT:
  LANE-CI-EXT (worklog/LANE-CI-EXT.md) -- absent
  WEB-EVENT-STREAM (worklog/WEB-EVENT-STREAM.md) -- absent
  WEB-HINT (worklog/WEB-HINT.md) -- absent
- `git log --all -- <scratchpad>` returns no commits for all three. CONFIRMED dead.

### Group A2 (27 additional aliases claimed by proposal)
The proposal's A2 (27 IDs, proposal line 50) is a strict subset of the authoritative
51-ID retire set from DISC-003-AUTHORITY-REMAP-PROPOSAL.md (lines 100-110).
- A2 subset-of-authority-51: True. CONFIRMED.
- A2 intersection with A1: 0 (disjoint). CONFIRMED.
- A2 count: 27. CONFIRMED.
- The proposal treats A2 as "additional" but they are already in the original 51.
  The proposal's retire = A1 UNION A2 = 54, but A2 overlaps authority-51 entirely,
  so 27 of the proposal's 54 are redundant duplicates.

### A2 fold-target table (proposal line 53) -- 15 contradictions
The proposal's "Proposed canonical fold targets" table (line 53) contradicts the
authoritative DISC-003 mapping on 15 of 27 IDs:
| alias | proposal target | authoritative target (DISC-003-AUTHORITY) |
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
These mismatches would misroute evidence to the wrong parent. The proposal's own
disclaimer (line 84) acknowledges this is "not this lane's authority", but the table
must not be used for reconciliation. REJECTED for controller use.

### Authoritative 78-row retire set validation
Union of authority-51 (DISC-003-AUTHORITY lines 100-110) and addendum-27 (lines 18-45):
- Union count: 78. CONFIRMED.
- Disjoint: True (no overlap between 51 and 27). CONFIRMED.
- All 78 exist as completed in current claims.json. CONFIRMED.
- None of the 78 are plan stories (plan_ids check). CONFIRMED:
  `len([t for t in retire78 if t in plan_stories]) == 0`.
- Dead scratchpads among 78: LANE-CI-EXT, WEB-EVENT-STREAM, WEB-HINT (3 rows).
  Their completedNote text is the sole surviving evidence and must be re-homed
  before removal.

### Group B orphans (proposal line 55-73) -- reclassified
The proposal classifies 24 IDs as "true orphans needing canonicalization".
The authoritative DISC-003 mapping already assigns every one a canonical parent:
| alias | proposal says | authoritative parent |
|---|---|---|
| ACP-001 | orphan/new-entry | AUD-009 (in authority-51) |
| OPS-009 | orphan/new-entry | AUD-018 (in authority-51) |
| PROV-018..PROV-022 | orphans/new-entries | AUD-002 (all in authority-51) |
| REL-003 | orphan/new-entry | AUD-018 (in authority-51) |
| RUN-001 | orphan/new-entry | AUD-010 (in authority-51) |
| SDK-001, SDK-002 | orphans/new-entries | AUD-010 (both in authority-51) |
| SYNC-001, SYNC-002 | orphans/new-entries | AUD-010 (both in authority-51) |
| TOOL-012, TOOL-018, TOOL-019 | orphans/new-entries | AUD-005 (all in authority-51) |
| WEB-004, WEB-005, WEB-006 | orphans/new-entries | AUD-014 (all in authority-51) |
| APP-012-FILE-OPS | fold into APP-012 | NOT in retire78; parent APP-012 is in-progress |
| APP-012-READ-EXECUTOR | fold into APP-012 | NOT in retire78; parent APP-012 is in-progress |
| APP-012-READ-INTEGRATION | fold into APP-012 | NOT in retire78; parent APP-012 is in-progress |
| APP-012-SERVER-READ-WIRING | fold into APP-012 | NOT in retire78; parent APP-012 is in-progress |
The 24 "orphans" the proposal names are 20 aliases (already in authority-51) + 4
APP-012 sub-lanes (whose parent APP-012 is in-progress, not complete). The proposal
correctly identifies the APP-012 sub-lanes as fold-able but incorrectly classifies the
other 20 as orphans. All 20 have worklogs present on disk.

## Disposable simulation results

Simulated on a `tempfile.mkdtemp()` copy of the entire repo (never on the real ledger):

### Simulation 1: Proposal set (54 removals + 3 demotions)
- Removed the proposal's 54 IDs (A1 union A2) from claims on the disposable copy.
- Demoted AUD-017, AUD-020, INSTALLED-DEFAULT-CONTRACT-INTEGRATION to blocked
  with completedNote -> blockedNote.
- Ran `python3 tools/convergence_gate.py` on the disposable copy.
- Result: total=28 finding lines (all off-plan completed).
- The proposal's "28" is reproduced. But this 28 is wrong as a residual: it includes
  24 authoritative aliases the proposal wrongly left in place, plus 4 APP-012
  sub-lanes. Only the 4 APP-012 sub-lanes are true orphans.

### Simulation 2: Authoritative 78-row set + 3 demotions (on current HEAD)
- Removed all 78 IDs from the authoritative retire set on the disposable copy.
- Demoted the same 3 rows.
- Ran `python3 tools/convergence_gate.py` on the disposable copy.
- Result: total=10 finding lines.
  Remaining 10 = 9 off-plan-completed + 1 bad-note (APP-010-FROZEN-INTEGRITY is both,
  producing 2 of the 10 lines):
  1. APP-010-FROZEN-INTEGRITY (off-plan + bad-note "out of scope") -- 2 lines
  2. APP-010-REVISION-RECEIPT (off-plan)
  3. APP-010-REVISION-RECEIPT-INTEGRATION (off-plan)
  4. APP-012-FILE-OPS (off-plan)
  5. APP-012-READ-EXECUTOR (off-plan)
  6. APP-012-READ-INTEGRATION (off-plan)
  7. APP-012-SERVER-READ-WIRING (off-plan)
  8. LEDGER-CONVERGENCE-PROPOSAL (off-plan)
  9. LEDGER-CONVERGENCE-VERIFY (off-plan)
- These 9 tids are the true orphans after authoritative reconciliation:
  4 APP-012 sub-lanes (parent in-progress) + 5 verifiers/proposal rows (self-authored,
  off-plan) + 3 APP-010 integration receipt rows (off-plan, added post-proposal).
- CONFIRMED: the correct residual is 10 finding lines, not 28.

### Simulation 3: Authoritative set at base 1f4a9e6 (prior verifier's claim)
- At 1f4a9e6, the 4 post-proposal rows (APP-010-FROZEN-INTEGRITY,
  APP-010-REVISION-RECEIPT, APP-010-REVISION-RECEPT-INTEGRATION,
  LEDGER-CONVERGENCE-VERIFY) do not exist.
- Remaining after authoritative 78-removal + 3-demotion at 1f4a9e6:
  4 off-plan completed (APP-012-FILE-OPS, APP-012-READ-EXECUTOR,
  APP-012-READ-INTEGRATION, APP-012-SERVER-READ-WIRING) + 0 bad-note.
  = 4 finding lines.
- The prior verifier's prediction of 4 was correct ONLY for base 1f4a9e6. The
  current HEAD has 6 additional finding lines from post-proposal rows.

## Exact controller authorization and safe mechanism required

The `completion_claims.py` module (tools/completion_claims.py:1-263) provides:
- No `delete`, `remove`, or `retire` function. `hasattr(cc, 'delete') == False`,
  `hasattr(cc, 'remove') == False`. CONFIRMED at line 243-248 __all__.
- `TRANSITIONS['completed'] == set()` (line 37). `validate_transition('completed',
  'blocked')` raises `ClaimError: illegal status transition 'completed' -> 'blocked'`
  (line 89-91). `validate_transition('completed', 'not-started')` also fails.
- `release()` is in-progress only (line 171-184: requires status == "in-progress").
- `save_ledger()` validates row shape via `validate_row()` (line 187-192) but does NOT
  run transition checks. The proposal's Stage 1 snippet uses `del doc['claims'][tid]` +
  `cc.save_ledger(root, doc)` which bypasses `validate_transition()` entirely. The
  proposal's Stage 2 snippet similarly sets `status='blocked'` + `blockedNote` and
  pops `completedNote` via raw JSON, bypassing `update()`. Both are out-of-authority
  for a worker using the `completion_claims` API surface.
- The `claim()` function (line 94-115) fenced against in-progress/blocked, not
  completed. There is no sanctioned worker API to delete a completed row or to
  demote completed->blocked. `TRANSITIONS['completed'] == set()` (line 37) is
  intentionally empty: a completed claim may not be modified by any transition.
- Only the orchestrator's `reclaim()` (line 118-141) can clear a foreign claim, and
  only from in-progress/blocked to not-started. It cannot touch completed rows.
- Conclusion: row deletion and completed->blocked demotion require a controller-level
  bespoke operation (direct dict mutation + save_ledger) or an extension to
  `completion_claims.py` adding a sanctioned `retire()`/`demote()` API under
  controller ownership. Neither is a worker-owned action.

### Required safe mechanism
A designated controller/integrator must perform one authorized ledger operation:
1. Read DISC-003-AUTHORITY-REMAP-PROPOSAL.md +
   DISC-003-INTEGRATED-REMAP-ADDENDUM.md; use the 78-ID retire set (authority-51 union
   addendum-27) and the AUTHORITATIVE fold-target table, not the proposal's 54-row set
   nor its 15-contradicting A2 table.
2. Author or preserve worklogs for LANE-CI-EXT, WEB-EVENT-STREAM, WEB-HINT before
   removing those rows (their completedNote is the sole evidence). These three rows
   must NOT be deleted until their notes are captured in authored worklog files.
3. Assert the pre-mutation ledger SHA-256 (currently 42fff5c6... at this HEAD, NOT the
   6c8cf2d8 hash pinned for 3ba4cf9 which no longer matches). Write a candidate file
   first, review the exact diff, then replace.
4. Remove the 78 alias rows and demote AUD-017, AUD-020,
   INSTALLED-DEFAULT-CONTRACT-INTEGRATION to blocked with note preserved as
   blockedNote.
5. Accept the plan-dependency consequence (AUD-017, AUD-020 become blocked) affecting
   PAR-001, COORD-001, SHIP-004, DISC-119.
6. Re-run `python3 tools/convergence_gate.py` (expect total=10) and
   `python3 tools/validate_repository.py` (expect rc 1, backlog exhaustion pre-existing).

## Prior verifier-row integrity finding

### Existence and provenance
- Prior verifier task row `LEDGER-CONVERGENCE-VERIFY` EXISTS in the current
  claims.json (lines 870-874 of the file, commit b16e71b). CONFIRMED.
- Prior verifier worklog `worklog/LEDGER-CONVERGENCE-VERIFY.md` EXISTS on disk
  (255 lines, authored by session ses_f2e217617ffenDJZbnchx5Jkjr). CONFIRMED.
- The worklog was committed at b16e71b (HEAD). It was NOT present at d93927e
  (`git show d93927e:worklog/LEDGER-CONVERGENCE-VERIFY.md` -> fatal: path does not
  exist in d93927e). The prior verifier's row and worklog were committed by a
  SUBSEQUENT commit (b16e71b), not by the session that errored.
- The task description states the prior session ended with provider HTTP 400
  ("The last message must have role=user"). The committed row and worklog were
  authored after that session failure, likely by an orchestrator or recovery agent.
  The row is therefore a RE-SCRIBED artifact, not a direct output of ses_f2e217617ffe...

### Content validity
- The prior verifier's raw-count reproduction (86/85/83/3 at base 1f4a9e6) is
  CONFIRMED correct.
- Its identification of the 54-vs-78 discrepancy is CORRECT.
- Its identification of 15 A2 fold-target contradictions is CORRECT.
- Its identification of 3 dead scratchpads is CORRECT.
- Its SHA hash verifications (25bb5530 for 1f4a9e6, 6c8cf2d8 for 3ba4cf9) are
  CORRECT (reproduced via `git show <ref>:tasks/completion/claims.json | sha256sum`).
- Its prediction of 4 residual findings at base 1f4a9e6 is CORRECT (confirmed by
  Simulation 3 above).
- Its claim "true residual=4 APP-012 sub-lanes, not 28" applies to base 1f4a9e6 only;
  at current HEAD the residual is 10 due to 6 post-proposal rows.

### Classification: STALE (valid content, wrong timestamp)
The prior verifier-row is not a false completion. Its content is independently
substantiated. It is STALE in the sense that:
- It reports on base 1f4a9e6, not the current HEAD b16e71b.
- It predicts 4 residual findings, but the current HEAD produces 10 after
  authoritative reconciliation (6 additional off-plan completed rows were added
  post-proposal).
- The prior verifier's row itself (LEDGER-CONVERGENCE-VERIFY) is off-plan and
  produces a finding at current HEAD.

The worklog's note "See worklog/LEDGER-CONVERGENCE-VERIFY.md" points to a real file
that exists. No canonical evidence is erased by this row. The row's verdict (FAIL on
proposal as written) is SUPPORTED by this independent verification.

### Ledger row integrity
- The prior row's status is `completed` with session
  `ses_f2e217617ffenDJZbnchx5Jkjr`. The task description says that session ended with
  HTTP 400. The row was committed at b16e71b by a possibly different agent identity.
  This does not invalidate the row's findings but introduces a custody ambiguity: the
  `completedNote` text was authored by whoever committed b16e71b, not necessarily by
  the session ID recorded in the claim.
- The prior row's claim row has no scratchpad path inconsistency: it correctly points
  to worklog/LEDGER-CONVERGENCE-VERIFY.md which exists.
- `validate_row()` passes for this row (valid status, session, scratchpad). CONFIRMED
  via cc.load_ledger on the real file.

## Immutable evidence preservation check

- No canonical accepted task would be erased:
  `len([t for t in retire78 if t in plan_stories]) == 0`. CONFIRMED.
- No plan story depends on any retire78 ID (plan_stories are independent of claims).
  CONFIRMED: `drift_errors` in completion_claims.py only checks plan membership, not
  claim-to-claim deps.
- Frozen test SHAs and RED/GREEN shas are in completedNote fields of the rows being
  retired; these are preserved in git history (commit b16e71b retains the full
  claims.json with all notes). The DISC-003 hash-locked ledger SHA (6c8cf2d8) is for
  commit 3ba4cf9, verified but stale for current HEAD.
- The 66 in-plan completed rows are never touched: APP-001..APP-011, AUD-001..019
  (except 017/020 which are demoted not deleted), DISC-102..117, PAR-001..010,
  TUI-001..010, COORD-001..008, MOB-001..006, NET-001..015, SHIP-001..008,
  PROV-014, PROV-023 (blocked), PROV-024 (blocked), REL-002 (blocked), etc.
  CONFIRMED: none of these are in retire78 or the 3 demotion targets.
- The 3 dead-scratchpad rows (LANE-CI-EXT, WEB-EVENT-STREAM, WEB-HINT) risk evidence
  loss ONLY if their rows are deleted without re-homing their completedNote text.
  All other 75 of 78 rows have existing worklog files or git history.

## Validate_repository.py check

- At current HEAD: `python3 tools/validate_repository.py` reports 51 backlog-exhaustion
  errors (stories in plan not in exhaustion ledger, or vice versa). This is orthogonal
  to ledger cleanup per DISC-003-INTEGRATED-REMAP-ADDENDUM.md:67-69. CONFIRMED.

## Commands run (reproducible)

1. `python3 tools/convergence_gate.py` at HEAD b16e71b -> total=92
2. `python3 -c "..."` reimplementing ledger_errors() -> 88 off-plan + 4 bad-note = 92
3. `git show d93927e:tasks/completion/claims.json` -> 150 completed rows (proposal)
4. `git show 1f4a9e6:tasks/completion/claims.json` -> 149 completed rows (base)
5. `git show 1f4a9e6:tasks/completion/claims.json | sha256sum` -> 25bb5530...
6. `git show 3ba4cf9:tasks/completion/claims.json | sha256sum` -> 6c8cf2d8...
7. Disposable copy simulation (tempfile.mkdtemp): 54-removal+3-demotion -> total=28;
   78-removal+3-demotion -> total=10
8. `git log --all -- worklog/LANE-CI-EXT.md worklog/WEB-EVENT-STREAM.md worklog/WEB-HINT.md`
   -> no commits (dead scratchpads)
9. `git diff --check` at HEAD -> clean (exit 0)
10. `cc.load_ledger(root)` on real claims.json -> passes validate_row for all rows

## Notes / remaining limitations

- This verifier is a verification lane only. It did NOT modify the proposal, the
  canonical ledger, product code, tests, or verifier scripts. Only
  `worklog/LEDGER-CONVERGENCE-VERIFY-RETRY.md` and the
  `LEDGER-CONVERGENCE-VERIFY-RETRY` claim row are owned by this lane.
- The verdict is REJECT because the proposal as written is unsafe: 24 missing
  authoritative aliases, 27 redundant duplicates, 15 misrouted fold targets, 3
  dead-scratchpad evidence-loss hazards, no sanctioned delete/demote API, and an
  outdated residual prediction. The DISC-003 authoritative 78-row set is valid and
  evidence-preserving for 75/78 rows; the 3 dead-scratchpad rows need pre-homering.
- The prior verifier row (LEDGER-CONVERGENCE-VERIFY) is classified STALE: its content
  is substantiated but reports on base 1f4a9e6, not current HEAD. Its own row is
  off-plan and produces a finding at current HEAD.
- This verifier's own row (LEDGER-CONVERGENCE-VERIFY-RETRY) will be off-plan after
  commit, adding 1 finding. A controller handling Stage 1 would retire or canonicalize
  it during reconciliation, same as the prior verifier's row.
- `validate_repository.py` backlog exhaustion (51 errors) is pre-existing and
  independent; not resolved by ledger cleanup.