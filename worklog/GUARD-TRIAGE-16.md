# GUARD-TRIAGE-16 — verification

Claim: repo guard + plan guard FAIL on stale FEATURES/backlog accounting; worktree has large unstaged/test delta, zero whitespace errors.

Evidence:
- tools/validate_repository.py exit=1, `validate_backlog_exhaustion: 122 error(s)`, 122 `  - ` lines in /tmp/opencode/aO-validate.log
- tools/validate_plan.py exit=1, `validate_plan: 133 error(s)`, 133 `  - ` lines in /tmp/opencode/aO-plan.log
- `git diff --check` exit=0, empty (/tmp/opencode/aO-diffcheck.log)
- `git status --short`: 447 total = 204 unstaged-modified (` M`), 0 staged, 244 untracked (`??`)

Observed scenario:
- Repo validator FAIL classes: missing non-accepted stories (27 listed: ACP/HEAD/PROV-015..024/RUN/SDK/SYNC/TOOL-016..020/UI-019/WSX), accepted/unknown in exhaustion ledger (43 listed), controller-accepted in ledger, PROV-015 unclassified, WEB-006/007/008 IMPLEMENTED-but-stale-local, ROUTE/OPS/REL/EXT/SHARE/INT ownership-gaps stale after Ralph semantics + task/worklog-appeared needs-review, ~70 FEATURES.md stale statuses (accepted vs in-progress/not-started; in-progress vs not-started).
- Plan validator tail: FEATURES.md stale UI/WEB/AUTO/PROV/OPS/EXT/SESS/ROUTE/REL statuses; head shows unknown task prefixes SYNC/RUN/ACP.
- Delta: 204 modified across crates/tools, providers, server, sessions, foundation, agents, storage, security, catalog + 1 worklog; 244 untracked dominated by 200 worklog files + providers/server/tools/sessions/cli tests+src. No staged changes. Whitespace clean.
- Count drift 242→244 untracked across runs = live worktree, recount from /tmp/opencode/aO-status2.log.

Target boundary: READ-ONLY; owned file only this worklog. No edits to ralph.json/FEATURES.md/controller/product/tests.

Tests:
- `timeout 120 rtk python3 tools/validate_repository.py` → exit 1, 122 errors, log /tmp/opencode/aO-validate.log
- `timeout 120 rtk python3 tools/validate_plan.py` → exit 1, 133 errors, log /tmp/opencode/aO-plan.log
- `rtk git diff --check` → exit 0
- `rtk git status --short` → 204 M + 244 ?? = 447 total, log /tmp/opencode/aO-status2.log

Decisions:
- Delta classified as test-heavy in-progress lane work + worklog churn, not whitespace breakage.
- Guard FAILs are accounting/semantic staleness (FEATURES.md vs controller, Ralph semantics), not diff-check failures.

Remaining unknowns: none for triage; integration decision belongs to verifier/controller.
