# PROV-005

Status: IN PROGRESS. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-038.
Dependencies: none.
Test obligations: PROV-005-T01, PROV-005-T02, PROV-005-T03, PROV-005-T04, PROV-005-T05.

## User-observable outcome

Rate limiter for API providers enforcing per-provider request quotas via a sliding window algorithm.

## Source evidence

- crates/providers/src/rate_limit.rs (stub)

## Observable contract

- Happy path, failure states, and resource bounds per test obligations.
- check returns true when tokens remain, false when exhausted.
- Tokens refill after the configured window elapses.
- remaining reflects decrement after a check.
- reset clears per-provider state.

## Test obligations

### PROV-005-T01

check_returns_true_when_empty - first request to a provider with capacity > 0 returns true.

### PROV-005-T02

check_returns_false_when_empty - request beyond capacity returns false.

### PROV-005-T03

refills_after_window - tokens are restored after the window duration elapses.

### PROV-005-T04

remaining_decreases - remaining token count decrements after a successful check.

### PROV-005-T05

reset_clears_state - reset removes the provider entry from internal state.
