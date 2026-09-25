# PHASE1-CONVERGENCE-RECONCILE

## Claim and boundary

- Candidate `7262682e31c1f473912c9483804c9430eb4ee4c5`; branch `plan/PHASE1-CONVERGENCE-RECONCILE`.
- Own only this scratchpad plus this task's claim row. Product, controller, tests, policy, and source manifests untouched.
- Existing worktree edit to `sources/disc-003-reconciliation.manifest.json` is pre-existing and outside this lane.

## Reproduction

`python3 tools/convergence_gate.py` => `CONVERGENCE BLOCKED`, `total=59`: exactly 57 completed IDs absent from the canonical plan, plus `AUD-017` and `AUD-020`, each completed with a note containing `no acceptance`.

## Complete 59-row authority manifest

Disposition: `BIND` means evidence candidate only, bind to a canonical plan owner/dependency and independently verify; it cannot close a parent. `REOPEN` means the note admits unfinished product/artifact wiring and the product boundary must reopen. `PRESERVE` means retain audit completion as evidence while parent stays open; no acceptance.

| ID | Exact finding in completed note | Proposed disposition |
|---|---|---|
| FIX-LOGROTATE | VERIFY-ONLY; diagnostics 7/7; transcript-entry 8 KiB cap and 128/256 bounds; no commit | BIND |
| FIX-LOOP-RULES | rules_globs 8/8 green; frozen tests unchanged | BIND |
| FIX-NATIVE-DAEMON | VERIFY-ONLY zero-diff; run_with_dir/stdin guard/resolve_origin_bearer; bins green; piped TUI exit 1, --once exit 0 | BIND |
| FIX-PACKAGING | --help identity gate; install_commands 5/5 green | BIND |
| FIX-SANDBOX | external DISC-106 coverage; detection/confinement/cap-close green; raw Landlock attach policy-gated | BIND |
| FIX-SESSIONS-STUBS | snapshot fold 3/3 green; integrator must add pub mod snapshot and lifecycle call | REOPEN |
| FIX-SQLITE-GATE | backports rejected without audited sqlite_source_id allowlist; storage tests green | BIND |
| FIX-TIMELINE | native_timeline 11/11 and native_shell 9/9 green; no commit | BIND |
| G6-CHAT-DATADIR | chat.rs forwards --data-dir; CLI bins green | BIND |
| LANE-AGENT-FILES | agent_files 14/14 and crate 126 green at 767a86a | BIND |
| LANE-APPSTART-VIEW | app_start standalone 16/16; cargo bin blocked by pre-existing tools/shell_tool and OpenTUI link errors | BIND |
| LANE-AUTH-401 | daemon_auth_api 5/5; 401/403 already correct | BIND |
| LANE-AUTODRIVE-CLAMP | py_compile green; default 2, hard maximum 4 serialized | BIND |
| LANE-CHAT-ORIGIN | CLI bins green; origin binding exact | BIND |
| LANE-CI | ci_output standalone 6/6 green | BIND |
| LANE-CI-CAPS | per-step Cargo/thread caps; repository validation fails backlog exhaustion | BIND |
| LANE-CI-EXT | 11/12 verified; T03 gated by pre-existing STREAM regression owned elsewhere | BIND |
| LANE-CI-FLAG | ci_mode plus ci_output 9/9 green | BIND |
| LANE-COMMANDS-LIVE | command_templates 14/14 and agents lib 34/34 green | BIND |
| LANE-CONTEXT-ACCOUNT | context_accounting 12/12; wired lib.rs:32 by integrator | BIND |
| LANE-CONTEXT-CMD | context_report standalone 11/11 green | BIND |
| LANE-DESC-STALE | daemon 15/15 plus auth 5/5; three daemon.rs tests added | BIND |
| LANE-DISPATCH-DENY | registry_dispatch 7/7 green | BIND |
| LANE-FILE-AUTHZ | file_ops 5/5; execute_authorized landed in file_ops.rs | BIND |
| LANE-GLOBS | rules_globs 8/8 at 767a86a | BIND |
| LANE-GLOBS-LIVE | rules_globs_live 3/3 at 767a86a | BIND |
| LANE-LOOP | loop_driver 10/10 verified twice | BIND |
| LANE-LOOP-CAP | agent_loop 6/6; end_round truncates ToolRound to 16 | BIND |
| LANE-LOOP-LIVE | loop_driver_live 16/16 verified | BIND |
| LANE-MAIN-ONCE2 | CLI check green; TUI --once/frame exit 0 | BIND |
| LANE-MCP-LIVE | mcp_session 15/15 verified | BIND |
| LANE-ONBOARD-SETUP | onboarding 13/13; RED 3d67c670, GREEN ef30545b; zero post-freeze test edits | BIND |
| LANE-PROV-FALLBACK | fallback 6/6; zero frozen-test edits | BIND |
| LANE-RALPH-MAX2 | py_compile green; default/effective 2; hard cap 2 | BIND |
| LANE-RULES | rules_loader 6/6 verified twice | BIND |
| LANE-SANDBOX | sandbox 12/12, security 148, diagnostics 7/7, doctor 5/5 | BIND |
| LANE-SHELL-AUTHZ | shell_tool 9/9 including broker gates | BIND |
| LANE-SRV-ROUTER | server lib 205/205; router_with_auth retained | BIND |
| LANE-SUBAGENT-LIVE | delegation_live 11/11 and crate green | BIND |
| LANE-THEMES | native_theme 19/19 verified | BIND |
| LANE-TIMELINE-LAND | bins zero errors, 441 pre-existing warnings; additive 286 lines | BIND |
| LANE-TOOL-PERM | permission 5/5 plus tools lib 99 | BIND |
| LANE-TRANSCRIPT-LAND | bins zero errors, 445 pre-existing warnings; transcript rendering compiles | BIND |
| LANE-TUI-GRAPH | native_graph 11/11 verified | BIND |
| LANE-TUI-HOST | native_host 5/5 and bins green; module wiring left to integrator | REOPEN |
| LANE-TURN-SETTLE | standalone turn_service 9/9; full crate blocked by pre-existing shell_tool breakage | BIND |
| LANE-ULTRA-CODEGEN | ultra_codegen 20/20 and workflow_schema 13/13; integrator wired | BIND |
| LANE-WEB-CANVAS | canvas model 16/16; implementation restored by verifier and remains untracked | REOPEN |
| LANE-WEB-HONEST | web_assets/capability suites 15/15; branch 43172b5 | BIND |
| LANE-WF-CREATE | workflow_schema 13/13; integrator wired | BIND |
| LANE-WF-TIMELINE | native_timeline 11/11 twice | BIND |
| PHASE1-VERTICAL-SYNTHESIS | proposal-only: 20 lanes, 83 stages, 3 waves; repository 51 and convergence 58 blockers preserved | BIND |
| RC-01 | server lib 205/205 plus daemon_auth_api 5/5 | BIND |
| RC-02 | daemon 21 plus server lib 205 | BIND |
| RC-03 | service_commands 14/14 | BIND |
| WEB-EVENT-STREAM | event_stream 5/5 | BIND |
| WEB-HINT | composer-effort 8/8 and TypeScript clean; blobs restored/untracked | BIND |
| AUD-017 | SHAPE OK 8 findings/5 repairs; test_auto 16 runs/6 failures; leases_validate 18/2; frozen drift; no acceptance | PRESERVE |
| AUD-020 | export 258 legacy/258 accepted/237 empty-deps/82 TBD +109 completion; all 258 require revalidation; no acceptance | PRESERVE |

