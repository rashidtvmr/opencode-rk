# GUARD-OPS-006 -- Audit: Offline Repository-Reference Normalization and Cache Identity

## Claim
- Task ID: GUARD-OPS-006
- Session: ses_guard_ops006_audit
- Status (ledger): in-progress (will set to blocked below)
- Scratchpad: worklog/GUARD-OPS-006.md
- Owned files: ONLY this scratchpad + tasks/completion/claims.json (ledger row update)

## Source Evidence (exact paths/lines/symbols)

### ralph.json (plan; canonical extended plan, forced not-started by loader)
- `ralph.json:1245-1256`: OPS-006 entry. `"status": "accepted"`, `"requirementIds": []`,
  `"userStory": "Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json"`,
  `"testObligations": [OPS-006-T01, OPS-006-T02, OPS-006-T03, OPS-006-T04, OPS-006-T05]`.

### tasks/OPS-006.md (task card)
- `tasks/OPS-006.md:3-4`: Status: NOT STARTED. Kind: product. Runtime optional: True.
- `tasks/OPS-006.md:5-7`: Requirements: none (discovered scope; repository-only group OPS-002/003/006 identical per sources/operations-ownership-gap.json; no task/path evidence selects one).
- `tasks/OPS-006.md:9`: Test obligations: OPS-006-T01..T05.
- `tasks/OPS-006.md:16-19`: Candidate fragment `repository-reference-normalization` (strongest strict-TDD-friendly fragment per gap file); cache materialization, git transport, locks and frozen INT-002/OPS-007/OPS-009 overlaps explicitly out of scope.
- `tasks/OPS-006.md:51-68`: Observable contract: `normalize_reference(raw, branch, cache_root) -> Result<NormalizedRef, RefError>`; default branch `main`; `cache_path = cache_root.join(canonical).join(branch)`; `cache_id` = hex hash of `canonical + '\0' + branch`.
- `tasks/OPS-006.md:70-79`: Failure states: Invalid, UnsafeBranch, BadCacheRoot.
- `tasks/OPS-006.md:81-89`: Resource bounds: raw<=256, branch<=128, cache_root<=1024; canonical<=256, cache_path<=2048, cache_id fixed 64 hex.
- `tasks/OPS-006.md:91-109`: Frozen test obligations T01-T05.
- `tasks/OPS-006.md:123-128`: Verification: `git status --short -- tasks/OPS-004.md tasks/OPS-005.md tasks/OPS-006.md` and `test -f tasks/OPS-004.md && test -f tasks/OPS-005.md && test -f tasks/OPS-006.md`.

### sources/operations-ownership-gap.json
- `sources/operations-ownership-gap.json:34-36`: OPS-006 surfaceSignature `[opencode.repository-operations]`.
- `sources/operations-ownership-gap.json:53-62`: equivalenceGroups: `repository-only` group = [OPS-002, OPS-003, OPS-006], all with signature `opencode.repository-operations`.
- `sources/operations-ownership-gap.json:76-83`: `ownershipDecision`: ALL OPS stories null, including OPS-006.
- `sources/operations-ownership-gap.json:121-126`: `taskBindingState.OPS-006`: ralphStory "Discovered during DISC-002", requirementIds [], taskCard null, worklog null.
- `sources/operations-ownership-gap.json:368-414`: Reviewed partition `repository-reference-normalization`: behavior "Repository references normalize supported forms, reject unsafe branches and derive branch-isolated cache paths/identities before materialization"; sideEffects include `github-default-remote-may-read-process-environment` (excluded here: caller passes resolved remote explicitly); ownershipBlocker "strongest small strict-TDD-friendly fragment, but repository-operations maps the same source family to OPS-002/003/004/006/008 and no task-specific source binding chooses one."
- `sources/operations-ownership-gap.json:474-487`: candidateFragment `repository-reference-normalization`: ownershipEstablished false, disqualifiers: "OPS-002/003/006 have identical repository-only surface signatures", "OPS-004/008 also include this surface and are mutually indistinguishable", "No task card, worklog, requirement or pinned task/path specification selects this fragment for one OPS id."

### sources/backlog-exhaustion.json
- `.stories[31]`: OPS-006 entry. category: `"unresolved-decomposition"`, reasonKey: `"operations-family-not-decomposed"`, controllerStatus: `"not-started"`, taskCard: null, taskStatus: null, worklog: null. evidenceIds: OC-REPOSITORY-RUNTIME, OC-REPOSITORY-CACHE, OC-REFERENCE-RUNTIME, OC-REPOSITORY-TEST, OC-REPOSITORY-CACHE-TEST, OC-CONFIG-REFERENCE-SPEC. implementationCommits: []. localEvidencePaths: [].

