# LEDGER-CONVERGENCE-TRANSACTION-CORRECTION

## Claim

- Task: `LEDGER-CONVERGENCE-TRANSACTION-CORRECTION`
- Type: research
- Session: `ses_f2dbead97ffedhjhN8zZC8VZDT`
- Branch/worktree: `plan/ledger-convergence`
- Owned path: `worklog/LEDGER-CONVERGENCE-PROPOSAL.md`
- Scratchpad: this file
- Boundary: specification only; no canonical ledger, controller, plan, test, or
  acceptance mutation

## Source evidence

- `642d8b824f4677dd9011b991aaf1a5bb351141e8` final verifier: b15 base has
  current gate `94`, R78 plus three demotions leaves `12`; transaction rejected
  for circular receipt hashes, missing lock/CAS/branch guard, atomic fsync,
  recovery, guarded rollback, and post-receipt protocol.
- `worklog/LEDGER-CONVERGENCE-PROPOSAL.md:29-39,45-131`: authoritative
  R51/R27 derivation, disjoint union R78.
- `worklog/LEDGER-CONVERGENCE-PROPOSAL.md:164-206`: dead evidence and
  controller-only API boundary.
- `tools/completion_claims.py:187-192`: `save_ledger()` direct `write_text()`;
  no CAS or durability.
- `tools/convergence_gate.py:126-148`: completed-only off-plan and bad-note
  finding algorithm.
- `PLAN.md:113-120,162-180,241-248`; `docs/TDD.md:58-83`; `docs/SECURITY.md:57-77`;
  `docs/CONVERGENCE.md:87-105`: independent verifier, immutable evidence,
  no self-acceptance, preservation and stop requirements.

## Requirement matrix

| Requirement | Corrected proposal location | Evidence/boundary |
|---|---|---|
| b15 residual is 12 | Count model, corrected base row and residual list | `b15e0b6` correction row is completed; not in-progress; 94 -> 12 after operation |
| Non-self-referential hashes | Transaction, Hash domains | `row-v1`, `file-v1`, `ledger-v1`, `manifest-v1`, `receipt-v1`; self fields absent |
| Phase A evidence | Phase A | Re-home file + manifest committed before ledger publication; no commit/ref/receipt preimages |
| Phase B ledger | Phase B | Exact Phase A IDs/hashes, final CAS immediately before rename, non-force push |
| Phase C receipt | Phase C | Independent verifier; receipt hash excludes `receiptHash`; Phase C commit recorded after hash |
| Exclusive lock | Lock/journal | Common Git dir, mode 0600, non-blocking exclusive, fail closed, held to Phase C push |
| CAS inputs | Phase B | HEAD, remote ref, ledger bytes/hash, plan/mapping refs, evidence/manifest hashes |
| Durable publication | Phase A/B/C and recovery | same-filesystem temp, file fsync, atomic replace, parent fsync |
| Crash recovery | Recovery matrix | temp, rename, directory fsync, commits, pushes, receipt all covered |
| Guarded rollback | Lock/journal | Expected current hash/tip required; no overwrite on concurrent change; post-B revert commit only |
| Helper restriction | Transaction introduction | `save_ledger()`, raw `del`, unreviewed status writes forbidden |
| Controller authority | Accounting, API boundary, handoff | only separate authorized controller may retire/demote/publish |
| Independent verification | Phase C and post-operation boundary | no gate result or receipt is parent acceptance |

## Observed scenario and accounting

The verifier's immutable evidence says:

- R51=51, R27=27, intersection=0, union=78.
- `b15e0b6` ledger SHA-256:
  `f0cf39f78d5dbc4ddeeb6d9725d45a53f22326a97b429f481f1a70b95ee280af`.
- b15 current gate: `94`; R78 plus three demotions leaves exactly 12 lines.
- Corrected residual IDs: `APP-010-FROZEN-INTEGRITY` twice,
  `APP-010-REVISION-RECEIPT`, `APP-010-REVISION-RECEIPT-INTEGRATION`, four
  `APP-012-*` rows, `LEDGER-CONVERGENCE-PROPOSAL`,
  `LEDGER-CONVERGENCE-PROPOSAL-CORRECTION`,
  `LEDGER-CONVERGENCE-VERIFY`, and `LEDGER-CONVERGENCE-VERIFY-RETRY`.
- Current branch additionally has final-verifier row and gate `95`; that row is
  not silently included in DISC-003.
- Three missing scratchpads remain `LANE-CI-EXT`, `WEB-EVENT-STREAM`,
  `WEB-HINT`; their row hashes are preserved in the proposal.

## Disposable author simulation

The bounded simulation below models canonical JSON hashing, a manifest that
excludes its own hash, Phase B CAS rejection on a changed ledger/tip, and
rollback rejection on a changed output. It uses no repository files or network.

## Decisions

- Phase A must be a separate immutable commit because a destination commit hash
  cannot be a preimage of the file that creates that commit.
- Phase B must re-read CAS values immediately before rename, not only at lock
  acquisition. A Git lock does not fence external branch advancement.
- Phase C receipt is a new append-only artifact. Its hash excludes its own hash
  and its commit ID; verifier receipt cannot prove itself.
- Existing claim API remains unchanged. A future controller lane must implement
  reviewed transaction primitives; workers cannot extend or bypass it here.

## Tests and limits

- No product tests. No Cargo, database, network, or heavy process.
- Required validators: `python3 tools/convergence_gate.py`,
  `python3 tools/validate_repository.py`, `git diff --check`.
- Disposable hash/CAS simulation: `SIM PASS manifest_nonrecursive=1
  cas_mismatch=1 guarded_rollback=1 receipt_nonrecursive=1`.
- `python3 tools/convergence_gate.py`: expected blocked current gate,
  `total=95`, exit 1; no ledger mutation.
- `python3 tools/validate_repository.py`: expected independent backlog failure,
  51 errors, exit 1; no policy/backlog mutation.
- `git diff --check`: exit 0.
- Frozen product tests/artifact hashes: none applicable to research; cited
  verifier hashes preserved verbatim.
- Resource bound: bounded stdlib Python only; no retained external data.

## Remaining unknowns

- Fresh application-time branch tip, ledger bytes, and mapping blob hashes must
  be recomputed by the future controller.
- Controller must choose an OS-specific lock primitive and reviewed JCS
  implementation compatible with its runtime; protocol does not authorize a
  worker to add either.
- Independent verifier must approve this corrected proposal before application.
