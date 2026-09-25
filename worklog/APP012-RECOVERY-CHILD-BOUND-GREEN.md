# APP012-RECOVERY-CHILD-BOUND-GREEN

## Claim
- Task: `APP012-RECOVERY-CHILD-BOUND-GREEN`.
- Session: `ses_f296b0162ffey6cK3KjlZp4BKX`.
- Continued by orchestrator session `ses_f3c4de578ffelQv59xDXmOs03B` after
  lawful reclaim when the prior workers exhausted their step budgets.
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/lane-app012-recovery-child-bound-green`.
- Branch: `lane/APP012-RECOVERY-CHILD-BOUND-GREEN`.
- Owned product file: `crates/storage/src/facade.rs`.
- Other permitted files: this worklog; own row in `tasks/completion/claims.json`.
- No test/schema/lib/Cargo edits.

## Authority and source evidence
- Read `.agents/WORKER.md`, `PLAN.md`, `docs/TDD.md`, `docs/SECURITY.md`, `docs/CONVERGENCE.md`, sqlite-safety skill.
- `crates/storage/src/facade.rs:148-221`: one IMMEDIATE recovery transaction; child attempts/tools scans are capped at `STARTUP_RECOVERY_LIMIT=500`, join roots while `e.state=1`, then roots are capped separately and converted to uncertain.
- `crates/storage/schema/v2/workspace.sql:144-212`: execution root states, open/uncertain child states, terminal checks, recovery indexes.
- `worklog/APP012-RESTART-RECOVERY-VERIFY.md:38-44`: R1 identifies the `e.state=1` dependency and stranded child beyond the first 500-row scan.
- `worklog/APP012-RECOVERY-CHILD-BOUND-RED.md:63-65`: frozen child RED requires bounded deterministic roots, all selected-root children eventually uncertain, terminal/prior/unrelated rows untouched.
- Frozen child RED SHA-256 expected: `fe78660842335e79e33787cb95bb675a80c65e541b3bd52fa9e92d1efc9138b0`.

## Contract
- Existing DB startup performs one bounded IMMEDIATE transaction.
- Select at most 500 deterministic prior-generation running roots.
- Convert open provider attempts and dispatched tools belonging to those selected roots; child updates must not require the root still have `state=1` after root conversion.
- Inspect at most 500 child rows per startup, as frozen by
  `app012_recovery_child_bound_red.rs:3-7,198-220`; later opens must still find
  residual open children after their root became uncertain. Root selection is
  independently capped at 500, and no unbounded Rust vector/parameter list is
  retained.
- Do not touch terminal children, prior/unrelated roots, or fabricate timestamps; no inserts/replay.
- Preserve existing restart and storage behavior.

## Observed baseline
- Frozen child target reproduced before implementation: `1 passed, 1 failed, 0 ignored` (`recovery_cleans_children_for_each_selected_root_atomically` passed; `recovery_does_not_strand_children_after_bounded_root_scan` failed at line 218 with `left: (1, 500, 502)`, `right: (0, 501, 502)`). Command: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-storage --test app012_recovery_child_bound_red -- --test-threads=1`.
- Frozen hashes matched before edit: child `fe78660842335e79e33787cb95bb675a80c65e541b3bd52fa9e92d1efc9138b0`; restart `d2745fe142fb1add3cc26c9fe9715438c73a51084d5498373c990c0a8e2a8cce`; installed APP-012 `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`.

## Decisions
- Use a connection-local temporary `recovery_roots` table inside the existing
  IMMEDIATE transaction. It contains at most 500 deterministic eligible roots.
- Eligible roots include prior-generation running roots and uncertain roots
  that still own open provider/tool children. Child scans retain the frozen
  500-row startup cap, so residual children drain on later opens instead of
  being stranded by the root's state transition.
- Retain the redundant partial-index state predicates required for SQLite to
  satisfy the explicit `INDEXED BY` clauses. The earlier simplified query
  failed with `no query solution`.
- Drop the temporary table before commit. Terminal child states/timestamps are
  outside update predicates; no insertion or replay occurs.

## Tests and evidence
- Frozen child: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p
  opencode-rk-storage --test app012_recovery_child_bound_red --
  --test-threads=1` -> 2 passed, 0 failed.
- Restart regression: same bounded environment, target
  `app012_restart_resume_red` -> 18 passed, 0 failed.
- Storage lib -> 122 passed, 0 failed.
- Storage `--tests` -> 207 total tests passed across lib and integration
  targets, 0 failed. Existing compiler warnings were unchanged and outside the
  leased file.
- `CARGO_BUILD_JOBS=1 cargo check -p opencode-rk-storage` -> pass.
- `rustfmt --edition 2021 --check crates/storage/src/facade.rs` and
  `git diff --check` -> pass.
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 python3 tools/lane_gate.py
  --json --run` -> all eight source lanes and four executable test lanes PASS
  (2 + 4 + 5 + 6 tests in the gate-owned targets).
- Frozen hashes preserved: child `fe78660842335e79e33787cb95bb675a80c65e541b3bd52fa9e92d1efc9138b0`,
  restart `d2745fe142fb1add3cc26c9fe9715438c73a51084d5498373c990c0a8e2a8cce`,
  APP012 journey `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`.
- No frozen test changed. Post-run process scan found no Cargo, rustc, or
  recovery/restart test survivor; the only broad-pattern match was an unrelated
  pre-existing npm/wrangler process.

## Remaining unknowns
- Independent verifier and exact integrated-revision rerun remain required.
- This repair closes only R1 child stranding. Generation enforcement, OS owner
  lock, approval conditional resume, and daemon/live-turn wiring remain open.
