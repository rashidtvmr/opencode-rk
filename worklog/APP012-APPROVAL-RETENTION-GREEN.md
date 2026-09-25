# APP012-APPROVAL-RETENTION-GREEN scratchpad

Claim: APP012-APPROVAL-RETENTION-GREEN, session
ses_f3c4de578ffelQv59xDXmOs03B.

Base: `8d9dc23c7a23a292f19a30ff3001dbab5632128f` on
`lane/APP012-APPROVAL-RETENTION-GREEN`.

Source evidence:

- `crates/storage/src/retention_v2.rs:87-106`,
  `RetentionV2::sweep_resolved_approvals`, selects terminal states `(1, 2, 4)`
  with non-null bounded age and a clamped `LIMIT`; expired state `3` is absent.
- `crates/storage/src/retention_v2.rs:131-147`,
  `RetentionV2::retention_backlog`, uses the same incomplete state set.
- `crates/storage/src/approvals_v2.rs:130-145`, `ApprovalsV2::expire_sweep`,
  performs the real pending-to-expired state-3 transition.
- Frozen behavioral test
  `crates/storage/tests/app012_approval_retention_red.rs` has SHA-256
  `e2f3ecd211ddd27dd7aad803bd95d9a9263873c8d61008d14367d9520f7111cc`.

Observable contract: resolved expired approvals (state 3) with
`resolved_at_us <= older_than_us` participate in both the backlog and bounded
sweep. Pending state 0 and rows newer than the cutoff remain untouched. Resource
rows retain existing `ON DELETE CASCADE` behavior. The implementation preserves
the existing two-parameter queries and sweep clamp of 1..=500.

Decision: add state `3` to the two existing terminal-state predicates only. No
schema, API, retention age, or resource-bound change.

## Candidate receipt

- Frozen test hash before and after implementation:
  `e2f3ecd211ddd27dd7aad803bd95d9a9263873c8d61008d14367d9520f7111cc`.
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p
  opencode-rk-storage --test app012_approval_retention_red --
  --test-threads=1`: 3 passed, 0 failed.
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p
  opencode-rk-storage --lib -- --test-threads=1`: 122 passed, 0 failed.
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p
  opencode-rk-storage --tests -- --test-threads=1`: 190 passed, 0 failed.
- `CARGO_BUILD_JOBS=1 cargo check -p opencode-rk-storage`: pass.
- `rustfmt --edition 2021 --check crates/storage/src/retention_v2.rs`: pass.
- `git diff --check`: pass; no Cargo, rustc, or focused-test process survived.

Remaining gap: `ApprovalsV2::expire_sweep` transitions rows to state `3` but
does not set `resolved_at_us`; such rows remain outside the age-based predicate.
The frozen test intentionally stamps a real state-3 row to isolate this task's
terminal-state predicate defect. Timestamping the production transition needs a
separate independently authored compiling RED and must keep the parent retention
journey open. This candidate does not claim to close that adjacent path.

Remaining authority: independent verifier and integration authority must review
and rerun this scoped candidate on the exact integrated revision. This is not
acceptance or release evidence.
