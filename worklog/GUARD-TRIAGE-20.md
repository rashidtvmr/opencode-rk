# GUARD-TRIAGE-20 — verification receipt

Scope: read-only guard triage. No product/controller/test edits.
Owned file: worklog/GUARD-TRIAGE-20.md only.

## Commands (all timeout 120, rtk-prefixed)
- `rtk python3 tools/validate_repository.py` → /tmp/opencode/p6-validate.log
- `rtk python3 tools/validate_plan.py` → /tmp/opencode/p6-plan.log (tail)
- `rtk git diff --check`
- `rtk git status --short` (+ counts)

## Results
- validate_repository: EXIT 1, `validate_backlog_exhaustion: 178 error(s)`, dash-lines 178.
- validate_plan: EXIT 1, `validate_plan: 189 error(s)`, dash-lines 189.
- git diff --check: EXIT 0 (whitespace clean).
- git status --short: 3 entries total → M=1, ??=2.
  - `M ralph.json` (diffstat: 82+/82-, single file)
  - `?? worklog/ACCEPTANCE-FLIP-PROPOSAL.md`
  - `?? worklog/TWINS-WATCH7.md` (appeared mid-run; concurrent lane active)

## Delta vs 122/133 baseline
- Baseline: repo 122 / plan 133. Current: repo 178 / plan 189.
- Delta: +56 / +56 (symmetric growth in both validators).
- Cause class (from log heads): `ralph.json` in-progress→accepted flips
  landed without updating exhaustion ledger / FEATURES.md accounting:
  - `controller-accepted stories must never appear in the exhaustion ledger`
    (98 accepted IDs now classified in ledger).
  - `FEATURES.md: <ID> has stale statuses [...] expected accepted` tail
    (plan tail ends at TOOL/SYNC/RUN/ACP/WSX/SDK/HEAD flips).
  - Backlog-exhaustion missing-non-accepted list now empty (0); staleness
    shifted to accepted-in-ledger + FEATURES drift.
- First-run 122 vs later 124 count in this session = same log counted with
  `grep -c "  - "` (124, includes non-error dash lines) vs validator header
  (122); later 178 runs reflect concurrent ralph.json flips landing mid-run.
- Whitespace gate clean; no product/controller/test files touched by this lane.

## Paths
- /tmp/opencode/p6-validate.log
- /tmp/opencode/p6-plan.log
- /tmp/opencode/p6-status2.log (status snapshot)
