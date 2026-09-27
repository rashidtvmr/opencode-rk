# FEATURES-REOPEN-32-W1

- Claim: `FEATURES-REOPEN-32-W1`; session `ses_f1e48c1dbffeML6UMUl9lvgldy`; ledger status `in-progress`. Assigned route checked: `9router/harbour-gpt-6-luna`, allowed by restored pool.
- Candidate: branch `reconcile/FEATURES-REOPEN-32-W1`, base `3eefe5e808ff4c37391f422e1db79a0c1faef954`, the serialized 32-status Ralph reconciliation.
- Boundary: own `FEATURES.md`, this append-only scratchpad, own completion claim. No Ralph/backlog/tools/tests/policy edits.
- Evidence: `PLAN.md:28-32` identifies `FEATURES.md` and `ralph.json` as implementation backlog and says all current stories are mandatory. At base `FEATURES.md:7,9-20` reports 258 stories and claims post-sync 258/258 accepted. `FEATURES.md:41-74` and category mirrors (`:520-621`) still show stale statuses. Current `ralph.json` has 258 stories: 226 accepted, 23 in-progress, 9 not-started; reopened 32 are only status changes in preceding serialized lane. `tools/validate_backlog_exhaustion.py:412-425` checks status occurrences; `:2462-2480` is canonical `--sync-features` renderer.
- Observable contract: every Ralph story appears in FEATURES and every displayed status matches Ralph; exactly the 32 reopened IDs lose stale accepted labels, the other 226 remain accepted. Historical evidence/caveats remain visible; no new acceptance. In particular retain AUTO-005 and EXT-008 evidence conflicts.
- Failure states: missing/duplicate or mismatched row, stale global completion claim, or altered unrelated story/evidence/caveat blocks completion. Lifetime/resource bounds: static Markdown sync only; no runtime state, queues, secrets, DB, or processes.
- Security: no secret/DB/platform access; static read-only verification plus one Markdown edit.
- Test plan: current `FEATURES.md` mismatch is compiling/executable RED fixture for canonical `_features_status_errors`/repository validator. Run canonical `python3 tools/validate_backlog_exhaustion.py --sync-features`, assert one row per Ralph ID and exact status parity for all stories; diff-check; then `validate_plan.py` and `validate_repository.py`. No product/frozen test applies to declarative status mirror.
- Convergence: `python3 tools/convergence_gate.py` run before work; blocked on pre-existing ledger conditions: AUD-017/AUD-020 no acceptance and off-plan completed task IDs. No controller/ledger fixes authorized.
- Decision: use canonical status renderer, replace obsolete global acceptance assertion with current status counts, reword historical sync table heading/evidence to prevent interpreting prior candidate evidence as current acceptance. Keep all other rows and unresolved caveats byte-identical where possible.
- Remaining: run status parity, validator gates, commit/push candidate branch only; no main merge.

## 2026-09-27 verification update

- Canonical renderer: `python3 tools/validate_backlog_exhaustion.py --sync-features` applied generated status mirrors. Its validation then reports 54 cross-file backlog/evidence errors, including missing PROV-016 classification, accepted/nonaccepted ledger mismatch, stale ownership/gap accounting. These are outside the FEATURES lease; no such files changed.
- Corrected the renderer's unhandled numbered historical proposal table using the same Ralph source statuses. Kept the 82 evidence rows and caveats, made the section explicitly historical/not acceptance, removed false `258/258 accepted` global statement. Added exact unresolved conflict notes for AUTO-005 and EXT-008 from their worklogs/DISC-003 reconciliation.
- Final status assertion: all 258 Ralph IDs have recognized FEATURES status occurrences and every occurrence matches Ralph; the 32 intended changed IDs are the only status differences vs base. Counts: accepted 226, in-progress 23, not-started 9. `git diff --check` clean.
- `python3 tools/convergence_gate.py`: blocked by pre-existing AUD-017/AUD-020 completion notes admitting no acceptance and off-plan completed ledger tasks. Not addressed under lease.
- `python3 tools/validate_plan.py`: expected exit 1, 54 backlog/evidence reconciliation errors (listed above), no FEATURES stale-status errors.
- `python3 tools/validate_repository.py`: protection/ruleset/readback gates pass; expected exit 1 at `validate_backlog_exhaustion` with same 54 backlog/evidence errors. No FEATURES stale-status errors.
- No product code or frozen tests apply. Independent reviewer/controller retains acceptance authority. Candidate commit/push only; do not merge main.
