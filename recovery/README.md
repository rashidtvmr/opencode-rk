# Recovery state

This directory is the small machine-readable state for active Recovery Mode.

- `legacy-snapshot.json` freezes the pre-recovery local branch tips and worktree registrations as evidence. Those refs are not the execution queue.
- `phase1-gates.json` is the exact-SHA acceptance state for the local native Phase-1 product.
- `failures.json` is the only active product-work queue. Add items only from observed failures or an executable probe for an `UNPROVEN` Phase-1 gate.
- `salvage-index.json` records legacy code/test dispositions only when a current failure causes that lineage to be inspected.

Run `python3 tools/recovery_guard.py` before taking work. The guard fails closed if frozen refs move or Recovery Mode configuration becomes unsafe.

Do not measure completion by the number of remaining branches/worktrees. Phase 1 completes when all Phase-1 gates are `ACCEPTED` on one integrated SHA and `failures.json` has no unresolved Phase-1 item.
