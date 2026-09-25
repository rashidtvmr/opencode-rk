# APP012-APPROVAL-RETENTION-RED — scratchpad

Claim: APP012-APPROVAL-RETENTION-RED, initially session
ses_f2843e480ffe4UDPtoLNjIlJJq and completed after lawful orchestrator reclaim
when that worker exited without a report or ledger transition.
Cwd/branch/HEAD: red-app012-approval-retention / red/APP012-APPROVAL-RETENTION / 7262682. Verified via git rev-parse.

Source evidence (exact):
- crates/storage/src/retention_v2.rs:87-106 `sweep_resolved_approvals` — `WHERE state IN (1, 2, 4)`, age `resolved_at_us <= ?1`, bounded LIMIT clamp 1..=500.
- crates/storage/src/retention_v2.rs:134-147 `retention_backlog` — same `state IN (1, 2, 4)` predicate.
- crates/storage/src/approvals_v2.rs:18-23 states: 0 pending, 1 allowed-once, 2 denied, 3 expired, 4 consumed. Expiry owned by `expire_sweep`.
- crates/storage/src/approvals_v2.rs:130-145 `expire_sweep` — sets `state = 3` where `state = 0 AND expires_at_us <= now`, bounded LIMIT clamp 1..=500. Does NOT stamp `resolved_at_us`.
- crates/storage/schema/v2/workspace.sql:223-240 `approvals` DDL, state 0..4, `resolved_at_us INTEGER` nullable.
- docs/storage/RISK-CLOSURE.md:17 retention row — sweeps cover terminal 1/2/4; state 3 omitted (gap).

Observed scenario: expired (state 3) approvals never participate in `retention_backlog` / `sweep_resolved_approvals`; they grow unbounded while 1/2/4 are swept per `resolved_at_us` age policy.

Target boundary: RED only. Own file: crates/storage/tests/app012_approval_retention_red.rs. No product/schema/lib/Cargo edits. Disposable SQLite via SchemaV2::initialize_workspace + tempdir. Public APIs only: SchemaV2, ApprovalsV2 (request/expire_sweep/add_resource), RetentionV2 (sweep_resolved_approvals/retention_backlog). Max 2 bound params per query, few rows, sweep LIMIT bounded.

Tests:
- T1 expired_state3_old_resolved_in_backlog: old state-3 (resolved 5) counted → expect backlog (0,1,0). Current: (0,0,0). FAILS (gap).
- T2 expired_state3_old_resolved_swept_with_cascade: sweep(50,500)==1, pending kept, resources cascaded. Current: sweep==0. FAILS (gap).
- T3 pending_and_too_new_expired_untouched: pending state 0 + too-new expired (resolved 1000) survive sweep(50). PASSES (guard).

State-3 fixture: real transition via ApprovalsV2::request (created 5, expires 100) + expire_sweep(100) → state 3, then bounded UPDATE stamps resolved_at_us (simulates resolved age per existing policy; expire_sweep currently leaves NULL). All columns real, no invented APIs.

Decisions: 3 tests (not 5; minimal behavioral RED for one predicate gap). Negative guard included. No network/clock/inherited env.

Remaining unknowns: GREEN fix shape (include state 3 in IN-list; decide age column for NULL-resolved expired rows — likely stamp resolved_at_us in expire_sweep). Verifier decides. No implementation attempted.

## RED receipt

- The original worker accidentally formatted tracked files throughout `crates/`,
  including frozen tests. Because this worktree was clean before delegation,
  every unauthorized tracked `crates/` diff was reversed before validation;
  only this new test, this scratchpad, and the own claim row remain.
- Focused command, run twice after restoring the tree:
  `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p
  opencode-rk-storage --test app012_approval_retention_red --
  --test-threads=1`.
- Both runs compiled and produced the same behavioral result: 1 passed, 2
  failed, 0 ignored. T1 observed `(0, 0, 0)` instead of `(0, 1, 0)`; T2
  observed sweep count `0` instead of `1`; the pending/too-new guard passed.
- Frozen SHA-256:
  `e2f3ecd211ddd27dd7aad803bd95d9a9263873c8d61008d14367d9520f7111cc`.
- After freezing: hash unchanged, `git diff --check` passed, and no Cargo,
  rustc, or focused-test process survived.
- This is a compiling deterministic RED candidate only. It does not authorize
  implementation, integration, or APP012 acceptance.
