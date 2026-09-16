# GUARD-TRIAGE-6 — validate_repository / validate_plan triage

- validate_repository exit: 1 (FAIL at backlog exhaustion; ruleset-import OK, readback self-test pass, protection OK)
- validate_repository log: /tmp/opencode/validate6.log (122 `  - ` error lines; header says `validate_backlog_exhaustion: 122 error(s)`; tail: `validate_repository: FAIL backlog exhaustion exit=1`)
- validate_plan exit: 1; log: /tmp/opencode/validate_plan6.log (133 `  - ` error lines; header `validate_plan: 133 error(s)`)
- Delta vs baselines: 122/122 match (validate_repository inner exhaustion count); 133/133 match (validate_plan). Plan-repo delta = 11 = `Unknown task prefix` lines (SYNC/ACP/WSX/SDK/HEAD/RUN) present only in validate_plan.
- git status: 321 entries total = 203 modified + 118 untracked. diff-check: clean (exit 0).
- ralph.json: NOT edited (git diff --name-only -- ralph.json empty). No ralph.json change this run.
- Head commit: 248f519 feat: add native harness feature batches.
- Note: direct `validate_repository exit` capture needed two passes because rtk-wrapped tee swallowed `$?`; authoritative exits re-captured via redirect: VALREPO_EXIT:1, VALPLAN_EXIT:1.
