# TOOL-009

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-037.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: TOOL-009-T01, TOOL-009-T02, TOOL-009-T03, TOOL-009-T04, TOOL-009-T05.

## User-observable outcome

ToolSearch discovery - just-in-time tool listing to keep lean context.

## Source evidence

- claude-code ToolSearchTool for just-in-time tool discovery.

## Observable contract

- Happy path, failure states, and resource bounds per test obligations TOOL-009-T01, TOOL-009-T02.
- No unbounded queue, no unbounded retained output; owner and cancel path defined.
