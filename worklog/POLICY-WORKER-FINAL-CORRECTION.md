# POLICY-WORKER-FINAL-CORRECTION

## Claim and scope

- Task: `POLICY-WORKER-FINAL-CORRECTION`
- Task type: `implementation`
- Role: worker-governance policy correction author
- Session: `ses_f2d75f8caffaXR9D6z5ygIymS5`
- Model/route: `vyce/deepseek-v4.1` / `vyce-deepseek-v41`; allowlist permission confirmed
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/structured-subagent-prompts`
- Branch/ref: `docs/structured-subagent-prompts`
- Owned file: `.agents/WORKER.md`
- Scratchpad: `worklog/POLICY-WORKER-FINAL-CORRECTION.md`
- Non-goals: no AGENTS edit, product/test/controller/plan edit, branch config, stash, or main merge

## Source evidence

- `AGENTS.md` canonical 12-field handoff schema: `AGENTS.md:235-250`
- Canonical task types: `AGENTS.md:88-93` - implementation, RED authoring, research, integration, verification
- Canonical `N/A` semantics: `AGENTS.md:38-45` - `N/A` only for product-test-only, exact `N/A — no user allowlist` sentinel
- Final verifier's seven defects: `worklog/POLICY-STRUCTURED-DELEGATION-FINAL-VERIFY.md:a575f89bb5ab9e122f33ece76d82f36660514e40` lines 56-64
- `.agents/WORKER.md` current text lines 349-381 (handoff schema), 356-357 (task types), 342 (N/A sentinel), 339-342 (preclaim route), 235-269 (checklist)

## Seven defects to correct

1. **Worker handoff order/labels drift** - `.agents/WORKER.md:349-379` does not match canonical 12-field order. Fix: reorder to canonical order, rename `Resources` to `Resource observations`.
2. **Extra handoff field** - `.agents/WORKER.md:380-381` adds `Scratchpad path`. Fix: remove it.
3. **Invalid task-type examples** - `.agents/WORKER.md:356-357` names `policy`, `discovery`, `test-author`. Fix: only canonical five types.
4. **Canonical no-allowlist sentinel** - `.agents/WORKER.md:342` lacks exact `N/A — no user allowlist`. Fix: use canonical em-dash sentinel.
5. **Conflicting ASCII sentinel** - `.agents/WORKER.md:342` uses `N/A -- no user allowlist`. Fix: replace with canonical `N/A — no user allowlist`.
6. **Illegal preclaim route response** - `.agents/WORKER.md:339-342` says set blocked preclaim. Fix: stop/report before claim, no ledger operation without claim.
7. **Checklist parity gap** - `.agents/WORKER.md:235-269` omits independent-verification-boundary field. Fix: add it.

## 33-check matrix

| # | Check | Expected | Source |
|---|---|---|---|
| 1 | Canonical task types only | 5 types: implementation, RED authoring, research, integration, verification | AGENTS.md:88-93 |
| 2 | No valid task-type examples outside 5 | No policy/discovery/test-author | WORKER.md:356-357 |
| 3 | Canonical `N/A — no user allowlist` sentinel | Exact em-dash form | AGENTS.md:42,97 |
| 4 | No ASCII `N/A -- no user allowlist` | Zero occurrences | WORKER.md:342 |
| 5 | Unauthorized route: stop before claim | No ledger mutation without claim | AGENTS.md:25-36,99-100 |
| 6 | Preclaim route check: no blocked update | Stop and report, not blocked status | WORKER.md:339-342 |
| 7 | Handoff schema: exactly 12 fields | 12 fields, no more | AGENTS.md:235-250 |
| 8 | Handoff: no `Scratchpad path` field | Removed | WORKER.md:380-381 |
| 9 | Handoff field order matches canonical | Same order as AGENTS.md | AGENTS.md:235-250 |
| 10 | `Resource observations` not `Resources` | Renamed | AGENTS.md:249 |
| 11 | Checklist: independent-verification-boundary | Explicit field | AGENTS.md:130-133 |
| 12 | Checklist: all canonical fields present | Parity with AGENTS.md | AGENTS.md template |
| 13-33 | ... (additional scenario/policies) | See scenario matrix below | |

## Scenario matrix (8 scenarios)

| Scenario | Expected | Source |
|---|---|---|
| 1 | Complete implementation brief | Intake + canonical handoff |
| 2 | RED authoring | Compiling RED, frozen hash, independent verifier |
| 3 | Discovery with product-test N/A | Executable validator + captured failing fixture |
| 4 | Raw incomplete request | Stop before claim/edit |
| 5 | Urgent stop before claim | Immediate stop, no claim held, no ledger mutation |
| 6 | Held claim with no legal transition | Safe existing API only; report exact recovery state |
| 7 | Unauthorized route | Stop and report mismatch before claim, no ledger operation |
| 8 | Implementer self-verification | Reject: separate implementer/verifier required |

## Decisions

- Reorder handoff schema to exact canonical 12 fields (AGENTS.md:235-250).
- Remove Scratchpad path field from handoff schema; scratchpad obligations remain in sections 1-4.
- Restrict task-type examples to the five canonical types only.
- Replace ASCII `N/A -- no user allowlist` with canonical `N/A — no user allowlist` (em-dash).
- Fix preclaim route validation: unauthorized route stops and reports before claim, no ledger mutation.
- Add independent-verification-boundary field to intake checklist (section 8).
- Replace generic N/A wording in sections 35-36 and 144-148 with canonical product-test-only rule and `none — [reason]` for non-test absent values.

## Unknowns

- none
