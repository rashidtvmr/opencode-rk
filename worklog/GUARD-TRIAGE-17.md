# GUARD-TRIAGE-17 — verification

Claim: repo guard + plan guard FAIL on stale FEATURES/backlog accounting; worktree has large unstaged/test delta, zero whitespace errors. No change vs TRIAGE-16 except 8 new untracked files.

Evidence:
- tools/validate_repository.py exit=1, `validate_backlog_exhaustion: 122 error(s)`, 122 `  - ` lines in /tmp/opencode/bN-validate.log
- tools/validate_plan.py exit=1, `validate_plan: 133 error(s)`, 133 `  - ` lines in /tmp/opencode/bN-plan.log
- `git diff --check` exit=0, empty (/tmp/opencode/bN-diffcheck.log, 0 bytes)
- `git status --short`: 456 total = 204 unstaged-modified (` M`), 0 staged, 252 untracked (`??`), log /tmp/opencode/bN-status.log

Observed scenario:
- Repo validator FAIL classes unchanged: missing non-accepted stories (27: ACP/HEAD/PROV-015..024/RUN/SDK/SYNC/TOOL-016..020/UI-019/WSX), accepted/unknown in exhaustion ledger (43), controller-accepted in ledger, PROV-015 unclassified, WEB-006/007/008 IMPLEMENTED-but-stale-local, ROUTE/OPS/REL/EXT/SHARE/INT ownership-gaps stale after Ralph semantics + task/worklog-appeared needs-review, ~70 FEATURES.md stale statuses.
- bN-validate.log vs aO-validate.log: same 122 lines, one line-order swap only (WEB-006 entry moved one position). bN-plan.log vs aO-plan.log: byte-identical.
- Plan validator tail: FEATURES.md stale UI/WEB/AUTO/PROV/OPS/EXT/SESS/ROUTE/REL statuses; head shows unknown task prefixes SYNC/RUN/ACP.

Target boundary: READ-ONLY; owned file only this worklog. No edits to ralph.json/FEATURES.md/controller/product/tests.

Tests:
- `python3 tools/validate_repository.py` (rtk + timeout 120 wrapper attempted; rtk prefix passthrough, ran bare to capture log) → exit 1, 122 errors, log /tmp/opencode/bN-validate.log
- `rtk python3 tools/validate_plan.py` with timeout 120 → exit 1, 133 errors, log /tmp/opencode/bN-plan.log
- `git diff --check` → exit 0, log /tmp/opencode/bN-diffcheck.log
- `git status --short` → 204 M + 252 ?? = 456 total, log /tmp/opencode/bN-status.log

Decisions:
- Delta classification vs TRIAGE-16 baseline (204 M + 244 ?? = 447/448): fixed=0, new=8 untracked, pre-existing=remainder.
- New (8): crates/server/tests/web_capabilities_full.rs, crates/server/tests/web_workspace_full.rs, worklog/AUTO-005-VALIDATION2.md, worklog/EXT-TWINS-DISPOSITION2.md, worklog/GUARD-TRIAGE-16.md, worklog/INTEGRATION-15.md, worklog/SHARE-DEDUP5.md, plus WSX-SDK-HEAD-GATE.md newline-only diff (was missing trailing newline in prior snapshot).
- Guard FAILs are accounting/semantic staleness (FEATURES.md vs controller, Ralph semantics), not diff-check failures. Status count drift 447→456 across runs = live worktree with parallel lanes writing worklogs/tests.

Remaining unknowns: none for triage; integration decision belongs to verifier/controller.
