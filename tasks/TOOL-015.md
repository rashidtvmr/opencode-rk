# TOOL-015

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-037.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: TOOL-015-T01, TOOL-015-T02, TOOL-015-T03, TOOL-015-T04, TOOL-015-T05.

## User-observable outcome

MCP tool own policy gate plus elicitation path with timeout and cancel.

## Source evidence

- codex mcp_tool_call.rs:14,58 plus mcp_policy.rs:6,52; MCP calls carry own policy gate plus elicitation path.

## Observable contract

- Happy path, failure states, and resource bounds per test obligations TOOL-015-T01, TOOL-015-T02.
- No unbounded queue, no unbounded retained output; owner and cancel path defined.
