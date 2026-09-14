# TOOL-007

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-017.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: TOOL-007-T01, TOOL-007-T02, TOOL-007-T03, TOOL-007-T04, TOOL-007-T05.

## User-observable outcome

Skill-gated approvals - skill invocation passes the same policy gate as tools.

## Source evidence

- codex skills/invocation.rs:14 plus loading.rs:16,33; skills pass policy gate before run.

## Observable contract

- Happy path, failure states, and resource bounds per test obligations TOOL-007-T01, TOOL-007-T02.
- No unbounded queue, no unbounded retained output; owner and cancel path defined.
