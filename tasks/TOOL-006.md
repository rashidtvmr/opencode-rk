# TOOL-006

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-037.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: TOOL-006-T01, TOOL-006-T02, TOOL-006-T03, TOOL-006-T04, TOOL-006-T05.

## User-observable outcome

Typed tool contract via buildTool factory - schema plus permission plus exec plus render in one definition.

## Source evidence

- claude-code src/Tool.ts:704,717,783 buildTool factory; ~40 tool dirs each with Tool plus prompt plus constants plus UI.

## Observable contract

- Happy path, failure states, and resource bounds per test obligations TOOL-006-T01, TOOL-006-T02.
- No unbounded queue, no unbounded retained output; owner and cancel path defined.
