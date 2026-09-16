# GUARD-TRIAGE-18 — repository guard verification

Scope: read-only verification. No edits to ralph.json / FEATURES.md / controller / product / tests. Owned file only.

## Commands (timeout 120, workdir /home/rashid/projects/opencode-rk)

- `python3 tools/validate_repository.py` -> /tmp/opencode/cJ-validate2.log (also /tmp/opencode/cJ-validate.log)
  - EXIT: 1
  - COUNT: 122 error(s) — matches expected 122. `grep -c "^  - "` = 122.
  - Tail: `validate_repository: FAIL backlog exhaustion exit=1`
  - Head: render_ruleset_import OK, fixture-self-test-passed (verified:false), protection_policy OK.
  - Breakdown: missing non-accepted stories (27 listed: ACP/HEAD/PROV-015..024/RUN/SDK/SYNC/TOOL-016..020/UI-019/WSX), accepted-in-ledger (~43), stale ownership-gaps (ROUTE/OPS/REL/SHARE/EXT/INT), FEATURES.md stale statuses (~70+).
- `python3 tools/validate_plan.py` -> /tmp/opencode/cJ-plan2.log
  - EXIT: 1
  - COUNT: 133 error(s) (`validate_plan: 133 error(s)`; file 134 lines incl header; `grep -c "^  - "` = 133)
  - Includes: 11x unknown task prefix (SYNC/RUN/ACP/WSX/SDK/HEAD) + same 122 backlog/FEATURES/gap errors + WEB-006/007/008 IMPLEMENTED-but-stale.
- `git diff --check`
  - EXIT: 0 (no whitespace errors)
- `git status --porcelain=v1`
  - TOTAL: 462 lines
  - M (unstaged modified): 204 (`" M"` = 204, staged `^M` = 0)
  - ??: 258 untracked
  - rtk-filtered `git status --short` display truncates (shows 20/30-file sample + "+N more") but porcelain counts authoritative.

## Delta classification

- Controller/plan untouched: `git status | grep ralph.json|FEATURES.md|controller|PLAN.md` -> NO_CONTROLLER_TOUCH.
- Modified (204, unstaged only), by dir: tools/tests 31, providers/tests 26, server/tests 25, sessions/tests 24, tools/src 17, server/src 14, providers/src 14, sessions/src 13, foundation/tests 13, foundation/src 12, agents/src 3, storage/src 2, security/src 2, catalog/src 2, agents/tests 2, worklog 1, storage/tests 1, security/tests 1, catalog/tests 1.
- Untracked (258), by dir: worklog 212, providers/tests 12, server/tests 11, server/src 7, tools/tests 4, sessions/tests 4, sessions/src 3, cli/tests 2, cli/src 2, agents/tests 1.
- Class: product-lane implementation + tests (crates/*/src + crates/*/tests) plus worklog scratchpads. No guard/config drift. Whitespace clean.

## Paths

- Repo log: /tmp/opencode/cJ-validate2.log (primary; /tmp/opencode/cJ-validate.log identical run)
- Plan log: /tmp/opencode/cJ-plan2.log (also /tmp/opencode/cJ-plan.log)
- Status raw: `git status --porcelain=v1` (462 lines; no file written — regenerable)
- This report: worklog/GUARD-TRIAGE-18.md

## Verdict

FAIL (expected): backlog exhaustion 122, plan 133, both exit 1. No new guard failure class introduced by this lane; delta is product/tests/worklog only, diff-check clean.
