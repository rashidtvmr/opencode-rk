# ROUTE-012

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-018.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: ROUTE-012-T01, ROUTE-012-T02, ROUTE-012-T03, ROUTE-012-T04, ROUTE-012-T05.

## User-observable outcome

Cheap-model chores routing - quota, topic, title, summarize go to cheapest capable model, fail-open.

## Source evidence

- claude-code-reverse README.md:84-154 quota probe Haiku max_tokens=1, topic check, title, summaries on cheapest model.

## Observable contract

- Happy path, failure states, and resource bounds per test obligations ROUTE-012-T01, ROUTE-012-T02.
- No unbounded queue, no unbounded retained output; owner and cancel path defined.
