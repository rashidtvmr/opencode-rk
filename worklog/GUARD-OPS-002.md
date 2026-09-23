# GUARD-OPS-002 reconciliation audit — 2026-09-23

## Claim
- Ledger `tasks/completion/claims.json:388-389`: GUARD-OPS-002 in-progress, session ses_f32482b65ffd8oU3ZqjMISssge, scratchpad worklog/GUARD-OPS-002.md.
- Owned files only: this scratchpad + ledger row. No product/canonical edits.
- HEAD audited: c8f6dcb39d98302f27f04a25897d3880e09e2c1b.

## Source evidence (exact)
- Card: `tasks/OPS-002.md:1-155` — status NOT STARTED, kind product, runtime-optional False, mandatory yes; requirementIds [] ; test obligations OPS-002-T01..T05; contract `normalize_ref(raw, branch, cache_root) -> Result<NormalizedRef, RefError>` with `RefError = Empty|UnsupportedForm|UnsafeBranch|BranchRequired|InputTooLong`, MAX_REF_BYTES=4096, env var shorthand-only, non-ownership of clone/fetch/locks/transport (lines 48-80, 95-103, 106-144); suggested boundary `crates/ops/src/repo_ref.rs`, fallback `crates/foundation/src/repo_ref.rs` (line 71).
- Gap: `sources/operations-ownership-gap.json` partition `repository-reference-normalization` (evidence `packages/core/src/repository.ts:57-137` blob 8ee5be6, test `packages/core/test/repository.test.ts:7-70` blob 1a6af67, caller `packages/core/src/reference.ts:75-103` blob 1e1ab9d, spec `specs/v2/config.md:73-84` blob 96b0f3a); ownershipBlocker: same family maps to OPS-002/003/004/006/008, no task/path evidence chooses one; sideEffects `github-default-remote-may-read-process-environment`; unresolved partition `repository-reference-normalization-and-cache-identity` (validator OPERATIONS_UNRESOLVED_PARTITIONS); storyIds include OPS-002 (tools/validate_backlog_exhaustion.py:47-57).
- Ledger: `sources/backlog-exhaustion.json` OPS-002 entry: category unresolved-decomposition, controllerStatus not-started, taskCard null, taskStatus null, worklog null, surfaceIds [opencode.repository-operations] (entry dump this session).
- Ralph: `ralph.json` OPS-002: status accepted, requirementIds [], userStory generic DISC-002 line, testObligations OPS-002-T01..T05.
- FEATURES: `FEATURES.md:65` row 23 OPS-002 accepted gate b quiesced-tree re-run; `:647` OPS table requirements `-` accepted generic; `:893` accepted bullet generic. No contract detail.
- Lane impl: `crates/foundation/src/repo_ref.rs:1-301` (OPS-002 header, MAX_REF_BYTES=4096:19, RefError 5 variants:49-55, normalize_ref:78-134, shorthand env OPS002_GITHUB_HOST:27,205) — matches card contract shape.
- Lane tests: `crates/foundation/tests/repo_ref.rs` `#[path="../src/repo_ref.rs"]`, T01..T05 (t01 happy matrix head verified).
- Lane worklog: `worklog/OPS-002.md:1-34` — GREEN 5/5, src sha 63c45dea, tests sha e00adb5b, no code change, purity claim.
- Nearby duplicate (NOT OPS-002): `crates/foundation/src/ops_repo_ref.rs:1-254` header says OPS-008, MAX_INPUT_BYTES=2048:12, RepoError Malformed|UnsafeBranch|InputTooLong only:41-48, no env read, canonical includes @branch:131; tests `crates/foundation/tests/ops_repo_ref.rs` OPS-008 T01..T05. Distinct slice; not to be conflated.
- Wiring: `crates/foundation/src/lib.rs:19,27-28` has `pub mod ops_repo_ref; pub mod repo_ref; pub mod repo_ref_ext;` — shared file, pre-existing, guard must not touch.
- Validators: `tools/validate_backlog_exhaustion.py` operations section lines 976-1080: story set/storyIds, ownershipDecision all-None, unresolvedPartitions exact, lock binding, taskBindingState exact + taskCard/worklog must be None + tasks/<id>.md or worklog/<id>.md existing = error, ralph status must be not-started + generic story ("OPS-002: operations ownership-gap is stale after Ralph task semantics changed" / "task/worklog appeared ... needs deliberate review"); repo reports validate_repository FAIL backlog exhaustion exit=1, validate_backlog_exhaustion 51 errors incl both OPS-002 lines + accepted/unknown classification list containing OPS-002.
- Gate total: lane_gate.py LANES list is storage-v2 only (12 lanes, no OPS lanes); "Gate total=80" not present in repo — treat as orchestrator-side count, not verifiable here. No cargo/network/heavy work run per orders.

