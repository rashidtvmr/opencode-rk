# FEATURES-REOPEN-2-W1

Claim: serialized lane `FEATURES-REOPEN-2-W1`, session `ses_f1e141a27ffebqsvOedkWx02eF`; base `693ab9a2aa11802f855acfab9abf2bb9ea00900b`; worktree `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/reconcile-features-34-w1`; branch `reconcile/FEATURES-REOPEN-34-W1`.

Source evidence: at the base, `FEATURES.md:46,53` showed AUTO-005/EXT-008 accepted while preserving their unresolved evidence-conflict caveats. Ralph's canonical `ralph.json` story records at those entries also said accepted, but current controller-authorized target is 224 accepted, 23 in-progress, 11 not-started. `tasks/AUTO-005.md:3,11,26-30,51-57` says not started and documents absent controller-frozen RED/GREEN and independent rerun evidence. `tasks/EXT-008.md:3,22-29,46-60` says not started and records plugin-hook-contract mismatch, explicit blocker, and unresolved ownership/semantics. `tools/validate_backlog_exhaustion.py:412-425` defines mirrored status extraction across inline and table occurrences. Historical acceptance notes are candidate evidence, not acceptance authority (`docs/TDD.md:74-82`, `PLAN.md:113-120`).

Boundary: only `FEATURES.md` status mirrors and this scratchpad/claim. AUTO-005 and EXT-008 status set to not-started in all seven status-bearing occurrences each (historical proposal table, requirement rows, story summaries); both caveats preserved. Header and historical count mirror state 224/23/11. No product, test, plan, policy, or Ralph edits. No runtime/persistence/resource behavior changed.

Verification: direct `validate_backlog_exhaustion._features_status_errors` must return no status-mirror errors; story counts must match Ralph exactly. Run `git diff --check`, `python3 tools/validate_plan.py`, `python3 tools/validate_repository.py`; full plan/repository validators are expected to remain blocked by pre-existing classifier/backlog/ledger failures, not this mirror. Convergence gate at intake failed on 59 pre-existing claim blockers including off-plan completed rows and AUD-017/AUD-020 notes admitting no acceptance.

Unresolved: independent acceptance evidence for both stories remains absent; their statuses are not accepted. Existing plan/repository classifier and ledger findings remain outside this lane. No test files authored or altered.

## Verification and handoff

Correction: AUTO-005 appears in four status mirrors and EXT-008 in three (seven combined), not seven each. All occurrences match Ralph. `python3` asserted counts `accepted=224`, `in-progress=23`, `not-started=11`; `tools.validate_backlog_exhaustion._features_status_errors` returned `[]`. `rtk git diff --check` passed. `rtk python3 tools/validate_plan.py` failed with 54 pre-existing backlog/classifier errors; `rtk python3 tools/validate_repository.py` failed at the same 54-error backlog gate. `rtk python3 tools/convergence_gate.py` intake failed with 59 existing ledger blockers. No product tests apply to this status-mirror reconciliation.

Current change limited to `FEATURES.md`, this scratchpad, and this task claim. Candidate branch only; no main merge.
