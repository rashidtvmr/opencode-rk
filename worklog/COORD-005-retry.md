# COORD-005 worklog (retry lane): serialized integration gate

Claim: new file tools/completion_integration.py implements T01..T05 gate.

Source evidence (HEAD 5af7884):
- tools/ralph_loop.py:421-448 finalize_worktree commits lane branch, ff-only
  merge into mainline, keeps branch on diverged/ff failure.
- tools/ralph_loop.py:601-608 record_result keeps receipt + last_error, no
  unintegrated acceptance.
- tools/completion_scheduler.py:61-64 Integration dataclass
  (candidate_revision/integrated_revision/integrated).
- tools/completion_scheduler.py:189-203 finalize serializes
  verify->integrate->verify_integrated; RetryableFailure becomes Rejected
  (reconcile, never blind re-run).
- tasks/completion/delivery.json COORD-005 tests T01..T05, paths
  [tools/completion_integration.py].

Observed: finalize_worktree runs git merge --ff-only without timeout,
ancestry probe, lock, or atomic receipt; no written integration module
existed (ls confirmed missing before create).

Target boundary: OWNED FILE ONLY tools/completion_integration.py.
No other file edited.

Tests (module self-check, real git fixtures in tmp repos, all green):
- T02 clean worktree + unintegrated branch raises Rejected, not no-op.
- T03 diverged nonconflicting branch kept (tip intact, mainline intact,
  no receipt), then rebased + integrated serially, both files present.
- T01 conflicting merge failure: Rejected, no receipt, reconcile=diverged.
- T04 failing post-merge rerun: Rejected, no receipt, despite prior verify.
- T05 passing rerun: atomic receipt binds integrated rev + frozen hash +
  rerun=passed; reread matches; idempotent on same binding; conflicting
  binding raises.
- Ambiguity: fabricated revision fails closed; reconcile read-only
  (mainline tip unchanged); lock serializes second holder
  (RetryableFailure on contention).

Decisions:
- List-form git argv only, no shell. GIT_TIMEOUT=10.0 for ancestry probe
  and all git calls (10s ceiling).
- IntegrationLock = threading guard + fcntl file lock (O_EXCL fallback),
  bounded wait, RetryableFailure on contention.
- Receipt: temp write + fsync + os.replace + dir fsync, 64 KiB cap,
  conflicting record raises.
- integrate_and_accept runs rerun hook outside git lock; rerun may be
  retried, merge never blindly retried (reconcile only reads).

Remaining unknowns: external concurrency harness binds this module via
COORD-008; out of scope here.