### tools/validate_backlog_exhaustion.py
- line 50: `OPERATIONS_GAP_STORIES = ("OPS-001", "OPS-002", "OPS-003", "OPS-004", "OPS-005", "OPS-006", "OPS-008")`.
- lines 52-54: `OPERATIONS_EQUIVALENCE_GROUPS`: `("repository-only", ("OPS-002", "OPS-003", "OPS-006"), ("opencode.repository-operations",))`.
- lines 191-195: CATEGORY_IDS `"unresolved-decomposition"` includes OPS-006.
- lines 1014-1019: expected_requirements OPS-006 = [].
- line 1032: checks `story.get("status") != "not-started"` -> error "operations ownership-gap is stale after Ralph semantics changed".
- line 1046-1047: checks `(root / "tasks" / f"{story_id}.md").exists()` -> error "task/worklog appeared; operations ownership gap needs deliberate review".
- line 1031: expected_story = "Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json" for OPS-006 (generic_story).

### FEATURES.md (acceptance sync)
- line 9: "## Acceptance sync - controller wave b60ceda (2026-09-16)"
- line 69: `| 27 | OPS-006 | accepted | I18 s7 r27, gate b | quiesced-tree re-run formality |`.
- line 651: `| OPS-006 | - | accepted | Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json |` (acceptance table).
- line 897: `- OPS-006 (accepted): Discovered during DISC-002 surface extraction; scope described by behavior-surface-rules.json` (narrative list).
- Lines 11-12: "Controller-authorized status sync (sole-writer lease: FEATURES.md only). All 82 remaining non-accepted stories flip to accepted; 176 already..."

### ralph.completion.json
- line 10: `.auditShards[17]`: AUD-018, "Resources, packaging, platform and release evidence", legacyPrefixes ["OPS", "REL"], local [config/resource-targets.json, Cargo.toml, tools/check_release_tdd.py].

### worklog/DISC-003.md
- line 270: "Operations: OPS-001, OPS-002, OPS-003, OPS-004, OPS-005, OPS-006, OPS-008. None has a task card. Configuration evidence is mixed across OC-CONFIG-RUNTIME (packages/core/src/config.ts, blob c76486968b0784d57dab102e2d51a7e18f57b7f5), OC-STATE-RUNTIME (packages/core/src/state.ts, blob ab3457fc18143c10d830979dbde3a9375e194ccf), and OC-LOCATION-SERVICES (packages/core/src/location-services.ts, blob 7da67673c31982abafc856f34fb52a2c82891525), while repository-operations additionally mixes Git/cache/observability/install/container behavior. Accepted BASE configuration/lifecycle semantics must remain closed, and OPS-004/008 also overlap repository-operations, so numeric uniqueness or path membership is not sufficient ownership proof."

### worklog/OPS-006.md
- Existing worklog (5 entries): GREEN 5/5 tests passing, offline pure normalize, no code change (rustfmt-only diffs), frozen hashes recorded, "Verifier acceptance separate."

### crates/foundation/src/repo_ref_ext.rs
- line 1-2: `//! OPS-006: offline repository-reference normalization plus branch-isolated` / `//! Contract (tasks/OPS-006.md):`
- This file exists in the worktree but is NOT owned by GUARD-OPS-006 (it is a product implementation file from a prior lane).

## Observed Scenario / Failure Trace

### Failure 1: ralph.json status discrepancy
- ralph.json line 1247: OPS-006 `"status": "accepted"`.
- validate_backlog_exhaustion.py line 1032: expects `"not-started"` for operations-ownership-gap stories. The plan loader forces story statuses to "not-started"; the ledger is the progress record.
- backlog-exhaustion.json: `.stories[31].controllerStatus` = `"not-started"`.
- DISCREPANCY: ralph.json says "accepted" but the validator and the ownership-gap analysis expect "not-started". The FEATURES.md acceptance sync row (line 69) independently says "accepted" -- so ralph.json and FEATURES.md agree with each other, but both conflict with the backlog-exhaustion validator expectation.

### Failure 2: FEATURES.md acceptance row contradicts ownership-gap status
- FEATURES.md line 69: OPS-006 `accepted` with caveat "quiesced-tree re-run formality", gate b.
- sources/operations-ownership-gap.json: `ownershipDecision.OPS-006 = null` (no owner assigned).
- tasks/OPS-006.md line 3: Status: NOT STARTED.
- backlog-exhaustion.json: `controllerStatus: "not-started"`, `taskCard: null`, `taskStatus: null`, `worklog: null`.
- worklog/DISC-003.md line 270: Lists OPS-006 among 34 residual unresolved/decomposition-blocked rows, "None has a task card."
- DISCREPANCY: FEATURES.md marks OPS-006 accepted but the entire ownership-gap analysis, backlog-exhaustion ledger, DISC-003 reconciliation worklog, and the task card itself all agree OPS-006 is NOT STARTED / unresolved / no owner.

