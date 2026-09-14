# SESS-018

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-008.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: SESS-018-T01, SESS-018-T02, SESS-018-T03, SESS-018-T04, SESS-018-T05.

## User-observable outcome

Fork boundary is typed - branch copy only on fork, cache cleared at boundary.

## Source evidence

- codex thread-store/src/types.rs:185,196,205,217 typed fork boundary.

## Observable contract

- Happy path, failure states, and resource bounds per test obligations SESS-018-T01, SESS-018-T02.
- No unbounded queue, no unbounded retained output; owner and cancel path defined.
