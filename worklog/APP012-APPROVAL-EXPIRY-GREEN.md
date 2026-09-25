# APP012-APPROVAL-EXPIRY-GREEN scratchpad

claim: APP012-APPROVAL-EXPIRY-GREEN, initially session
ses_f2801ed1effe7zeEP3xCHot9L5 and completed after orchestrator reclaim when
that worker exited without validation, a ledger transition, commit, or report.
base: branch lane/APP012-APPROVAL-EXPIRY-GREEN @ 7d90cdd (verified pwd/rev-parse)

source evidence:
- crates/security/src/app_policy.rs:160 `Grant::covers` expiry check `if expected.now > self.scope.expires_at` — inclusive at boundary (bug; RED froze it)
- crates/security/src/app_policy.rs:291-339 `decide` — Expired maps to Deny, no ledger consume (already correct, preserved)
- crates/security/src/app_policy.rs:222-226 `PolicyDeny::Expired` typed reason (preserved)
- frozen test crates/security/tests/app012_approval_expiry_red.rs sha256 a7f844388a6f0c5f2774f51c5989b9dfd70ed3a569d22e8d765ea5862b1542ea

observed scenario: RED lane APP012-APPROVAL-EXPIRY-RED completed — equality boundary (now == expires_at) wrongly covered; before/after correct.

target boundary: own exactly crates/security/src/app_policy.rs. Minimum fix: `>` → `>=` in covers. Preserve before-boundary Allow, decide Deny + empty-ledger on expired, all other policy behavior. No stubs/deps/shared edits.

tests: frozen target app012_approval_expiry_red; regressions security lib/tests; cargo check; formatting baseline comparison; hashes before/after; diff.

decisions: one-char fix; native, no new code paths.
remaining: independent verifier and integration authority must review and rerun
on the exact integrated revision.

## Candidate receipt

- The original GREEN worker ran workspace formatting and changed the frozen RED.
  Because the worktree was clean before delegation, all tracked `crates/` changes
  were reversed. The sole intended product change (`>` to `>=` in
  `Grant::covers`) was then reapplied manually. The frozen test was restored to
  SHA-256 `a7f844388a6f0c5f2774f51c5989b9dfd70ed3a569d22e8d765ea5862b1542ea`.
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p
  opencode-rk-security --test app012_approval_expiry_red -- --test-threads=1`:
  1 passed, 0 failed.
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p
  opencode-rk-security --lib -- --test-threads=1`: 148 passed, 0 failed.
- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p
  opencode-rk-security --tests -- --test-threads=1`: all affected approval tests
  passed; one unrelated existing macOS path-spelling assertion failed in
  `sandbox_enforcement::scenario_b_deny_write_outside_allowed_roots` because the
  implementation canonicalized `/var/...` to `/private/var/...`. Running that
  exact test on untouched RED revision `7d90cdd` reproduces the same failure.
- `CARGO_BUILD_JOBS=1 cargo check -p opencode-rk-security`: pass (two pre-existing
  unused-import warnings).
- `rustfmt --check crates/security/src/app_policy.rs` reports pre-existing
  formatting differences identically on untouched revision `7d90cdd`; the
  one-character semantic patch itself introduces no formatting change.
- `git diff --check`: pass; no Cargo/rustc/test process survived.
- This is scoped GREEN evidence only, not acceptance or release evidence.