### Failure 3: operations-ownership-gap.json ownershipDecision.OPS-006 = null
- The candidate fragment `repository-reference-normalization` has `ownershipEstablished: false` with disqualifiers: identical signatures across OPS-002/003/006 and OPS-004/008 overlap; no task card, worklog, requirement or pinned task/path specification selects this fragment for one OPS id.
- The task card `tasks/OPS-006.md` exists but is insufficient per the gap analysis: no source-to-task decomposition resolves the ambiguity across the OPS family.
- `ownershipDecision.OPS-006` remains `null` -- no authority disposition has been made.

### Failure 4: validate_backlog_exhaustion.py detects OPS-006 as stale/appeared
- Line 1032-1033: ralph.json status "accepted" != "not-started" -> "operations ownership-gap is stale after Ralph semantics changed"
- Line 1046-1047: `tasks/OPS-006.md` exists -> "task/worklog appeared; operations ownership gap needs deliberate review"
- Line 1042-1044: gap `taskBindingState.OPS-006.taskCard is None` is expected; if the gap file is updated to point at the task card, this check would fire.

## Convergence Gate State
- `python3 tools/convergence_gate.py`: total=80 (all pre-existing "completed off-plan task" ledger entries; unrelated to OPS-006 audit).
- `python3 tools/validate_repository.py`: FAIL backlog exhaustion (pre-existing, all OPS stories show "accepted" in ralph.json vs expected "not-started").
- `python3 -m unittest tests.bootstrap.test_disc003_reconciliation -v`: OK (8 tests, all pass).

## Target Boundary (Audit Only)
- This audit lane owns ONLY: worklog/GUARD-OPS-006.md (this scratchpad) and the single ledger row `GUARD-OPS-006` in tasks/completion/claims.json.
- DO NOT make protected edits to: ralph.json, ralph.completion.json, FEATURES.md, verifier/config/policy files, controller state, frozen tests, crates/** (product code), tools/ (validator scripts).
- No Cargo, no canonical/product/test/validator edits.

## Authority Disposition Recommendation
- OPS-006 MUST remain classified as `unresolved-decomposition` pending task-specific source/path evidence that separates it from the OPS-002/003/004/006/008 equivalence group.
- The candidate fragment `repository-reference-normalization` is the strongest strict-TDD-friendly fragment but cannot be assigned to OPS-006 alone without a source-grounded decomposition that resolves the identical-signature ambiguity.
- The FEATURES.md "accepted" row for OPS-006 (line 69, gate b, "quiesced-tree re-run formality") is NOT a valid authority basis: the ownership-gap file, backlog-exhaustion ledger, DISC-003 worklog, and task card itself all indicate NOT STARTED / no owner / no task-specific binding.
- OPS-006 should be treated as BLOCKED (not completed) at the controller/plan level until a deliberate source-to-task decomposition is performed by the orchestrator/authority.

## Patch Fields Recommendation (do NOT apply; recommend to orchestrator)
1. **ralph.json OPS-006 status** (line 1247): reconcile `"status": "accepted"` against validator expectation `"not-started"`. Either (a) revert ralph.json OPS-006 to `"not-started"` to match backlog-exhaustion.json `controllerStatus: "not-started"` and validate_backlog_exhaustion.py expectations, or (b) if "accepted" is intentional, the validator and gap analysis must be updated via the controller authority process -- NOT by this audit lane.
2. **FEATURES.md acceptance row** (line 69): OPS-006 acceptance row should NOT carry `accepted` while `ownershipDecision.OPS-006 = null` and `taskCard: null`. Recommend flipping to `not-started` or adding an explicit `blocked` status with the reason `operations-family-not-decomposed` and citing the ownership-gap disqualifiers.
3. **sources/operations-ownership-gap.json** `ownershipDecision.OPS-006` (line 82): must transition from `null` to a source-grounded task-specific binding ONLY after a source-to-task decomposition resolves the OPS-002/003/004/006/008 ambiguity. Until then it must remain `null` and the gap must remain open.
4. **sources/operations-ownership-gap.json** `taskBindingState.OPS-006` (lines 121-126): `taskCard` is currently `null` but `tasks/OPS-006.md` exists. Either the gap file taskBindingState must be updated to reference the task card (with a deliberate review note per line 1046-1047 check), or the gap analysis must remain treating it as insufficient. This is a controller-level decision, not an audit-lane edit.

## Blocker
OPS-006 ownership is blocked by `operations-family-not-decomposed` (reasonKey in backlog-exhaustion.json). The `repository-only` equivalence group (OPS-002/003/006) has identical surface signatures and no task-specific path evidence selects one. The candidate fragment `repository-reference-normalization` has `ownershipEstablished: false