# BACKLOG-REOPEN-32-W1

## Claim
Session `ses_f1e3fef19ffeLbTow3iFy0Dkp2`; branch `reconcile/BACKLOG-REOPEN-32-W1`; base `3b716c7f405df7b7621a26f83fdd4aadd0541606`. Owns only `sources/backlog-exhaustion.json` plus this scratchpad and the claim row.

## Source evidence
- `ralph.json` `userStories`: 258 stories, 226 accepted, 23 in-progress, 9 not-started. The canonical nonaccepted set has 32 IDs.
- `tools/validate_backlog_exhaustion.py:173-198` fixes category ID sets; `:334-409` builds live rows and summary; `:2483-2550` requires exact status partition and forbids accepted IDs in the ledger.
- `sources/backlog-exhaustion.json` at base contains 98 rows / 133 accepted, 31 in-progress, 67 not-started; classification is stale versus current `ralph.json`.
- `FEATURES.md:46,53` explicitly preserves AUTO-005 and EXT-008 unresolved-evidence conflicts; original ledger rows also retain blocker categories/evidence refs.
- `tasks/PROV-016.md:3-7` says NOT STARTED, five obligations; `worklog/PROV-016.md:14,23,30-31` says no valid RED and acceptance verifier-owned; `worklog/PROV-016-FREEZE.md:39` says the additive suites require freeze attestation/waiver.
- `tasks/WEB-007.md:3`, `tasks/WEB-008.md:3` say IMPLEMENTED / NOT ACCEPTED, despite current Ralph statuses not-started. Validator `:2566-2570` requires stale-local accounting update for nonaccepted IMPLEMENTED task cards.

## Observed scenario
Initial `validate_backlog_exhaustion.py` and `validate_plan.py` fail: prior ledger lists 67 newly accepted IDs, misses PROV-016, and validator category sets still include accepted IDs. Independent protected manifests produce stale-ownership/task-emergence errors after the 32 Ralph statuses changed.

## Target boundary / decision
Reconcile only the ledger rows to current Ralph statuses; preserve existing evidence, categories, pinned paths/commits and AUTO-005 / EXT-008 conflict notes. Do not modify validators, Ralph, FEATURES or any gap manifests. PROV-016 has no fixed validator category in `CATEGORY_IDS` or `REASON_BY_ID`; do not invent one. Leave the lane blocked if this unresolved policy conflict prevents exact validator GREEN.

## Tests
- `python3 tools/convergence_gate.py` on base: BLOCKED, including off-plan completed claims and AUD-017/AUD-020 notes; unrelated to this file.
- `python3 tools/validate_backlog_exhaustion.py --ledger sources/backlog-exhaustion.json`: FAIL, 54 errors before reconciliation.
- `python3 tools/validate_plan.py`: FAIL, 54 errors before reconciliation.
- `python3 tools/validate_repository.py`: FAIL at backlog exhaustion before reconciliation.

## Remaining unknowns
Requires policy-owner decision/classification for PROV-016 and reconciliation of protected `sources/*ownership-gap.json` manifests. Fixed category policy is immutable in this lane; no acceptance or parent-completion claim.

## Stop decision
Blocked without editing the owned ledger. Current validator policy is internally inconsistent for this tree: `CATEGORY_IDS` (`tools/validate_backlog_exhaustion.py:173-198`) requires historical accepted IDs in category sets, while `validate_ledger` (`:2503-2510`) forbids all accepted IDs and demands only current nonaccepted IDs. In addition, current `PROV-016` is nonaccepted but has no classification or reason in fixed policy (`:228-255`), so `build_expected_ledger` raises `KeyError`. Correcting these requires edits outside this lease. Deleting rows or inventing a category would violate the task constraints.
