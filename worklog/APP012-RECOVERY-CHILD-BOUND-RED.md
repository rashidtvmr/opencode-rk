# APP012-RECOVERY-CHILD-BOUND-RED

## Claim

- Task: `APP012-RECOVERY-CHILD-BOUND-RED`.
- Role: RED authoring for verifier residual R1.
- Session: `ses_f2979f4fdffe6mFaOAzpCO77L2`.
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/red-app012-recovery-child-bound`.
- Branch: `red/APP012-RECOVERY-CHILD-BOUND`.
- Owned test: `crates/storage/tests/app012_recovery_child_bound_red.rs`.

## Authority and source evidence

- Read `.agents/WORKER.md`, `PLAN.md`, `docs/TDD.md`, `docs/SECURITY.md`, `docs/CONVERGENCE.md`, sqlite-safety skill, frozen restart RED/worklog, implementation `f83637d`, verifier `48d0351`.
- `crates/storage/src/facade.rs:27-43`: `StorageFacade::open` opens an existing v2 file through `SchemaV2::open_existing` and invokes `recover_existing` before returning.
- `crates/storage/src/facade.rs:148-221`: recovery uses one IMMEDIATE transaction, `STARTUP_RECOVERY_LIMIT=500`, updates attempts/tools before roots, joins child scans on `e.state=1`, then converts running roots to state 4.
- `crates/storage/schema/v2/workspace.sql:170-185`: provider attempts permit 501 distinct open children under one execution through `UNIQUE(execution_pk, ordinal)` and state/finished timestamp checks.
- `crates/storage/src/execution_v2.rs:20-137`: `ExecV2::start_execution`, `transition_execution`, `record_attempt`, and `finish_attempt` provide the real lifecycle setup and terminal CAS.
- `worklog/APP012-RESTART-RECOVERY-VERIFY.md:38-44`: verifier residual R1 identifies child rows beyond the 500 child batch as stranded after their root becomes uncertain because later scans require `e.state=1`.
- `docs/storage/CRASH_CONSISTENCY.md:162-177`: ambiguous dispatched/running work becomes uncertain; no automatic replay.

## Observable contract and reproduction

One disposable v2 DB contains one prior-generation running execution, 501 open provider attempts, and one terminal success attempt. First `StorageFacade::open` may update only 500 child rows, but must also update the selected root atomically. A later open must reconcile the one child left outside the first child batch. Terminal state and finished timestamp must remain unchanged; row count stays 502; no replay/insertion occurs.

Current `f83637d` behavior is expected to fail the second-open assertion: first open leaves one open child after the 500-row child scan and converts the root to state 4; second child scan cannot select that child because of `e.state=1`, so one attempt remains open. This is fail-for-cause R1, not a compile or fixture failure.

## Tests authored

- `recovery_cleans_children_for_each_selected_root_atomically`: 501 independent prior-generation running roots, one open attempt each. First open asserts exactly 500 roots handled, no open child remains under a selected uncertain root, and second open drains the remaining root. This locks the root bound plus selected-root atomicity.
- `recovery_does_not_strand_children_after_bounded_root_scan`: one prior-generation running root with 501 open attempts plus one terminal success attempt. Real `SchemaV2::initialize_workspace`, `ExecV2` lifecycle, disposable `tempfile` DB, two real `StorageFacade::open` calls.
- First pass asserts root state 4, exactly 500 uncertain + 1 open child, 502 total rows, terminal attempt unchanged.
- Second pass asserts zero open, 501 uncertain, 502 total rows, terminal attempt unchanged.
- Resource bound: fixed 501 open children; no unbounded query or retained output.

## Verification boundary

Package lane owns Cargo. No `cargo`, `cargo check`, `cargo test`, freeze, commit, or product/existing-test edit was run. Only direct rustfmt and diff checks are permitted for this lane.

Pending command for the package lane:

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-storage --test app012_recovery_child_bound_red -- --test-threads=1
```

Expected RED: compile succeeds; current implementation fails the second-open assertion with one open attempt remaining (`left: (1, 500, 502)`, `right: (0, 501, 502)`).

## Status

Claim remains `in-progress` pending package-lane compile/RED execution. No completion claim made.

## Direct checks

- `rustfmt --edition 2021 --check crates/storage/tests/app012_recovery_child_bound_red.rs` -> pass.
- `git diff --check` -> pass.
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-storage --test app012_recovery_child_bound_red -- --test-threads=1` -> compiling target; 1 pass, 1 fail, 0 ignored. Failure is only `recovery_does_not_strand_children_after_bounded_root_scan`: after second open, `left: (1, 500, 502)`, `right: (0, 501, 502)`. The single remaining open child is stranded by the current `JOIN executions ... e.state=1` predicate after the root changed to state 4. The independent-root test passes, proving the root cap and selected-root child cleanup.
- Targeted binary rerun confirmed `recovery_cleans_children_for_each_selected_root_atomically` passes and the residual test fails only at its eventual-uncertainty assertion. No invalid fixture or setup failure.
- Resource profile: 501 rows in each bounded fixture; focused Cargo build 21.08s; focused test 0.05-0.21s; one Cargo process, `pgrep` showed no Cargo/rustc/test survivors after completion. `vm_stat` immediately before testing: 7,075 free pages, 353,023 inactive pages.
- Frozen SHA-256: `fe78660842335e79e33787cb95bb675a80c65e541b3bd52fa9e92d1efc9138b0` for `crates/storage/tests/app012_recovery_child_bound_red.rs`.
- Preserved hashes: restart RED `d2745fe142fb1add3cc26c9fe9715438c73a51084d5498373c990c0a8e2a8cce`; installed APP-012 RED `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`.
- `cargo check`, freeze tooling, and product/existing-test edits were not performed. The package lane owned Cargo; this lane ran only the requested focused test plus direct rustfmt/diff checks.

## Candidate repair direction

Constrained to `crates/storage/src/facade.rs`: keep the one IMMEDIATE transaction and 500-row bound, but make child selection follow a bounded set of roots selected in that same transaction, or select children by prior-generation ownership without requiring `e.state=1`. Convert each selected root only after its selected children are converted. Preserve terminal predicates, no replay, no inserts, and no test edits.

## Completion

Valid compiling RED. Ledger may transition to `completed`; verifier must rerun the frozen hash and focused command. Parent APP-012 remains open.
