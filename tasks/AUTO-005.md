# AUTO-005

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-002 (Plan file Ralph JSON independent slices and autonomous loop), REQ-004 (Strict TDD and independent verification).
Dependencies: none.
Test obligations: AUTO-005-T01, AUTO-005-T02, AUTO-005-T03, AUTO-005-T04, AUTO-005-T05.

## User-observable outcome

The trusted RED/GREEN pipeline enforces the TDD contract end to end: implementer proposes tests, trusted controller freezes them, independent verifier runs them. No slice is accepted on a self-report, an edited test, or an unfrozen suite. Every RED compiles and fails for the missing behavior; every GREEN passes the frozen suite on the exact integrated revision; verifier reruns independently before acceptance.

## Source evidence

- PLAN.md sections 5-6: slice independence rules, mandatory RED/GREEN lifecycle (inspect -> contract -> RED -> freeze -> implement -> GREEN -> regress -> integrate -> rerun).
- PLAN.md section 8: serial model-agnostic controller, trusted adapters, fresh candidate workspaces, state/receipts/budgets outside worker sandbox.
- docs/TDD.md sections 2-5: lifecycle order, compiling RED, frozen hash + command manifest, GREEN minimum, evidence + patch never acceptance.
- tools/plan_model.py: AUTO-005 rank 0 reference/trusted pipeline (PHASE_RANK AUTO=6, PHASE_OVERRIDE AUTO-001/AUTO-002=0; AUTO-005 inherits trusted-pipeline reference role per milestone M0 table PLAN.md:129).
- tools/ralph_loop.py: prototype controller (ready queue, isolated worktree, bounded worker, verification commands, append-only receipts, resumable ledger; only controller writes `state/`).
- tools/lane_gate.py: lane gate re-checks artifact on disk and runs test target, never trusts self-report.
- tools/validate_repository.py: canonical repository guard (backlog exhaustion, DISC-003 manifest, plan structure).
- docs/SECURITY.md: scope/verifier config/resource limits/reference pins not writable by implementation sandbox (ADR-007).

## Observable contract

- Implementer proposes tests; controller freezes test hash + command manifest; independent verifier runs frozen suite on exact revision.
- RED suite compiles and fails for the missing behavior (missing module/behavior assertion, not import error).
- Frozen hash recorded by controller equals hash of test artifact on disk at verify time.
- GREEN passes the frozen suite unedited; any post-freeze test edit is rejected.
- Verifier reruns frozen suite independently (fresh read of disk + test target run); worker text/`passes:true` is not evidence.
- Suggested module boundary: controller-owned pipeline code under `tools/` + `config/controller.settings.json` + `state/` ledger (prototype: `tools/ralph_loop.py`, `tools/lane_gate.py`, `tools/plan_model.py`); product Rust port owns no `state/` writes from worker sandbox. Worker ships additive fragment only, never edits shared `lib.rs`, `Cargo.toml`, schemas, migrations.

## Failure states

- RED does not compile (missing imports, syntax error): invalid RED, rejected; no freeze.
- RED compiles but passes before implementation: invalid RED (no missing-behavior failure), rejected; no freeze.
- RED fails for wrong reason (disabled test, `#[ignore]`, never-executed test, fabricated log): invalid RED, rejected.
- Post-freeze test edit (hash mismatch on disk vs frozen): rejected; lane FAIL, re-run from RED.
- Worker-edited `passes:true` / self-report without gate run: not evidence, rejected.
- Verifier run on different revision than frozen/integrated revision: rejected; rerun on exact revision required.
- Blocked pipeline (no safe ready work, budget exhausted, missing credentials, isolation test failure, human authority required): record exact blocker, stop; blocked is not complete.

## Resource bounds

- Bounded retries only on classified repairable failures; after attempt cap record blocker and continue independent tasks (PLAN.md section 8).
- No unbounded queue, no unbounded retained output; receipts append-only, ledger resumable, both outside worker sandbox.
- Byte budgets on retained logs/evidence; lazy services; scoped cancellation with timeout on worker + verifier runs.
- No detached task without owner; heartbeat/lease lifetime scoped to lane where applicable; no per-agent OS process for orchestration.
- No worker writes to `state/`, test source, verifier config, resource limits, scope, or reference pins after freeze.

## Test obligations (frozen)

- AUTO-005-T01 (RED compiles and fails for missing behavior): fixture slice with missing behavior: `assert!(suite_compiles)` , `assert!(fail_count >= 1)`, `assert!(fail_reason == MissingBehavior)`, `assert!(no_compile_error && no_ignored && executed_count == total_count)`.
- AUTO-005-T02 (freeze hash recorded): after RED: `assert_eq!(frozen_hash, hash(test_files_on_disk))`, `assert!(command_manifest_frozen)`, `assert!(test_source_not_writable_by_worker)`, controller re-read matches frozen bytes.
- AUTO-005-T03 (GREEN passes frozen suite): after minimum implementation: `assert!(green_on_frozen)`, `assert_eq!(hash(test_files_on_disk), frozen_hash)`, `assert!(regressions_clean)`, refactor + rerun still green.
- AUTO-005-T04 (edited-test rejected): mutate one frozen test byte / flip one assertion, rerun gate: `assert_eq!(gate_result, REJECTED)`, `assert!(hash_mismatch_detected)`, `assert!(!accepted)`, restore frozen bytes before any further GREEN claim.
- AUTO-005-T05 (verifier independent rerun): independent verifier process re-reads disk + reruns test target on exact integrated revision: `assert!(verifier_rerun_pass == gate_pass)`, `assert_eq!(verifier_revision, integrated_revision)`, `assert!(worker_self_report_ignored)`, receipt appended with exact commands/hashes.

## TDD steps

1. Inspect: PLAN.md 5-6/8, TDD.md 2-5, tools/plan_model.py (AUTO-005 rank 0 reference/trusted pipeline), tools/ralph_loop.py, tools/lane_gate.py (done, see evidence).
2. Contract: defined above.
3. Author tests AUTO-005-T01..T05; establish compiling RED (fail: missing pipeline enforcement, not import error).
4. Freeze test hash + command manifest via controller.
5. Implement minimum trusted RED/GREEN pipeline enforcement (propose -> freeze -> verify -> accept/reject).
6. GREEN, refactor, rerun; negative tests (edited-test rejected, wrong-revision rerun rejected, fabricated-pass rejected, blocked-stop recorded).
7. Evidence + patch; verifier decides acceptance.

## Verification

```bash
git status --short -- tasks/AUTO-005.md
python3 tools/validate_repository.py
python3 tools/lane_gate.py
```
