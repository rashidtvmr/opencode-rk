# APP012-APPROVAL-EXPIRY-RED

## Claim and boundary

- Task: `APP012-APPROVAL-EXPIRY-RED`
- Session: `ses_f284cf14affeaJdLjqZWO1Mmnd`
- Branch: `red/APP012-APPROVAL-EXPIRY`
- Candidate: `7262682e31c1f473912c9483804c9430eb4ee4c5`
- Owned files: `crates/security/tests/app012_approval_expiry_red.rs`, this scratchpad, own ledger row.
- Test-author RED only. No product, manifest, lib, schema, or existing-test edits.

## Source evidence

- `crates/security/src/app_policy.rs:152-181`, `Grant::covers`, checks `expected.now > self.scope.expires_at`; equality currently remains valid.
- `crates/security/src/app_policy.rs:219-243`, `PolicyDeny::Expired` is the existing typed expiry denial.
- `crates/security/src/app_policy.rs:288-338`, `decide`, maps expired/stale/invalid-scope grant defects to `AppDecision::Deny`; `GrantLedger` is consumed only after `covers` succeeds.
- `crates/security/src/lib.rs:161-226`, `PermissionBroker::authorize` is the existing broker seam used by `decide`.
- `worklog/PHASE1-VERTICAL-SYNTHESIS.md:180-189`, lane contract requires equality expiry denial through existing `app_policy` public seam.

## Observable contract

For a digest- and scope-matching human grant:

1. `now < expires_at` covers and permits the protected delete operation once.
2. `now == expires_at` returns `Err(PolicyDeny::Expired)` from `Grant::covers`.
3. `decide` at equality returns `AppDecision::Deny { reason: "grant expired" }`.
4. Equality expiry leaves `GrantLedger` empty, proving no authorization-consumption side effect.
5. `now > expires_at` returns the same typed expiry denial.

Fixed `SystemTime` values avoid wall-clock dependence.

## RED evidence

Command:

```text
env CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-security --test app012_approval_expiry_red -- --nocapture
```

Result: compiled, then failed at `approval_expiry_is_exclusive_and_expired_grant_has_no_side_effect`, line 82. Current implementation returns `Ok(())` for `now == expires_at`; frozen assertion requires `Err(PolicyDeny::Expired)`. Failure is behavioral, not compile failure.

Initial import correction (`ApprovalId` is not re-exported by security) was made before RED freeze; final test compiles. Deterministic rerun: exit 101, 0 passed, 1 failed.

## Freeze

- Frozen test SHA-256: `a7f844388a6f0c5f2774f51c5989b9dfd70ed3a569d22e8d765ea5862b1542ea`.
- No test edits after hash freeze.
- Implementation/acceptance out of scope.

## Remaining unknowns

- Product implementation must change the comparison to make expiry exclusive. This lane does not edit `crates/security/src/app_policy.rs`.
- Integrator/verifier must rerun this exact test source and hash after implementation.
