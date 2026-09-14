# AGENT-031

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-013.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: AGENT-031-T01, AGENT-031-T02, AGENT-031-T03, AGENT-031-T04, AGENT-031-T05.

## User-observable outcome

Plan mode plus structured review child - plan permission mode with Enter and Exit tools, review returns machine-checkable findings.

## Source evidence

- claude-code EnterPlanMode/ExitPlanMode tools plus plan permission mode; codex turn_input.rs:133 ReviewDecision accept revise.

## Observable contract

- Happy path, failure states, and resource bounds per test obligations AGENT-031-T01, AGENT-031-T02.
- No unbounded queue, no unbounded retained output; owner and cancel path defined.
