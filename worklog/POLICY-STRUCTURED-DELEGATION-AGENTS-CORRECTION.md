# POLICY-STRUCTURED-DELEGATION-AGENTS-CORRECTION

## Claim and scope

- Status: `completed` (feature candidate; independent verification and integration remain separate)
- Session: `ses_f2de5cddbffeQtkE264unOuVpT`
- Model/route: `9router/oc/space-bunny-free` / `oc-space-bunny-free`
- Task type: `implementation` (policy/documentation only)
- Role: repository-level delegation-policy correction author
- Branch/ref: `docs/structured-subagent-prompts`; starting revision `84d0effa0603b00841d9a80adbd52bf51172f35e`
- Owned file: `AGENTS.md`
- Permitted evidence paths: this scratchpad; own row in `tasks/completion/claims.json`
- Assigned route permission: confirmed. `oc-space-bunny-free` appears in the user allowlist copied in the task brief.

## Source evidence

- `AGENTS.md:23-82` — current structured-brief contract, authorization prose, disk-evidence safeguards.
- `AGENTS.md:84-189` — canonical reusable template; missing route/allowlist slots, uses broad `N/A` wording, handoff fields drift.
- `AGENTS.md:264-270` — current completion-report summary; inconsistent with the template.
- `docs/TDD.md:43-66` — RED must compile and fail; discovery/declarative work retains executable validators and a captured failing fixture; frozen hash required.
- `docs/SECURITY.md:1-82` — binding capability, secret, sandbox, approval, and stop safeguards.
- `docs/CONVERGENCE.md:87-105` — immutable tests, independent verification, exact integrated revision.
- `worklog/POLICY-STRUCTURED-DELEGATION-VERIFY.md:18-62` — F1 routing fields, F2 emergency stop, F4 discovery `N/A`, F5 handoff drift.
- Candidate policy commits: `0f8829a9942191394b83f4055de6d3c877aa31c1` (`AGENTS.md`), `c083eabe99a2b6e0abbe613ff9700313699171e8` (`.agents/WORKER.md`).
- Verifier report landed at `84d0effa0603b00841d9a80adbd52bf51172f35e`; verdict `ACCEPT WITH CORRECTIONS`.

## Target boundary and decisions

- Edit only `AGENTS.md`; make it the canonical correction source. Do not edit `.agents/WORKER.md`.
- Add one routing/authorization section with assigned route, exact allowlist or `N/A — no user allowlist`, and explicit route-permission confirmation.
- Add a stop-first exception. Stop before work tools/mutations. No-claim stop reports `no claim held`; no fabricated transition. Claimed stop records honest state only through safe existing APIs; otherwise reports exact recovery status. No new work.
- Restrict `N/A` to absence of product tests. Discovery/declarative executable validators, captured failing fixture, RED evidence, frozen-test status/hash, and contract validation remain required where applicable.
- Use one ordered handoff schema everywhere: task ID, task type, role, status, model/route, analysis, changes, commands/results, commit/ref, hashes, resource observations, unresolved gaps.
- Preserve exactly-one task type and the independent-verification boundary. No acceptance claim.

## Requirement matrix

| ID | Requirement | Final evidence | Result |
|---|---|---|---|
| R1 | Canonical assigned route | `AGENTS.md` "Assigned route and authorization" | PASS |
| R2 | Canonical allowlist or exact no-allowlist value | Same section contains `N/A — no user allowlist` | PASS |
| R3 | Route-permission confirmation | Same section explicitly confirms permission or exact no-allowlist sentinel | PASS |
| R4 | Immediate emergency stop | Top-level stop-first exception plus template emergency section | PASS |
| R5 | Honest no-claim/claimed stop handling | Exact `no claim held`; safe existing-API status action; exact recovery status otherwise | PASS |
| R6 | Stop grants no new work | Both emergency sections say cessation/status recording only, never new work | PASS |
| R7 | Discovery-safe `N/A` | Validation-only no-product-test sentinel; validator/failing fixture/RED/frozen/contract rules | PASS |
| R8 | Consistent handoff | Brief requirement, template, and Completion report share the same 12 fields and order | PASS |
| R9 | Independence preserved | Exactly-one task type and required independent-verification boundary remain | PASS |
| R10 | Scope/safeguards | `AGENTS.md` diff confined to structured contract/template and completion report; existing suffix preserved | PASS |

## Textual scenario matrix

| Scenario | Policy path exercised | Required result |
|---|---|---|
| Complete implementation brief | All canonical headings; exact allowlist contains assigned route; frozen RED identified; independent verifier named; complete handoff | Pass intake; claim only after validation |
| Incomplete raw work request | Raw/partial briefs forbidden | Stop before claim/edit; request corrected brief |
| Urgent stop before claim | Emergency stop overrides completeness and claim-first intake | Stop immediately; report `no claim held`; no ledger mutation/new work |
| Discovery task says product tests `N/A` | `N/A` limited to product tests | Require executable validator and captured failing fixture; no RED/frozen/contract bypass |

## Validation plan

- Bounded inline Python textual requirement/scenario assertions against `AGENTS.md`.
- `rtk git diff --check`.
- Changed-path and hunk review; confirm no deletion outside the structured-policy section.
- `rtk python3 tools/validate_repository.py`; report any pre-existing repository-level failure without acceptance.
- Explicit push: `rtk git push origin HEAD:refs/heads/docs/structured-subagent-prompts`; fetch; prove local commit is contained remotely.

## Validation results

- Final inline Python textual checker: `requirements=15 failed=0`; all four required scenarios PASS.
- Checker setup notes: first draft ended at the nested shell fence (`IndexError`); second draft required labels with colons and failed the unlabelled Completion report list. The corrected parser uses the outer section boundary and label extraction. No policy text was changed to satisfy either harness defect.
- `rtk git diff --check` -> PASS.
- `rtk git diff --numstat 84d0effa0603b00841d9a80adbd52bf51172f35e -- AGENTS.md` -> `110 33`; review confirms all changes are confined to the structured-prompt contract/template and Completion report.
- `rtk python3 tools/validate_repository.py` -> expected pre-existing FAIL: `validate_backlog_exhaustion: 51 error(s)`; protection fixture passed but hosting-platform state remains unverified. No product, plan, controller, verifier, test, or acceptance change made.
- `rtk git fetch --prune origin` -> PASS; pre-commit `HEAD` and `origin/docs/structured-subagent-prompts` both `84d0effa0603b00841d9a80adbd52bf51172f35e`.
- Segment comparison against `84d0eff` -> PASS for unchanged pre-structured authority, convergence/TDD/security/scratchpad sections, and resource/claim/landing suffix.
- Final policy evidence: `AGENTS.md:25-48` stop/N/A semantics; `:94-101` routing authority; `:132-137` canonical routing fields; `:181-200` validation semantics; `:209-220` emergency template; `:232-247` canonical handoff; `:323-347` matching completion report.

## Resource observations and unknowns

- Workload: one Markdown policy file, one scratchpad, one own ledger row, plus bounded text/Git/Python checks. No Cargo, browser, database, credentials, broad host access, parallel workers, or resource-heavy command.
- `rtk sysctl -n hw.memsize` -> `25769803776` bytes; `rtk vm_stat` recorded only. Work remained sequential and lightweight.
- Existing unstaged `tasks/completion/claims.json` change belongs to verifier landing. Preserve unstaged; stage/commit only this task's ledger row.
- `.agents/WORKER.md` parity remains a separate lane. This correction will make `AGENTS.md` canonical; no claim of combined-policy acceptance.
