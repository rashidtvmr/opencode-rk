# APP-012-READ-INTEGRATION

## Claim

- Task/session: `APP-012-READ-INTEGRATION` / `ses_f2e63a880ffeDs27MCzFINfVE5`.
- Claimed before edits through `tools/completion_claims.py`.
- Role: read-path integrator. No product or test source edits.
- Integration branch: `lane/PHASE1-product-spine-20260923`.
- Candidate base: `eed2bdfd0c8f4f575838e07b820c62235da1a278` (APP-012 frozen RED commit).

## Source evidence

- `crates/tools/src/file_ops.rs:12-13`: hard 64 KiB read cap.
- `crates/tools/src/file_ops.rs:176-208`: concrete file authorization before I/O; `Deny`/`RequireHuman` return without execution.
- `crates/tools/src/file_ops.rs:231-257`: `File::open` + `seek` + `take` bounded read.
- `crates/tools/src/executor.rs:97-123`: ordinary executor remains shell/echo only.
- `crates/tools/src/executor.rs:133-229`: authorized read validates path/range, uses `spawn_blocking`, caps at 64 KiB, maps errors without path/content.
- `crates/server/src/lib.rs:1613-1626`: non-shell tools call `execute_authorized` with the trusted broker.
- `crates/server/src/lib.rs:1653`: existing tool transcript output bound.
- `crates/server/tests/app012_tool_journey_red.rs:313-380`: frozen authenticated provider/read/persistence journey.

## Frozen test verification

- Required SHA-256: `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`.
- Working-tree SHA-256 matches.
- `eed2bdf:crates/server/tests/app012_tool_journey_red.rs` SHA-256 matches.
- `git diff --quiet eed2bdf -- crates/server/tests/app012_tool_journey_red.rs`: exit 0.

## Scope audit

- Reviewed product source scope: `crates/tools/src/file_ops.rs`, `crates/tools/src/executor.rs`, `crates/server/src/lib.rs`.
- Intended evidence scope: four APP-012 read-lane scratchpads plus `tasks/completion/claims.json`; parent `worklog/APP-012.md` records the candidate and explicit open parent gaps.
- Unexpected product diffs: none observed.
- Test diffs: none observed.

## Commands / results

All heavy commands serial, `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`:

- `env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test app012_tool_journey_red -- --test-threads=1`: 1 passed, 0 failed.
- `env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --lib executor -- --test-threads=1`: 5 passed, 0 failed.
- `env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-tools --lib file_ops -- --test-threads=1`: 5 passed, 0 failed.
- `env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --lib broker_gate_tests -- --test-threads=1`: 1 passed, 0 failed.
- `env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --lib agent_loop -- --test-threads=1`: 6 passed, 0 failed.
- `git diff --check`: exit 0.
- `python3 tools/validate_repository.py`: expected repository-wide backlog-exhaustion failure (51 findings); policy/fixture checks passed. No source/controller repair authorized.
- `python3 tools/convergence_gate.py`: expected repository-wide ledger failure (83 findings, including off-plan repair rows); no parent completion claimed.

## Landing evidence

- Integration commit: `40d56d5c6232570a477f79a9f109a407f9811b52`.
- Pushed without force to `origin/lane/PHASE1-product-spine-20260923`.
- After fetch, local HEAD and remote-tracking ref were byte-identical at the integration commit.
- On that exact pushed revision, frozen test SHA-256 remained `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`.
- On that exact pushed revision, the frozen APP-012 command passed 1/1 with `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`.
- Final evidence commit: recorded in the handoff after this evidence-only commit is pushed. Source/test tree unchanged from `40d56d5c6232570a477f79a9f109a407f9811b52`; final frozen hash/test confirmation follows push.

## Remaining unknowns

- Protected-path denial remains open.
- Human approval/resume remains open.
- Daemon restart/resume remains open.
- Installed-package proof remains open.
- Timeout drops the blocking-task handle; bounded I/O remains finite, but cancellation is not hard.
