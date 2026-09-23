# GUARD-OPS-001 - reconciliation audit

## Claim and boundary

- Task: `OPS-001`; session `ses_f32482b65ffe7kTat8JXZRugef`; claimed through `tools/completion_claims.py`.
- Audit only. Owned writes: this scratchpad and the `OPS-001` ledger row. No canonical, product, test, controller, validator, or policy edits.
- Revision audited: `c8f6dcb39d98302f27f04a25897d3880e09e2c1b` (`lane/GUARD-OPS-001-20260923`).

## Exact attributable errors

1. **Ralph versus operations gap.** `ralph.json:1171-1185` declares `OPS-001` `accepted`; `FEATURES.md:64` mirrors `accepted`. The source authority at `sources/operations-ownership-gap.json:76-95` keeps `ownershipDecision.OPS-001: null`, `taskCard: null`, `worklog: null`, and says the requirements do not distinguish OPS-001 from OPS-005. `sources/backlog-exhaustion.json:735-751` independently classifies it `not-started`, `unresolved-decomposition`, no local evidence. `tools/validate_backlog_exhaustion.py:1031-1047,1097-1104` therefore emits the OPS-001 stale-semantics and appeared-task/worklog findings.
2. **Local artifact contradicts unresolved gap.** `tasks/OPS-001.md:1-168`, `worklog/OPS-001.md:1-33`, `crates/foundation/src/resource_ledger.rs:1-469`, `crates/foundation/tests/resource_ledger.rs:1-241`, and `crates/foundation/src/lib.rs:29` exist. The gap and exhaustion ledger require no task card/worklog/implementation commits. This is attributable to OPS-001's local artifact, but not proof of authority to award the upstream partition.
3. **Shared-signature ownership remains unresolved.** `sources/behavior-surface-rules.json:304-329` maps OPS-001 and OPS-005 to the same `opencode.configuration-runtime` surface. `sources/operations-ownership-gap.json:42-52,229,490-501,517-521` forbids ownership by task-number inference, surface subtraction, or equivalence-group membership. The pure ledger implementation cites a new requirement (`tasks/OPS-001.md:54-57`) and explicitly excludes the unresolved upstream partitions (`:87-90`), but no authority record changes the gap's null decision.
4. **Validator/repository gate is RED.** `python3 tools/validate_backlog_exhaustion.py` returned 51 errors; OPS-001-specific errors: stale Ralph semantics and task/worklog appeared. `python3 tools/validate_repository.py` reached protection checks, then failed backlog exhaustion with the same 51-error result. `tools/validate_backlog_exhaustion.py:2490-2510` rejects accepted stories in the exhaustion ledger; `:1013-1047` hardcodes unresolved OPS semantics. Convergence context reports `total=80` blocked. This is a shared authority-state failure, not a product test failure.

## Evidence-based disposition

- **Implementation fragment:** candidate GREEN only. `worklog/OPS-001.md:9-25` records source/test hashes, 5/5 focused tests, and `cargo check -p opencode-rk-foundation`; source is native, bounded, and no-I/O except explicit caller-directed spill (`resource_ledger.rs:383-417`). This does not establish installed daemon wiring, whole-process-tree measurement, or release acceptance. `tasks/OPS-001.md:18-20,107-118` makes those boundaries explicit.
- **Ownership:** blocked. Existing implementation cannot convert the source audit's null ownership into accepted authority. Parent acceptance remains prohibited by `docs/CONVERGENCE.md:26-28,99-105` and `ralph.completion.json:27-35`.
- **Exact status:** ledger `OPS-001` set `blocked`, not `completed`; reason records 51-error validator result and required controller decision.

## Exact authority patch proposal

Controller/integrator only. Apply one atomic authority change after source review; do not edit this audit to make the gate green.

**Recommended Option A: retire or supersede the local fragment unless an owner is proven.**

1. Reconcile `ralph.json`, `FEATURES.md`, `sources/backlog-exhaustion.json`, and `sources/operations-ownership-gap.json` under one controller revision. Set OPS-001 to `not-started` or explicitly retire it with a decision note; keep the unresolved row only if the task remains in scope.
2. If retired, mark `tasks/OPS-001.md` and `worklog/OPS-001.md` `SUPERSEDED` with the controller disposition, and remove or re-attribute `crates/foundation/src/resource_ledger.rs`, its test, and `crates/foundation/src/lib.rs:29` only through the owning integrator. Do not delete frozen tests without test-author authority.
3. Keep `operations-ownership-gap.json` null ownership and unresolved partitions unchanged unless new pinned task-specific source/caller/test/spec evidence is added. Regenerate the exhaustion projection and FEATURES status from Ralph.
4. Run `python3 tools/validate_backlog_exhaustion.py`, `python3 tools/validate_repository.py`, and `python3 tools/convergence_gate.py` on the quiesced integrated tree. Then independently rerun the frozen OPS-001 target only if the controller retains the story.

**Alternative Option B: award the new native fragment deliberately.** Requires all of the following in one authority patch: split OPS-001 from OPS-005 despite identical live signature; add pinned source/caller/test/spec evidence and bounded lifetime/authority to `operations-ownership-gap.json`; assign the exact foundation paths and landing commit; update its `taskBindingState`, `ownershipDecision`, reviewed partition residuals, candidate fragments, and closure record; regenerate `backlog-exhaustion.json`; update validator expectations and Ralph/FEATURES consistently; prove real caller wiring and whole-tree measurement. Surface subtraction alone is explicitly disallowed by `operations-ownership-gap.json:517-521`.

## Verification

- `python3 tools/validate_backlog_exhaustion.py` -> exit 1, 51 errors.
- `python3 tools/validate_repository.py` -> exit 1 at backlog exhaustion; protection/ruleset checks passed first.
- No cargo, browser, database, or heavy command run. Existing lane worklog supplies focused 5/5 and `cargo check` evidence.
- No canonical/product/test edits. Only `tasks/completion/claims.json` own row and this scratchpad changed.

## Remaining unknowns

- Controller choice: retire/supersede versus source-backed award with OPS-005 split.
- Whether implementation remains under foundation fallback or is moved to a dedicated ops crate after authority resolution.
- Integrated caller, daemon process-tree measurement, and release verification remain unproven.
