# GUARD-EXT-001 reconciliation audit

Claim: EXT-001, session ses_f3215f2e6ffe6Lw6TDiPweG8X8, scratchpad worklog/GUARD-EXT-001.md.
Worktree: guard-EXT-001 (branch lane/GUARD-EXT-001-20260923, HEAD 06ed486). Audit only; no canonical/product/test/validator edits.

## Source evidence (guard worktree paths)
- ralph.json EXT-001 (lines 851-865): status accepted, requirementIds [REQ-017], userStory "TBD - see source audit", testObligations EXT-001-T01..T05.
- tasks/EXT-001.md: EXISTS (10.0K, product card, REQ-017, native add/remove/wait contract, crates/ext boundary).
- worklog/EXT-001.md: EXISTS (5.7K, claims GREEN 5/5 plugin_lifecycle, notes crates/ext drift + duplicate module + verifier acceptance external).
- sources/req017-extensibility-ownership-gap.json: status source-reviewed-no-exact-task-owner; residualStoryIds [EXT-001, EXT-002]; ownershipDecision EXT-001 null EXT-002 null; taskBindingState EXT-001 {controllerStatus in-progress, ralphStory "TBD - see source audit", requirementIds [REQ-017], taskCard null, worklog null}; unresolvedPartitions 6; closureCriteria require task-specific distinguishing evidence; commit/tree bound to upstream.lock opencode 95daf90.
- sources/backlog-exhaustion.json EXT-001 story row: category unresolved-decomposition, controllerStatus in-progress, reasonKey extensibility-family-not-decomposed, taskCard null, worklog null, implementationCommits [], reopenPolicy source-grounded-task-decomposition-required.
- sources/extensibility-remaining-ownership-gap.json: EXT-001 excluded as guarded-by-REQ-017-extensibility-gap (not a residual owner).
- FEATURES.md: EXT-001 accepted placeholder rows (line 46 table, 149 REQ-017 map, 292 list, 614 topology). ralph.completion.json contract: legacyAcceptedIsReleaseEvidence false.
- tools/validate_backlog_exhaustion.py req017_extensibility_gap_errors (lines 1347-1506+): expects ralph status in-progress + TBD + [REQ-017] (line 1405), binding taskCard/worklog null (1408-1415), no on-disk tasks/<id>.md or worklog/<id>.md (1416-1417), exhaustion row unresolved-decomposition/extensibility-family-not-decomposed with null/null/[] (1452-1455).

## Observed validator output (read-only, guard worktree)
- python3 tools/validate_backlog_exhaustion.py: 51 errors total.
- Exactly two error lines attributable to EXT-001:
  - `EXT-001: REQ-017 ownership gap is stale after Ralph semantics changed` (ralph status accepted vs expected in-progress).
  - `EXT-001: task/worklog appeared; REQ-017 ownership gap needs deliberate review` (tasks/EXT-001.md + worklog/EXT-001.md exist vs recorded null/null).
- Adjacent EXT-002 pair lines exist but out of scope. Repo-wide accepted-classification/summary lines are shared pre-existing conditions, not EXT-001-owned.

## Existing implementation/test evidence (distinguished from ownership/acceptance)
- crates/tools/src/plugin_lifecycle.rs EXISTS; crates/tools/tests/plugin_lifecycle.rs EXISTS; crates/ext DOES NOT EXIST (task-card path/command drift).
- worklog/EXT-001.md asserts 5/5 GREEN + RED probe sensitivity, but ownershipDecision remains null, gap taskBindingState records null/null, exhaustion row records null/null/[]. On-disk files prove existence, not task-specific ownership: EXT-001/EXT-002 share identical surface signature [opencode.extensibility], and closure criteria forbid ownership by surface arithmetic, task order, or residual membership.
- Legacy ralph accepted + FEATURES.md accepted rows are explicitly not release evidence per completion contract.

## Disposition: BLOCKED (source-grounded)
- Ownership null by checked-in source review; six REQ-017 unresolved partitions open; skill/command candidate fragments disqualified (ownershipEstablished false); subtracted owners EXT-013/TOOL-007/UI-010 stay excluded.
- No authority patch exists inside guard bounds: flipping ralph.json status, recording task/worklog bindings, reconciling gap + backlog-exhaustion + FEATURES.md, or decomposing EXT-001 vs EXT-002 requires controller-authored source-grounded decomposition under integration authority. Product/test/validator edits forbidden.

## Authority patch fields
- None. Lawful change requires controller-authored task-specific binding evidence distinguishing EXT-001 from EXT-002 (inputs/outputs/failure/state-resource lifetime/authority per closure criteria), then deliberate reconciliation of sources/req017-extensibility-ownership-gap.json + sources/backlog-exhaustion.json + ralph.json under integration authority.

## Unresolved permission/network/runtime/resource blockers (preserved)
- External JS/TS plugin runtime (npm/package-install/network authority, module validation/failure policy, cache/update lifetime) outside authorized native lane.
- Reload/watch + deferred external activation remain design work per spec; no runtime authority granted.
- Verifier/controller acceptance external; not claimed here.

## Verification (read-only, no Cargo/network/heavy)
- python3 tools/validate_backlog_exhaustion.py (51 errors; 2 lines attributable to EXT-001 as above)
- ls tasks/EXT-001.md worklog/EXT-001.md crates/tools/src/plugin_lifecycle.rs (exist); ls crates/ext (absent)
- git status/branch/log for worktree identity