Only `FIX-SESSIONS-STUBS`, `LANE-TUI-HOST`, and `LANE-WEB-CANVAS` are proposed reopenings: their own notes admit unwired integrator work or an untracked artifact. No existing status is changed here.

## Protected owner, order, and guard interaction

Canonical owner: `@rashidtvmr` (catch-all plus exact protected records in `.github/CODEOWNERS`). Exact guard order from `tools/validate_repository.py:21-27`:

1. `tools/render_ruleset_import.py --check`
2. `tools/verify_ruleset_readback.py --self-test`
3. `tools/validate_protection_policy.py`
4. `tools/validate_backlog_exhaustion.py`
5. `tools/reconcile_surfaces.py --check-manifest`
6. `tools/validate_plan.py`

Protected owner/files/order is authoritative in `.github/protection-policy.json:20-76`; owner records are `.github/CODEOWNERS:4-58`. The exact protected sequence is: `.github/CODEOWNERS`, `.github/protection-policy.json`, `.github/rulesets/main.disabled.json`, `.github/workflows/ci.yml`, `AGENTS.md`, `FEATURES.md`, `PLAN.md`, `README.md`, `config/controller.settings.json`, `config/resource-targets.json`, `config/security-policy.json`, `docs/ADAPTER_PROTOCOL.md`, `docs/AUTONOMOUS_EXECUTION.md`, `docs/REPOSITORY_PROTECTION.md`, `docs/SECURITY.md`, `docs/TDD.md`, `ralph.json`, `requirements/user-requirements.json`, `sources/backlog-exhaustion.json`, `sources/behavior-surface-rules.json`, `sources/disc-003-evidence.json`, `sources/disc-003-reconciliation.json`, `sources/disc-003-reconciliation.manifest.json`, `sources/enterprise-remote-spec-gap.json`, `sources/evidence.json`, `sources/extensibility-remaining-ownership-gap.json`, `sources/integrations-ownership-gap.json`, `sources/operations-ownership-gap.json`, `sources/release-assurance-gap.json`, `sources/req017-extensibility-ownership-gap.json`, `sources/routing-ownership-gap.json`, `sources/sharing-ownership-gap.json`, `sources/upstream.lock.json`, `tasks/DISC-003.md`, `tests/bootstrap/test_backlog_exhaustion.py`, `tests/bootstrap/test_ci_enforcement.py`, `tests/bootstrap/test_disc003_reconciliation.py`, `tests/bootstrap/test_protection_policy.py`, `tests/bootstrap/test_ruleset_readback_verifier.py`, `tests/bootstrap/test_validate_plan.py`, `tests/fixtures/rulesets/github-active-incomplete-export.json`, `tests/fixtures/rulesets/github-active-matching.json`, `tests/fixtures/rulesets/github-active-mismatching.json`, `tools/auto_drive.py`, `tools/lane_gate.py`, `tools/plan_model.py`, `tools/ralph_loop.py`, `tools/reconcile_surfaces.py`, `tools/render_ruleset_import.py`, `tools/validate_backlog_exhaustion.py`, `tools/validate_plan.py`, `tools/validate_protection_policy.py`, `tools/validate_repository.py`, `tools/verify_ruleset_readback.py`, `workspaces/DISC-003/progress.md`, `workspaces/DISC-003/source-map.json`.

