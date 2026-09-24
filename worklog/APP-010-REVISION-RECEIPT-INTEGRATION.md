# APP-010-REVISION-RECEIPT-INTEGRATION

## Claim
- Task: `APP-010-REVISION-RECEIPT-INTEGRATION`
- Session: `ses_f2e29869bffeaVXDcsIsbjwjDl`
- Branch: `lane/PHASE1-product-spine-20260923`
- Claim result: successful; no pre-existing task row found. Prior delegated worker reportedly stopped at provider startup for insufficient quota; no accepted work observed.
- Owned paths: `crates/cli/build.rs` (audit only; no product/test edits), this worklog, and ledger row.

## Source evidence
- `crates/cli/build.rs:23-48`: emits env rerun trigger, discovers repository, validates explicit `OC2_BUILD_REVISION`, emits `GIT_COMMIT`.
- `crates/cli/build.rs:62-169`: bounded repository metadata discovery and watch paths.
- `crates/cli/build.rs:171-214`: direct bounded Git invocation, environment clearing, output validation.
- `crates/cli/tests/installed_default_entrypoint.rs:356-360`: runtime receipt precedence, compile-time receipt fallback.
- `crates/cli/tests/installed_default_entrypoint.rs:530-535`: missing/empty receipt failure path.
- `worklog/APP-010-REVISION-RECEIPT.md:38-49`: prior worker claimed native check and installed tests green, but integration still requires independent exact-revision verification.
- `worklog/APP-010-FROZEN-INTEGRITY.md:10-31`: frozen test canonical blob and hash; no test edits.
- `tasks/completion/claims.json`: prior receipt/frozen-integrity rows are completed; no integration row before claim.

## Observable contract
- Explicit `OC2_BUILD_REVISION` is 40 lowercase hex characters; valid value takes precedence and emits `GIT_COMMIT`.
- Invalid explicit value fails with fixed diagnostic and never echoes supplied value.
- Without explicit value, direct Git HEAD discovery emits only a validated exact revision.
- Git failure/missing metadata omits `GIT_COMMIT` without failing the build.
- Installed frozen journey uses compile-time `GIT_COMMIT` when runtime `OC2_E2E_REVISION` is absent; explicit valid runtime receipt remains 5/5.
- Frozen test source must remain byte-identical to origin; no test edits.

## Failure states and bounds
- Explicit invalid receipt: deterministic build-script failure; sensitive value absent from output.
- Git unavailable/invalid output: no receipt, successful build script.
- File reads bounded 4096 bytes; Git stdout bounded 128 bytes; no shell; no inherited environment except PATH and noninteractive Git controls.
- Any source correction requirement stops lane blocked; do not edit `build.rs`.

## Decisions
- Integrator role only: audit, verify, land, push; do not alter product/test logic.
- Use one heavy command at a time with `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`.
- Preserve current worktree uncommitted artifacts until exact diff/hash audit determines whether they are intended lane files.

## Tests / verification
Pending exact-head, receipt, native check, frozen-integrity, diff, landing, and post-push checks.

## Remaining unknowns
- Whether `build.rs` has a semantic defect requiring blocked stop.
- Whether exact current HEAD differs from receipt worker's recorded `1f4a9e6ce4fa96ea2bf8b01bb034160a9d9c60a9`.
- Whether remote branch advances during integration.
