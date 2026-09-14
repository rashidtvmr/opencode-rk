# ROUTE-003 - Account failure lock planning

Status: IMPLEMENTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: none (DISC-002 discovered routing/account-storage surface).
Dependencies: ROUTE-001 account eligibility.
Test obligations: ROUTE-003-T01, ROUTE-003-T02, ROUTE-003-T03, ROUTE-003-T04, ROUTE-003-T05.

## User-observable outcome

Provider failures produce a deterministic caller-owned account-unavailable update plan: ordinary failures lock only the failing model, GitHub monthly premium exhaustion locks the whole account until the supplied next-month reset, precise provider resets override generic backoff, and persisted error text is bounded.

## Source evidence and decomposition

- Pinned 9router commit `17c4cc76877bd1755030a8414f8d0083f48dcccf`, `src/sse/services/auth.js:229-287`: `markAccountUnavailable` selects cooldown source, scopes the model/account lock, truncates the reason, updates unavailable/error/backoff state, and returns the fallback decision.
- Pinned `src/sse/services/auth.js:12-18`: GitHub status 402 plus the monthly-usage message is the account-wide special case.
- Pinned `open-sse/services/accountFallback.js:105-150`: model-lock key semantics and caller-time lock expiry construction.
- Pinned `tests/unit/github-monthly-usage-lock.test.js:31-85`: monthly GitHub exhaustion is account-wide while unrelated GitHub 402 remains model-scoped.
- `sources/behavior-surface-rules.json` jointly nominates ROUTE-003/004 for routing + account-storage + token-refresh. The ledger does not preassign the two adjacent lifecycle operations, so this card explicitly decomposes **failure -> unavailable/lock planning** into ROUTE-003; ROUTE-004 owns the paired success/expired-lock cleanup. This is a source-audit decision, not semantics inferred from the numeric id.

## Observable contract

- Missing/`noauth` connection ids produce a no-op decision.
- GitHub monthly premium exhaustion uses the supplied next-UTC-month reset, an account-wide lock, and resets backoff to zero.
- Other fallback-eligible failures use a model-scoped lock when a model is known and preserve the generic fallback planner's cooldown/backoff decision.
- A future precise reset overrides generic fallback cooldown; non-Antigravity precise cooldown is capped to the pinned maximum while Antigravity preserves the exact reset.
- Persisted failure reason is bounded to the pinned 100-character limit; the update records status/error/backoff and caller-supplied `now` without doing persistence itself.

## Ownership and safety

- Pure planning only. Caller supplies `now`, generic fallback outcome, and the next-month reset timestamp used by the GitHub special case.
- No DB/network/environment/wall-clock/background work and no retry execution.
- Lock deadlines use checked/saturating arithmetic; retained error text is explicitly bounded.