Observed results: protection validator PASS (`@rashidtvmr`, 56 paths); `reconcile_surfaces.py --check-manifest` PASS (32 partial families, 182 evidence references, 79 unresolved findings); `validate_backlog_exhaustion.py` exactly 51 errors; `validate_plan.py` exactly 51; canonical guard stops at check 4 with `backlog exhaustion exit=1`.

The 51 errors are: 3 global classification/accepted-row/summary-drift errors; stale routing `ROUTE-009`, `ROUTE-010`; stale plus task/worklog review errors for `OPS-001..OPS-006`, `OPS-008`, `REL-001..REL-003`, `EXT-001`, `EXT-002`, `SHARE-001..SHARE-005`; stale ownership errors for `EXT-004`, `EXT-006`, `EXT-009..EXT-012`, `INT-001`, `INT-003`, `INT-005..INT-007`, `INT-009`. The exact accepted/unknown set emitted by the first error is recorded by the command output and consists of `AUTO-003..AUTO-007`, `EXT-001..EXT-013`, `INT-001..INT-010`, `OPS-001..OPS-010`, `PROV-014`, `REL-001..REL-004`, `ROUTE-001..ROUTE-012`, `SESS-019`, `SESS-020`, `SHARE-001..SHARE-005`, `TOOL-015`, `UI-001..UI-018`, and `WEB-001..WEB-017`.

DISC-003 remains `in-progress-not-release-evidence`; 32 families remain `partial`, 79 findings unresolved. Its checked manifest is review evidence only. The pre-existing one-line `planSha256` diff in `sources/disc-003-reconciliation.manifest.json` is not regenerated or approved by this lane; intentional plan/input edits require protected-owner review plus manifest regeneration before guard acceptance.

## Verification and landing

Commands run:

```text
python3 tools/convergence_gate.py                         # FAIL total=59
/usr/bin/python3 tools/validate_repository.py             # FAIL backlog exhaustion, 51
python3 tools/validate_plan.py                            # FAIL, 51
/usr/bin/python3 tools/reconcile_surfaces.py --check-manifest # PASS
```

Before commit: `git diff --check`; update only this task's claim to `completed` with the above exact evidence. Do not mutate existing statuses. This manifest is reconciliation evidence, never acceptance.
