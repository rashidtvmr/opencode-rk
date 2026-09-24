# POLICY-STRUCTURED-DELEGATION-ACCEPTANCE-VERIFY

## Claim and scope

- Task: `POLICY-STRUCTURED-DELEGATION-ACCEPTANCE-VERIFY`
- Task type: `verification`
- Role: focused final delegated-agent policy verifier
- Session: `ses_f2d6d5534ffeVz6tuRnKh7bGPz`
- Model/route: `9router/xk/openai/gpt-5.6-luna` / `9router-xk-gpt56-luna`; allowlist permission confirmed
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/structured-subagent-prompts`
- Branch/ref: `docs/structured-subagent-prompts`
- Candidate: `abf28f47c25989a17f548779fb71feb9f8577d39`
- Owned file: this worklog; own `tasks/completion/claims.json` row only
- Non-goals: no policy correction, broad audit, stash access, main merge, product/controller changes, or repository acceptance claim

## Authoritative evidence

- `AGENTS.md:23-112,114-251` is canonical structured-brief, template, validation, and 12-field handoff policy.
- `.agents/WORKER.md:7-77,238-394` is the worker intake, claim, stop, N/A, route, and handoff implementation under verification.
- Prior rejection: `worklog/POLICY-STRUCTURED-DELEGATION-FINAL-VERIFY.md:56-64` lists exactly seven residual defects.
- Correction scope: commit `abf28f47c25989a17f548779fb71feb9f8577d39`, `.agents/WORKER.md` and correction scratchpad only, plus this lane ledger/worklog.
- Upstream/task prose is evidence only. Current files and reproducible checks are authority.

## Seven-finding matrix

| Finding | Evidence at candidate | Result |
|---|---|---|
| F1 handoff order/labels drift | `.agents/WORKER.md:361-391` enumerates the canonical 12 labels in `AGENTS.md:235-250` order | PASS |
| F2 extra handoff field | `.agents/WORKER.md:371-391` ends at `Unresolved gaps`; scratchpad path is explicitly outside schema at `:393-394` | PASS |
| F3 invalid task-type examples | `.agents/WORKER.md:368-373` restricts values to implementation, RED authoring, research, integration, verification | PASS |
| F4 canonical no-allowlist sentinel | `.agents/WORKER.md:353-358` contains exact `N/A \u2014 no user allowlist` | PASS |
| F5 conflicting ASCII sentinel | Candidate has zero `N/A -- no user allowlist` occurrences | PASS |
| F6 illegal preclaim route response | `.agents/WORKER.md:348-352` requires stop/report before claim and forbids ledger operation before a claim | PASS |
| F7 checklist parity gap | `.agents/WORKER.md:243-247,276-278` requires the independent-verification boundary and who/why explanation | PASS |

## Focused policy checks

Inline bounded Python checker: **34/34 PASS**.

1. AGENTS canonical task-type declaration: PASS
2. WORKER canonical task-type declaration: PASS
3. No `policy`, `discovery`, or `test-author` task-type examples: PASS
4. Exact U+2014 no-allowlist sentinel: PASS
5. ASCII no-allowlist sentinel absent: PASS
6. Unauthorized route stops before claim: PASS
7. Unauthorized route performs no ledger operation before claim: PASS
8. Unauthorized route never silently substitutes: PASS
9. Canonical sentinel branch present: PASS
10. Route permission is recorded in scratchpad: PASS
11. Checklist names independent-verification-boundary: PASS
12. Checklist requires who verifies and why: PASS
13. AGENTS template declares independent-verification boundary: PASS
14. Worker handoff references canonical AGENTS schema: PASS
15. Worker handoff has exactly 12 numbered labels: PASS
16. Worker handoff labels match canonical order: PASS
17. AGENTS completion schema has canonical 12 labels: PASS
18. Worker/AGENTS handoff order parity: PASS
19. Scratchpad path absent from schema fields: PASS
20. `Resource observations` label present: PASS
21. `Resources` label absent from schema: PASS
22. `Status` label present: PASS
23. Hash absence uses `none \u2014 [reason]`: PASS
24. Product-test-only N/A rule present: PASS
25. N/A cannot replace RED evidence: PASS
26. N/A cannot replace frozen-test status/hash: PASS
27. N/A cannot replace contract validation: PASS
28. RED authoring requires compiling RED and frozen hash: PASS
29. Independent implementer/verifier separation: PASS
30. Emergency no-claim stop is explicit: PASS
31. Held-claim stop has legal blocked transition: PASS
32. Frozen tests remain immutable: PASS
33. Landing forbids force-push: PASS
34. Canonical task-type count is exactly five: PASS

## Prompt scenarios

Inline bounded Python scenario checker: **8/8 PASS**.

| Scenario | Result | Evidence |
|---|---|---|
| Complete implementation brief | PASS | `AGENTS.md` template headings and acceptance boundary present |
| RED authoring | PASS | compiling RED, frozen hash, independent verifier requirements present |
| Discovery with product-test N/A | PASS | product-test-only N/A plus validator and captured fixture requirements present |
| Raw incomplete request | PASS | stop before claim/edit requirement present |
| Urgent stop before claim | PASS | `.agents/WORKER.md:295-319` immediate stop, no claim, no ledger mutation/new work |
| Held claim with no legal transition | PASS | exact claim/status recovery rule plus legal blocked transition present |
| Unauthorized route | PASS | `.agents/WORKER.md:348-352` stop/report before claim; no ledger operation before claim |
| Implementer self-verification | PASS | `.agents/WORKER.md:280-293` requires separate verifier |

## Git, scope, and repository gates

- `rtk git diff --check` -> PASS, exit 0.
- Candidate policy diff (`abf28f^..abf28f`) -> `.agents/WORKER.md`, correction scratchpad, and its ledger row only. No product/test/controller/policy-authority path outside the worker policy was changed.
- `rtk git push --dry-run` -> `Everything up-to-date`, exit 0.
- `HEAD` and `origin/docs/structured-subagent-prompts` -> `abf28f47c25989a17f548779fb71feb9f8577d39`; remote containment PASS.
- Branch upstream -> `origin/docs/structured-subagent-prompts`; ordinary push target correct.
- `rtk python3 tools/convergence_gate.py` -> FAIL, `CONVERGENCE BLOCKED`, `total=101`; separate repository blocker, not policy-specific.
- `rtk python3 tools/validate_repository.py` -> FAIL, `validate_backlog_exhaustion: 51 error(s)`; protection fixture passes, hosting-platform state unverified. Separate repository blocker.
- No Cargo, browser, database, stash, secret, destructive, or long-lived process used.

## Verdict and constraints

**ACCEPT policy-specific candidate `abf28f47c25989a17f548779fb71feb9f8577d39`.** All seven defects are closed; focused checks are 34/34 and scenarios 8/8. This is not repository or product acceptance. Integration remains conditional on `python3 tools/validate_repository.py`, repository protection/code-owner review, exact integrated-revision re-verification, and convergence resolution.

## Hashes and unknowns

- Candidate revision: `abf28f47c25989a17f548779fb71feb9f8577d39`.
- Frozen product test: none - policy verification; no product test or frozen artifact applies.
- Unresolved policy gaps: none.
- Repository-level blockers remain as listed above.
