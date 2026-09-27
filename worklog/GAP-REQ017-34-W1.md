# GAP-REQ017-34-W1

## Claim and boundary

- Reclaimed the abandoned `GAP-REQ017-34-W1` claim from stopped session `ses_gap_req017_w1` using `tools/completion_claims.py:118-141`; current session `ses_f1dd6415cffeV8brkRPYhk2ou0` owns the blocked claim.
- Base/candidate start: `f21a01e21a4a82b57dae641c032472585f84b2c4`, branch `reconcile/GAP-REQ017-34-W1`.
- Owned artifact: `sources/req017-extensibility-ownership-gap.json` only, plus this scratchpad and own claim row. No backlog/Ralph/FEATURES/validator/test/other-manifest edits.

## Source evidence and observed state

- At start commit `f21a01e`, `ralph.json:851-865` and `:867-881` mark EXT-001/002 `accepted`, both still `TBD - see source audit` with REQ-017. `ralph.json:2627-2641` marks UI-010 `accepted`; `:3147-3162` marks EXT-013 `accepted` with the safe-extraction story. `ralph.json` is current controller projection, not pinned upstream proof of feature behavior.
- `sources/backlog-exhaustion.json:2662-2678` is stale: summary says 133 accepted / 98 nonaccepted / 231 stories, not current 224 / 34 / 258. Current classifier evidence is `worklog/BACKLOG-CLASSIFIER-34-W1.md:5-20` (224 accepted, 34 nonaccepted, 258-story union); validator checks are in `tools/validate_backlog_exhaustion.py:1405-1417,1459-1487`.
- `sources/req017-extensibility-ownership-gap.json:29-51` held null ownership decisions and legacy in-progress/null-path bindings for EXT-001/002. Current files exist: `tasks/EXT-001.md:11-46`, `tasks/EXT-002.md:11-46`, `worklog/EXT-001.md:11-13,41-45`, `worklog/EXT-002.md:11-13,38-43`. They propose lifecycle versus built-in partitions, but source-level owner resolution remains unestablished; keep `ownershipDecision` null and candidate fragments disqualified.
- The gap source pins OpenCode commit/tree at manifest lines 4-7. Skill/command source/caller/test/spec evidence, command-to-skill design caveat, six unresolved partitions and closure criteria remain unchanged.
- UI-010 has no task/worklog in this candidate; Ralph acceptance alone does not establish a distinct skills/commands owner. Preserve its exclusion and unresolved ownership; project current controller status.
- EXT-013 has explicit implementation evidence: `tasks/EXT-013.md:9-19`, `worklog/EXT-013.md:5-18,20-39`, commit `accb2e919fe3e02ac34aa4b42851ec710f8ca282`. Project accepted controller status; preserve owned partition and provenance.
- Adjacent hook mismatch remains out of scope: `sources/extensibility-remaining-ownership-gap.json:77` records EXT-008 `plugin-hook-contract-mismatch`; its unresolved decomposition and pinned evidence remain intact.
- Baseline `rtk python3 tools/validate_backlog_exhaustion.py`: 93 errors, including `EXT-001/002: REQ-017 ownership gap is stale after Ralph semantics changed`, task/worklog appeared, missing nonaccepted-ledger rows, and stale UI-010/EXT-013 projections. This is a blocked reconciliation candidate, not permission to falsify Ralph or edit the validator.

## Decisions

- Correct manifest projections supported by current source: controller status for EXT-001/002/UI-010/EXT-013; actual task/worklog paths for EXT-001/002.
- Keep EXT-001/002 `ownershipDecision` null. Controller acceptance does not resolve their semantic partition collision. Retain all source evidence, SkillV2/CommandV2 partitions, six unresolved partitions, design caveat and closure criteria.
- Do not edit `sources/backlog-exhaustion.json`, `ralph.json`, `FEATURES.md`, validator, tests or `sources/extensibility-remaining-ownership-gap.json`. Validator's hardcoded in-progress/null-path/row-presence assumptions remain an exact integration contradiction; candidate remains blocked until verifier/controller authority reconciles it.

## Verification

- Manifest JSON + assertions passed: accepted controller statuses; EXT-001/002 current task/worklog paths; null ownership; pinned EXT-013 commit; two reviewed partitions; six unresolved partitions.
- `rtk git diff --check`: PASS.
- Post-edit `rtk python3 tools/validate_backlog_exhaustion.py`: FAIL, 95 errors. Exact remaining REQ-017 contradictions: `EXT-001/002: REQ-017 ownership gap is stale after Ralph semantics changed`, `EXT-001/002: REQ-017 task binding drifted`, appeared task/worklog, missing exhaustion rows; `UI-010: REQ-017 dependency-constrained exclusion drifted`; `EXT-013: REQ-017 implemented exclusion drifted`. Validator requires nonaccepted/in-progress, null paths and unresolved rows while Ralph records accepted and local task/worklog evidence exists for EXT-001/002/013.
- No product tests authored or modified; no acceptance claimed.

## Blocker

Validator/controller contradiction; edits to its code/config, Ralph and backlog ledger are outside this task. Preserve candidate blocked and hand exact reproduction to integrator.