## Observed scenario
- OPS-002 card/worklog/impl/tests are mutually consistent on the pure normalization fragment and respect non-ownership (no clone/fetch/lock/network in repo_ref.rs).
- Canonical authority records still say unowned: gap ownershipDecision OPS-002 None, ledger taskCard/taskStatus/worklog null + controllerStatus not-started, while ralph.json says accepted (generic, requirementIds [] matches card) and tasks/OPS-002.md + worklog/OPS-002.md + impl exist on disk.
- Validator therefore fires exactly the two expected OPS-002 errors (stale semantics + task/worklog appeared). These are deliberate-review signals, not regressions. The broader 51-error FAIL (incl accepted/unknown classification of OPS-002) is controller/ledger scope, out of guard authority.
- FEATURES rows are generic discovery lines; they neither grant nor deny the OPS-002 fragment. No acceptance implied.
- Blockers preserved: gap ownershipBlocker (family shared OPS-002/003/004/006/008); card non-ownership (materialization/transport/locks/INT-002/OPS-007/OPS-009/BASE owners); ledger reopenPolicy source-grounded-task-decomposition-required; validator task/worklog-appeared deliberate-review error.

## Target boundary
- Guard owns only worklog/GUARD-OPS-002.md + its ledger row. No edits to tasks/OPS-002.md, ralph.json, FEATURES.md, sources/*.json, tools/*, crates/**.
- Commit/push blocked per task orders (tree already has unrelated `M tasks/completion/claims.json` from claim write; left for orchestrator to land).

## Tests
- None run (No Cargo/network/heavy work per orders). Evidence is read-only: file:line citations above + `python3 tools/validate_backlog_exhaustion.py` grep output (51 errors, both OPS-002 lines) + `git rev-parse HEAD` c8f6dcb.

## Decisions
- Verdict: lane artifact self-consistent; authority records still unowned/blocked. No acceptance, no canonical edits. Patch proposal below is controller-authority only.

## Authority patch proposal (controller + verifier only, NOT applied)
1. `sources/operations-ownership-gap.json` (deliberate review, source-grounded): EITHER (a) keep ownershipDecision.OPS-002=None and record tasks/OPS-002.md + worklog/OPS-002.md + crates/foundation/src/repo_ref.rs + tests as candidate evidence under review (no ownership change), OR (b) assign ownership to exactly one of OPS-002/003/004/006/008 for partition `repository-reference-normalization` with per-story path evidence, update taskBindingState (taskCard/worklog paths), storySurfaceSignatures, equivalenceGroups, unresolvedPartitions accordingly. Do not do (b) by subtraction; OPS-003/006 equivalence-group siblings need explicit disposition.
2. `sources/backlog-exhaustion.json` OPS-002 entry: set taskCard/worklog/localEvidencePaths/implementationCommits to the reviewed paths only after (1) is decided; keep category unresolved-decomposition until controller accepts; keep reopenPolicy.
3. `ralph.json` / `FEATURES.md` OPS-002: no guard change. If verifier accepts the lane, controller flips status/userStory/requirementIds deliberately; until then the stale-semantics validator error correctly persists.
4. Deconfliction note for reviewer: `crates/foundation/src/ops_repo_ref.rs` (OPS-008 header, 2048 cap, no env) vs `crates/foundation/src/repo_ref.rs` (OPS-002, 4096 cap, OPS002_GITHUB_HOST shorthand-only) are distinct contracts; any future OPS-008 claim must cite the former, never borrow OPS-002 evidence.
5. Re-run to prove: `python3 tools/validate_repository.py` (expect FAIL until controller acts), `python3 tools/validate_backlog_exhaustion.py | grep OPS-002`, `git status --short -- tasks/OPS-002.md`, plus the card Verification block test commands (not run by guard).

## Remaining unknowns
- Gate total=80 definition lives orchestrator-side; not verifiable in-repo.
- Whether controller intends OPS-002 vs sibling ownership of the shared family; guard makes no recommendation beyond the two options.
- Verifier acceptance of worklog/OPS-002.md GREEN claim is separate; guard did not re-run tests.
