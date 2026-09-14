# SESS-019

Status: IMPLEMENTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-008.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: SESS-019-T01, SESS-019-T02, SESS-019-T03, SESS-019-T04, SESS-019-T05.

## User-observable outcome

Revert is free pointer move distinct from truncating rollback with separate API names.

## Source evidence

- codex fork vs revert vs rollback split: branch copy, pointer rewind, truncating rollback as separate API names.

## Observable contract

- Happy path, failure states, and resource bounds per test obligations SESS-019-T01, SESS-019-T02.
- No unbounded queue, no unbounded retained output; owner and cancel path defined.
