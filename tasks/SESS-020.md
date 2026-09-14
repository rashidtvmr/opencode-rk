# SESS-020

Status: IMPLEMENTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-006.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: SESS-020-T01, SESS-020-T02, SESS-020-T03, SESS-020-T04, SESS-020-T05.

## User-observable outcome

Auto-compact thresholds plus warning states plus 3-strike circuit breaker with env kill-switches.

## Source evidence

- claude-code src/services/compact/autoCompact.ts thresholds AUTOCOMPACT_BUFFER 13000, WARNING ERROR 20000, MANUAL 3000, MAX_CONSECUTIVE_FAILURES=3.

## Observable contract

- Happy path, failure states, and resource bounds per test obligations SESS-020-T01, SESS-020-T02.
- No unbounded queue, no unbounded retained output; owner and cancel path defined.
