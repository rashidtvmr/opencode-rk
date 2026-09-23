# GUARD-OPS-005 — reconciliation audit (read-only guard, no acceptance)

- claim: OPS-005 claimed in-progress, session ses_f32482b4bffeNKh3cwt6IFtWXo, scratchpad worklog/GUARD-OPS-005.md. Own files only: this file + ledger row. No canonical/product/test edits.
- rev: c8f6dcb. Gate: convergence_gate BLOCKED total=80; validate_backlog_exhaustion 51 errors; validate_repository FAIL backlog exhaustion exit=1.

## 1. Ralph vs task vs worklog (current code truth)

- ralph.json userStories: OPS-005 status accepted, requirementIds [], testObligations OPS-005-T01..T05, userStory generic DISC-002 line.
- tasks/OPS-005.md: Status NOT STARTED header (line 3) but full product contract present: pure merge_overlay, ConfigDoc Scope Global<Project<Local, caps 4096/256B/64KiB, failure states, T01-T05 frozen. Suggested boundary crates/ops/src/config_overlay.rs:57 (crate crates/ops does NOT exist).
- worklog/OPS-005.md: lane claims GREEN 5/5, frozen test sha 5ca563b6, src sha bb3579ae, lib.rs:5 prewired. Lane worklog exists.
- on-disk impl (NOT this lane's work, cited only): crates/foundation/src/config_overlay.rs:1-137 pure merge; crates/foundation/tests/config_overlay.rs:8-9 #[path] include (frozen T01-T05); crates/foundation/src/lib.rs:5 pub mod config_overlay wired. Task-card path crates/ops/... MISSING by construction; AUDIT-NONACCEPTED.md:54 records MISSING + lane-variant verify.
- FEATURES.md: OPS-005 accepted at lines 68, 650, 896. Consistent with ralph accepted. No FEATURES drift for OPS-005.

## 2. Authority conflicts blocking acceptance

- operations-ownership-gap.json: status source-reviewed-no-exact-task-owner; ownershipDecision.OPS-005 null:81; taskBindingState.OPS-005 taskCard null + worklog null:115-120; equivalence group configuration-only OPS-001/OPS-005 identical signatures:42-52; partition configuration-discovery-and-policy ownershipBlocker:229 explicitly denies OPS-005 ownership (identical signature + closed BASE overlap).
- backlog-exhaustion.json stories OPS-005: controllerStatus not-started, taskCard null, taskStatus null, worklog null, implementationCommits [], category unresolved-decomposition, reasonKey operations-family-not-decomposed. Directly contradicts ralph accepted + existing task/worklog/impl.
- validator tools/validate_backlog_exhaustion.py:1011-1050 hardcodes expected status not-started + generic story + taskCard None + worklog None for OPS-005. Live tree violates all four, emitting exactly: "OPS-005: operations ownership-gap is stale after Ralph task semantics changed" + "OPS-005: task/worklog appeared; operations ownership gap needs deliberate review". Plus 2 global rows covering OPS-005 (accepted-story-in-ledger list, summary drift). Total OPS-005-attributable: 2 specific + share of 2 global of 51.
- convergence_gate total=80 BLOCKED; OPS-005 is one blocked family member, not the sole cause. Parent OPS family unresolved partitions (gap:504-512) still list configuration-discovery-policy-and-location-lifetime etc. as unresolved.
- surface-identity bar: OPS-001/OPS-005 live signatures both [opencode.configuration-runtime] (gap:17-33). No evidence in task/gap distinguishes them. Accepting the pure-overlay fragment for OPS-005 without retiring/duplicating OPS-001 invents ownership by subtraction, violating closure criteria gap:517-521.

## 3. Exact authority patch proposal (controller-only, NOT applied)

Controller decides A or B as one atomic commit (gap + ledger + validator + ralph/task disposition together; partial edits re-drift):

Option A — retire OPS-005 overlay claim (keeps gap closed-world):
1. ralph.json: OPS-005 accepted -> not-started, or remove testObligations if story retired; record decision note.
2. tasks/OPS-005.md + worklog/OPS-005.md: delete or mark SUPERSEDED with pointer to owning story; remove crates/foundation/src/config_overlay.rs + tests/config_overlay.rs + lib.rs:5 wire if fragment has no accepted owner (or re-attribute to BASE-006/OPS-001 with evidence).
3. sources/backlog-exhaustion.json OPS-005 row: keep not-started/null (already so) — no change.
4. sources/operations-ownership-gap.json: no change (already null/no-owner).
5. tools/validate_backlog_exhaustion.py: no change.
6. FEATURES.md lines 68/650/896: accepted -> not-started/removed per ralph truth.

Option B — deliberately award pure-overlay partition to OPS-005 (opens gap by review):
1. sources/operations-ownership-gap.json: taskBindingState.OPS-005 set taskCard "tasks/OPS-005.md", worklog "worklog/OPS-005.md", ralphStory = live ralph userStory; ownershipDecision.OPS-005 = "configuration-discovery-ordering-only (pure merge; discovery/mutation/cache excluded)"; reviewedPartitions[configuration-discovery-and-policy] add owner OPS-005 + exclusion list (BASE-006, INT-002, OPS-007, OPS-009); unresolvedPartitions remove or annotate configuration-discovery-policy-and-location-lifetime with residual items; candidateFragments add overlay-merge fragment with inputs/outputs/failure/lifetime from tasks/OPS-005.md:50-80; equivalenceGroups: split configuration-only (OPS-001 vs OPS-005 partition split) or document shared-signature dual-owner rationale; OPS-001 disposition in same commit (else ownership still ambiguous).
2. sources/backlog-exhaustion.json OPS-005 row: controllerStatus accepted, taskCard tasks/OPS-005.md, worklog worklog/OPS-005.md, implementationCommits [landing commit of config_overlay.rs], localEvidencePaths [crates/foundation/src/config_overlay.rs, crates/foundation/tests/config_overlay.rs], category/surfaceIds updated to owned-partition.
3. tools/validate_backlog_exhaustion.py:1011-1050: expected_requirements/status/story/binding for OPS-005 updated to accepted + taskCard/worklog paths; ledger-row check (line ~1100) allow owned-partition values for OPS-005; FEATURES/ledger accounting updated so global accepted-in-ledger + summary rows clear.
4. tasks/OPS-005.md:57-59: correct module boundary crates/ops/... -> crates/foundation/src/config_overlay.rs (or create crates/ops with move); header Status NOT STARTED -> IMPLEMENTED/ACCEPTED per controller vocabulary.
5. FEATURES.md 68/650/896: keep accepted (already consistent).
6. Verifier re-run on quiesced tree: frozen T01-T05 via crate target (not #[path] shim) + RED receipt (RED-VALIDITY-OPS Orosz: OPS-005 config_overlay 5 E0432) re-attached to lane commit; I18-style quiesced re-run per FEATURES:68 caveat.

- Recommended: Option A unless controller writes the OPS-001/OPS-005 split evidence, because gap:229 + gap:517-521 forbid subtraction-based ownership and OPS-001 remains accepted with identical signature.
- Guard applied neither; no test/product/canonical edits made; no acceptance claimed.

## 4. Verification run (read-only, light)

- python3 tools/validate_backlog_exhaustion.py: 51 errors; OPS-005 rows: stale-after-Ralph + task/worklog-needs-review (+2 global rows listing OPS-005).
- python3 tools/validate_repository.py: FAIL backlog exhaustion exit=1; same OPS-005 rows.
- python3 tools/convergence_gate.py: CONVERGENCE BLOCKED total=80.
- git status: only M tasks/completion/claims.json + untracked this file pre-commit. No product diff.

## 5. Remaining unknowns / handoff

- Which option controller takes (retire vs award + OPS-001 split).
- Whether foundation config_overlay impl is re-attributed, moved to crates/ops, or removed under Option A.
- Quiesced-tree GREEN re-run belongs to verifier post-decision, not this guard.
