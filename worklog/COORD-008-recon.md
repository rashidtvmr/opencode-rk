# COORD-008-recon worklog — old-vs-new reconciler

Claim: create `tools/completion_reconcile.py` per COORD-008 card journey
(reconcile old/new controller gaps, prefix/dependency semantics, entrypoints).
Source evidence: HEAD 5af7884; card via completion_plan --card COORD-008
(deps COORD-003..007, tests T01..T05 = prd-bypass, dep-protection,
families-load, entrypoint-parity, guards-pass);
`tools/ralph_loop.py:601-608` (accepted set before finalize_worktree,
kept accepted-but-not-integrated); `tools/plan_model.py:193-205`
(rank closure synthesized only when zero authored deps exist);
`ralph.json` 258 legacy rows all status accepted;
`prd.json` schemaVersion 1 flat tasks (id/status/userStory/testObligations,
no deps/evidence); `ralph.completion.json` contract
legacyAcceptedIsReleaseEvidence false; audit shards AUD-009 (ACP),
AUD-010 (SYNC/RUN/WSX/SDK), AUD-001 (BASE/HEAD).

Observed scenario: file absent (tools/ had completion_{budgets,ownership,
plan,scheduler,verification}.py only). Target boundary: owned file only,
no edits to controller/plan/guards. Stdlib only, pure, bounded 10000,
no processes/network/writes.

Tests: no frozen suite exists for this file. Verified live:
`timeout 60 python3 -m compileall -q tools/completion_reconcile.py` OK;
`timeout 30 python3 tools/completion_reconcile.py` -> "reconcile self-check:
OK" (asserts: accepted-w/o-integrated flagged; flat prd + prd-as-evidence
rejected; rank-closure protection enforced; cycle/unknown-dep fail;
SYNC/RUN/ACP/WSX/SDK/HEAD coverage incl. catch-all-AUD-020 reject;
entrypoint refuse-unsafe vs same-policy; full reconcile GREEN case);
`timeout 30 python3 tools/completion_plan.py --check` -> SPEC OK
(additions=90, legacy=258, scenarios=450).

Decisions: local PHASE_RANK copy incl. SYNC/RUN/ACP/WSX/SDK/HEAD ranks
(import-safe, no shared mutable drift); reconcile() aggregates T01..T04
guards; T05 (bootstrap/canonical guards) left to existing
validate_repository.py, untouched.

Remaining unknowns: none for lane. RED/GREEN freeze + verifier rerun belong
to test author/verifier lanes, not this one-file lane.
