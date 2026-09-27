# RALPH-REOPEN-32-W1

- Claim: task ID is not yet listed in `ralph.completion.json`; controller authorization supplied. Ledger claim: `tasks/completion/claims.json`, session `ses_f1e4e6058ffesfXnGzxlpZBGJ3`, status `in-progress`.
- Candidate: branch `reconcile/RALPH-REOPEN-32-W1`, starting commit `8a91a7b49a5a1c948218ad8f176d44e015530dcb`, exact `origin/main`.
- Source evidence: `tools/plan_model.py:59,171-185` accepts legacy status vocabulary; `tools/completion_plan.py:144` normalizes legacy status for additive plan and says historical accepted flags are not evidence (`:6`); `tools/validate_backlog_exhaustion.py` validates classifications against `ralph.json`.
- Boundary: only `ralph.json` statuses, this scratchpad and own claims row. No `FEATURES.md`, backlog, tooling, tests or policy changes. Preserve all other story fields and 50 proposed accepts. AUTO-005/EXT-008 evidence conflict recorded for later review, untouched.
- Change: reopened 23 stories as `in-progress`, 9 as `not-started`; 258 total unchanged; 226 remain accepted. Status-only diff; no story text, dependencies, requirements or obligations changed.
- Checks: JSON parse and asserted exact changed ID set/status counts; `git diff -- ralph.json` shows exactly 32 status replacements. Before change, `python3 tools/validate_plan.py` failed on backlog-exhaustion reconciliation, including classification and stale ownership gap errors. Read-only `python3 tools/validate_repository.py` likewise failed at backlog exhaustion; canonical protection/render/readback checks passed. These cross-file failures await serialized FEATURES/backlog lanes; not repaired here.
- Remaining: commit/push candidate branch only after exact-diff and JSON verification. Do not merge main. Parent integration must reconcile backlog and review AUTO-005/EXT-008 evidence.

## 2026-09-27 verification update

- Exact `HEAD:ralph.json` comparison: 258 stories, exactly 32 changed rows; only `status` differs. Reopened 23 `in-progress`, 9 `not-started`; 226 remain accepted. This preserves every other record including proposed accepts; AUTO-005 and EXT-008 unchanged.
- `python3 -m json.tool ralph.json`: pass.
- `python3 tools/validate_plan.py`: expected fail (86 errors after the planned status changes): backlog/reconciliation needs serialized cross-file updates, notably PROV-016 classification and FEATURES stale statuses. No unauthorized cross-file edits made.
- `python3 tools/validate_repository.py`: expected fail at backlog exhaustion for same serialized reconciliation; render ruleset, readback fixture, protection policy checks pass.
- No source tests apply to this status-only reconciliation. Candidate commit/push requested; keep branch unmerged until FEATURES/backlog lanes serialize.
