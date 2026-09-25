# APP-012 expiry and retention integration proposal

Base: `7262682e31c1f473912c9483804c9430eb4ee4c5`.

Candidate sources:

- expiry GREEN `ea295455444aa879eca6126f6afc92fc9ba4ec53`;
- retention GREEN `6b65207494bc1b089f7a236760cacdc1b42ea8e4`.

Scope: product-only composition of the two independently authored candidate
patches. The candidate claim-ledger commits intentionally are not copied: their
histories conflict with this common integration base and the ledger is
controller-owned. Integration authority must reconcile/release claims rather
than accepting a cherry-pick conflict resolution from this proposal.

Frozen tests remain byte-identical:

- `crates/security/tests/app012_approval_expiry_red.rs` expected SHA-256
  `a7f844388a6f0c5f2774f51c5989b9dfd70ed3a569d22e8d765ea5862b1542ea`;
- `crates/storage/tests/app012_approval_retention_red.rs` expected SHA-256
  `e2f3ecd211ddd27dd7aad803bd95d9a9263873c8d61008d14367d9520f7111cc`.

This branch is an integration proposal, not mainline acceptance. Repository
authority, independent verification, and exact integrated-revision reruns remain
mandatory. The adjacent `expire_sweep` null `resolved_at_us` gap remains open.

## Combined-tree receipt

- Frozen test files were copied byte-for-byte from the pushed GREEN worktrees;
  both SHA-256 values matched the expected values above before execution.
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p
  opencode-rk-security --test app012_approval_expiry_red --
  --test-threads=1`: 1 passed, 0 failed.
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p
  opencode-rk-storage --test app012_approval_retention_red --
  --test-threads=1`: 3 passed, 0 failed.
- Commands were serialized under the one-Cargo semaphore. This proves scoped
  compatibility on the proposal revision only; it is not independent
  verification or origin/main evidence.
