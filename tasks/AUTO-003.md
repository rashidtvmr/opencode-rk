# AUTO-003 - Worktree leases and heartbeats

Status: IMPLEMENTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-002.
Dependencies: AUTO-001, AUTO-002 controller bootstrap/trusted pipeline.
Test obligations: AUTO-003-T01, AUTO-003-T02, AUTO-003-T03, AUTO-003-T04, AUTO-003-T05.

## User-observable outcome

The autonomous controller gives each running task/worktree a durable lease with an owner and heartbeat. A live lease cannot be stolen, a stale lease can be reclaimed after its expiry, only the owning lane may renew/release it, and the heartbeat lifecycle stops when the lane finishes.

## Source evidence

- `PLAN.md:202-220`: autonomous state/receipts live outside worker sandboxes; production AUTO work adds worktree leases and heartbeats, remains bounded, and records blockers rather than spinning forever.
- `prompts/START_HERE.md:18-30`: workers use isolated worktrees; hardened leased parallel execution must preserve strict TDD and safe unattended stop behavior.
- `tools/ralph_loop.py:12-19`: current controller creates isolated worktrees, runs bounded workers, persists resumable state and append-only receipts.
- `tools/ralph_loop.py:397`: explicit implementation note: `AUTO-003 upgrades leases/heartbeats`.
- Pinned OpenCode commit `95daf90670b7c039c436c85537da5fbfe2205b41`, `packages/core/src/util/flock.ts:24-67,127-146,148-271,274-345`: bounded owner-token leases, heartbeat, stale recovery, cancellation-aware waiting, and owner-checked release provide behavioral reference semantics.
- Pinned `packages/core/test/util/flock.test.ts:117-183,185-242,315-390`: contention/timeout, crashed-owner recovery, owner metadata, and token-mismatch release coverage.

## Observable contract

- Lease acquisition records task id, owner token, acquired/heartbeat time, and expiry in controller-owned state.
- A non-expired lease owned by another lane is not claimable.
- An expired lease is reclaimable deterministically by a new owner.
- Heartbeat and release are owner-checked; stale/non-owner operations fail without mutating the lease.
- Lease/heartbeat work is bounded by the configured concurrent-lane limit and the heartbeat owner is joined/stopped when the lane finishes.

## Safety and lifetime

- No worker can edit controller lease state directly; controller owns lease files outside the worktree.
- No unbounded queue or detached heartbeat thread; heartbeat lifetime is scoped to one lane.
- Stdlib only, no new dependency or network behavior.
