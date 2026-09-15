# REL-SLICE-01: promote 12 inbuilt-agent-tool slices to canonical backlog (PROPOSAL)

Status: PROPOSAL ONLY. Not applied. Workers must NOT hand-edit `ralph.json`,
`FEATURES.md`, `requirements/`, `sources/`, `tools/`, `PLAN.md`. Only the
integration owner applies this, via the commands in section 5, in order.

## 1. Proposed REQ-039

```json
{
  "id": "REQ-039",
  "requirement": "Inbuilt agent tools default-on with opt-out: repowise index/query/risk, RTK token-optimized commands, terse telegraphic style, telemetry collect/report",
  "tasks": ["TOOL-016","TOOL-017","TOOL-018","TOOL-019","TOOL-020","TOOL-021","TOOL-022","TOOL-023","REL-005","REL-006","REL-007","REL-008"],
  "mandatory": true
}
```

Shape follows existing entries (`requirements/user-requirements.json:396-414`).
`validate_plan.py:90-91` fails any non-mandatory seeded requirement; REQ-039 is `mandatory:true`.
`validate_plan.py:92-94` fails unknown tasks, so REQ-039 lands in the same owner commit as the 12 stories.

## 2. Story-ID table (12)

Prefix rank from `tools/plan_model.py:27-46` (`TOOL`=2, `REL`=6). Obligation
format from `tools/validate_plan.py:57-58` (`{tid}-Txx`, globally unique per `:60-62`).

| # | Story | Rank | Source spec | Test obligations |
|---|-------|------|-------------|------------------|
| 1 | TOOL-016 | 2 | RW-SLICE-01-index-spec.md | TOOL-016-T01..T05 |
| 2 | TOOL-017 | 2 | RW-SLICE-02-query-spec.md | TOOL-017-T01..T05 |
| 3 | TOOL-018 | 2 | RW-SLICE-03-risk-spec.md | TOOL-018-T01..T05 |
| 4 | TOOL-019 | 2 | RTK-SLICE-01-core-spec.md | TOOL-019-T01..T05 |
| 5 | TOOL-020 | 2 | RTK-SLICE-02-test-spec.md | TOOL-020-T01..T05 |
| 6 | TOOL-021 | 2 | RTK-SLICE-03-pass-spec.md | TOOL-021-T01..T05 |
| 7 | TOOL-022 | 2 | CV-SLICE-01-render-spec.md | TOOL-022-T01..T05 |
| 8 | TOOL-023 | 2 | CV-SLICE-02-clarity-spec.md | TOOL-023-T01..T05 |
| 9 | REL-005 | 6 | HR-SLICE-01-collect-spec.md | REL-005-T01..T05 |
| 10 | REL-006 | 6 | HR-SLICE-02-report-spec.md | REL-006-T01..T05 |
| 11 | REL-007 | 6 | SET-SLICE-01-optout-spec.md | REL-007-T01..T05 |
| 12 | REL-008 | 6 | research consolidation (RW/RTK/CV/HR RES) | REL-008-T01..T03 |

11 stories carry T01..T05 plus REL-008 carries T01..T03 = 58 obligation
IDs total (matches `ralph-snippet-REQ-039.json` count modulo the
REL-007/REL-008 role swap: snippet files REL-007 as HR-RES research and
REL-008 as SET opt-out; canonical per `tasks/REL-007.md` + `tasks/REL-008.md`
is REL-007 = SET opt-out T01..T05, REL-008 = research consolidation
T01..T03).

Rank justification: TOOL prefix = build-time agent tooling, no release gate
dependency, so rank 2 alongside TOOL-006/TOOL-007. REL prefix = release/
assurance surface (telemetry evidence, default-on policy, gate), so rank 6
alongside REL-001..004. Milestone gating synthesizes rank-order deps
(`plan_model.py:194-205`); REL stories stay out of the ready queue until
ranks 0-5 accept. Research slice (REL-008) keeps Kind: research, product
lanes keep Kind: product.

## 3. Category placement (`tools/validate_backlog_exhaustion.py:173-195`)

`build_expected_ledger` raises `ValueError` for any non-accepted story missing
from `CATEGORY_IDS` (`:351-352`), and `validate_ledger` fails category drift
(`:2505-2510`) plus missing/non-accepted coverage (`:2491-2498`). Owner adds:

- `dependency-constrained`: none (no client-architecture dep).
- `unresolved-decomposition`: TOOL-016..023, REL-005..008 (source-grounded task
  decomposition required; matches existing REL-001..003 placement at `:188-194`).
- `REASON_BY_ID` (`:241-250`): TOOL-* -> `extensibility-family-not-decomposed`
  is wrong family; owner adds explicit keys, e.g. TOOL-* ->
  `tool-family-not-decomposed`, REL-* -> `release-assurance-not-product-owner`.
  Any new reason key must also satisfy `_reason_policy` (`:264-271`) via the
  `unresolved-decomposition` branch (no validator edit beyond the two maps).
- No story may duplicate a category (`:336-338`).

## 4. Validator impact checklist

- [ ] plan_model (`tools/plan_model.py:118-124`): TOOL/REL prefixes known, no
      `PHASE_OVERRIDE` needed, no `Unknown task prefix` error.
- [ ] validate_plan obligations (`tools/validate_plan.py:47-63`): all 58 new
      obligation IDs unique, prefixed, no duplicates (11xT01..T05 + REL-008 T01..T03).
- [ ] validate_plan requirements (`:84-95`): REQ-039 mandatory, all 12 tasks exist.
- [ ] backlog exhaustion (`validate_backlog_exhaustion.py:2471-2510`): 12 new
      rows classified, ledger regenerated, no missing/extras/drift.
- [ ] FEATURES mirror (`:400-413`, `sync_features_status :2450-2468`): every new
      story has a status occurrence; owner syncs via `--sync-features`, never by hand.
- [ ] ledger collision (`validate_plan.py:66-81`): new `suggestedModule` paths unique.
- [ ] CODEOWNERS/protection unchanged: this proposal adds no protected-path
      files; `docs/REPOSITORY_PROTECTION.md` semantics untouched;
      `platformState.verified` stays false.
- [ ] canonical gate: `tools/validate_repository.py:21-28` chain
      (ruleset-import, readback, protection, exhaustion, DISC-003, plan) green.

## 5. Ordered owner steps (exact commands)

1. Append 12 stories to `ralph.json` (`status: not-started`, empty
   `dependencyIds`, obligations per table) and REQ-039 to
   `requirements/user-requirements.json` in one commit with 12 new
   `tasks/{ID}.md` cards (format per `tasks/TOOL-006.md:1-7`, `tasks/REL-004.md:1-7`).
2. Extend `CATEGORY_IDS` + `REASON_BY_ID` in
   `tools/validate_backlog_exhaustion.py` (section 3). Reviewed owner-only edit.
3. `/usr/bin/python3 tools/validate_backlog_exhaustion.py --write`
4. `/usr/bin/python3 tools/validate_backlog_exhaustion.py --sync-features`
5. `/usr/bin/python3 tools/validate_plan.py`
6. `/usr/bin/python3 tools/validate_repository.py` (must print OK line, `:112`).
7. Open PR; `planning` CI job is the required check
   (`tools/validate_repository.py:56-70`).

## 6. Rollback

Revert the owner commit(s) in reverse order (steps 2,1), then re-run steps 3,4,6.
Checked-in ledger and FEATURES.md are regenerated artifacts: never hand-patch
them to fake green; a stale ledger fails `validate_ledger` field comparison
(`:2528-2535`) by design.
