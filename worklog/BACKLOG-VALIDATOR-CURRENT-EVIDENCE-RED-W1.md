# BACKLOG-VALIDATOR-CURRENT-EVIDENCE-RED-W1

## Claim and boundary
- Task: protected validator contract review; own only `tools/validate_backlog_exhaustion.py`, this scratchpad, and own claim row.
- Session: `ses_f1a169bd0ffeP8SptMFPpJCtvq`.
- Candidate base: `eebf2359466f862ec98a54a7b8799734a77ad59a`; branch `red/BACKLOG-VALIDATOR-CURRENT-EVIDENCE-W1`.
- Claim recorded before editing; current tree isolated in named worktree.

## Source evidence
- `PLAN.md:28-32`: current mandatory stories live in FEATURES/Ralph; not silently removed.
- `PLAN.md:113-120`: acceptance belongs to trusted controller/verifier, not implementer.
- `docs/TDD.md:25-41,55-75`: evidence and tests remain honest; submit patch, not acceptance.
- `docs/REPOSITORY_PROTECTION.md:32-33,169-170`: canonical gate retains backlog exhaustion and manifest reconciliation; DISC/backlog evidence is not acceptance.
- `docs/CONVERGENCE.md:26-28,87-97,101-109`: blockers stay open; tests immutable; run convergence gate.
- Baseline `python3 tools/validate_backlog_exhaustion.py` on exact base: exit 1, 107 errors. First errors included stale WEB-004 residual membership, accepted-status projection mismatches, and accepted stories incorrectly required in the 34-row nonaccepted ledger. External DISC-003 manifest reconciliation remains contradictory.

## Contract / approach
- Correct validator-only stale expectations from Ralph/task/receipt evidence. Preserve explicit `258/224/34`, exact category sets, independent task binding, pinned evidence, implementation receipts, missing-spec and unresolved-coverage checks. Do not treat accepted contextual gap stories as ledger coverage.
- Correct SHARE task/worklog bindings per each of the five independently listed task paths; derive each row's path from its own story ID. Keep EXT-005 independently guarded as `explicit-blocker` with task/worklog evidence; adjacent singleton review is not unresolved-row coverage.
- WEB-004 remains accepted contextual evidence, not an exhaustion row. Pin its eight evidence IDs to literal repository/path/blob metadata and verify against the immutable `sources/inventory/opencode.jsonl` plus independent evidence records. Preserve enterprise-remote missing-spec state from pinned commit/tree evidence.
- Base comparison: `eebf2359466f862ec98a54a7b8799734a77ad59a` REL block had no historical git status equality. Removed only the candidate-added `RELEASE_TASK_STATUS_RECEIPT_COMMIT` and `git show` status equality. Retained current independent task/ownership binding, controller status and accepted/nonaccepted ledger projection checks. Task-card Status is descriptive, not controller acceptance; changing REL card prose alone neither grants acceptance nor causes REL checker failure. Canonical acceptance authority remains the trusted completion integration receipt: `tools/completion_integration.py:22-25,295-319` binds task, candidate/integrated revisions, frozen-test hash, successful rerun; `PLAN.md:113-120`, `docs/TDD.md:38-41` reserve acceptance to controller/verifier and exact integrated rerun.
- `.agents/WORKER.md:174-188` and `tools/completion_claims.py:32-37,89-91` define only separate claims-ledger transitions, not task-card status authority. No historical task-card status is frozen.
- WEB-004 pins, exact 258/224/34 accounting, SHARE ownership, source evidence and other implementation/acceptance receipt checks were not changed by this correction.
- WEB-004 pin sufficiency and eight negative probes independently reviewed and verified; no change to those paths or accounting/SHARE logic.
- Validator SHA-256 after REL correction: `ceff1088485c6c626bb4014659388bb0f279db57b7e91a683ddba06a454d771f`. No freeze, commit or push.

## Verification
- `python3 -m py_compile tools/validate_backlog_exhaustion.py`: PASS.
- `python3 tools/validate_backlog_exhaustion.py`: PASS, `stories=258 accepted=224 stale=2 blockers=9 unresolved=17 user_directed=6`.
- `python3 tools/validate_plan.py`: PASS, 258 stories / 47 requirements / 1290 obligations.
- Disposable archive fixture without `.git`: REL checker passed and output remained unchanged after changing only REL-001 task Status prose from `NOT STARTED` to `IMPLEMENTED`; this has no acceptance effect. Forged SHARE-004 ownership, INT-004 implementation receipt, and EXT-005 blocker reclassification each rejected.
- Earlier negative probes remain valid: partition drift, missing enterprise spec, forged/missing WEB-004 path/blob. Standalone REL task-text mutation is reclassified as no acceptance effect, not safeguard failure. Full validator not claimed archive-compatible; unrelated ancestry checks require git.
- `git diff --check`: PASS. Validator SHA-256 `ceff1088485c6c626bb4014659388bb0f279db57b7e91a683ddba06a454d771f`. No frozen tests changed. Unfrozen, uncommitted pending review.
- Previously recorded external gates: repository guard fails on DISC-003 manifest input drift; convergence gate blocked by 60 findings. Not rerun; out of scope.

## Review and landing record
- Independent reviewer `ses_f19bbebbfffequuSF972KtAk9R`: scoped PASS for validator SHA-256 `ceff1088485c6c626bb4014659388bb0f279db57b7e91a683ddba06a454d771f`. This is a source review receipt, not frozen RED evidence or acceptance.
- Final landing gates: `python3 tools/validate_repository.py` FAIL, `DISC-003 reconciliation manifest drifted at inputs`; `python3 tools/convergence_gate.py` BLOCKED, 60 ledger findings (AUD-017/AUD-020 completed notes admit no acceptance; remaining findings are completed off-plan tasks). Exact outputs recorded in lane handoff.
- Claim remains blocked for those external gates. Candidate may be pushed only as protected review candidate, not integrated or accepted; repository-owner approval remains required for protected-path integration.

## Remaining unknowns / blockers
- DISC-003 manifest input drift and 60 convergence findings remain external blockers, paths out of scope. No frozen RED suite exists; ad-hoc disposable probes only. Candidate remains unaccepted.
