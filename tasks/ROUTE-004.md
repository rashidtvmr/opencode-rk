# ROUTE-004 - Account recovery lock cleanup

Status: IMPLEMENTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: none (DISC-002 discovered routing/account-storage surface).
Dependencies: ROUTE-003 account failure lock planning.
Test obligations: ROUTE-004-T01, ROUTE-004-T02, ROUTE-004-T03, ROUTE-004-T04, ROUTE-004-T05.

## User-observable outcome

After a successful provider request, routing deterministically plans cleanup of the successful model's lock, any account-wide lock, and all expired locks while preserving active unrelated model locks; error/backoff state resets only when no active lock remains.

## Source evidence and decomposition

- Pinned 9router commit `17c4cc76877bd1755030a8414f8d0083f48dcccf`, `src/sse/services/auth.js:289-337`: `clearAccountError` selects locks to clear, retains active unrelated locks, and resets error state only when no active locks remain.
- Pinned `tests/unit/fetch-success-clears-account.test.js:74-95`: a successful fetch invokes account-error cleanup for the successful model.
- Pinned `open-sse/services/accountFallback.js:105-160`: account-wide/model lock naming and clear semantics.
- `sources/behavior-surface-rules.json` jointly nominates ROUTE-003/004 for the account lifecycle surfaces. This card explicitly assigns the paired **success -> lock cleanup/recovery** operation to ROUTE-004 after ROUTE-003's failure planning; the split is documented source-audit decomposition, not numeric-id inference.

## Observable contract

- Missing/`noauth` ids and already-clean state are no-ops.
- A successful model clears that model's lock plus the account-wide lock; expired locks are lazily cleared regardless of model.
- Active unrelated model locks remain intact.
- Error/test/backoff fields reset only when no active lock remains after planned clears.
- Caller lock input is explicitly bounded and overflow returns a typed error.

## Ownership and safety

- Pure cleanup planning. Caller owns lock/error state, `now`, and persistence of the returned patch.
- No DB/network/environment/wall-clock/background work.
- Cleanup work and returned clear-list are bounded by a small public maximum frozen by tests.
