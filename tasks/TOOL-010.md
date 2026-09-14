# TOOL-010

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-021, REQ-026.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: TOOL-010-T01, TOOL-010-T02, TOOL-010-T03, TOOL-010-T04, TOOL-010-T05.

## User-observable outcome

request_permissions mid-turn tool - agent asks for a named capability with scope, rate-limited.

## Source evidence

- codex protocol.rs:804 InterAgentCommunication op, permissions.rs:71,226 request_permissions tool, safety.rs:17,29.

## Observable contract

- Happy path, failure states, and resource bounds per test obligations TOOL-010-T01, TOOL-010-T02.
- No unbounded queue, no unbounded retained output; owner and cancel path defined.
