# Recovery state

This directory contains **tracked immutable definitions/seeds** for Recovery Mode. Mutable operational state must not be committed because doing so would change the exact SHA being evaluated.

- `legacy-snapshot.json` freezes the pre-recovery local branch tips and worktree registrations as evidence. Those refs are not the execution queue.
- `phase1-gates.json` defines the Phase-1 gates and required judge checks. Runtime gate status lives in Git common-dir `recovery-state/gates.json`.
- `failures.json` seeds the runtime queue. Claims/reviews/transitions live in Git common-dir `recovery-state/failures.json` and are changed only by `tools/recovery_state.py`.
- `salvage-index.json` is the tracked salvage template. Runtime dispositions live outside tracked history.

Initialize once with `python3 tools/recovery_state.py init`. Thereafter run `python3 tools/recovery_state.py status` and `python3 tools/recovery_guard.py` before taking work. The guard fails closed if frozen refs move, unauthorized branches/worktrees appear, claims overlap, lanes drift too far, or accepted state becomes stale.

Final judge evidence is stored under Git common-dir `recovery-evidence/<sha>/`; it is intentionally not committed. `tools/phase1_judge.py --sync-state` can mark runtime gate state accepted only when every Phase-1 gate passes in one installed-artifact run on the exact current `main-v2` SHA.

Do not measure completion by the number of remaining branches/worktrees. Phase 1 completes when all Phase-1 gates are `ACCEPTED` on one integrated SHA and `failures.json` has no unresolved Phase-1 item.
