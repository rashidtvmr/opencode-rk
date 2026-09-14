# AUTO-007

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-002, REQ-012.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: AUTO-007-T01, AUTO-007-T02, AUTO-007-T03, AUTO-007-T04, AUTO-007-T05.

## User-observable outcome

Turn submission state machine - submit to running to interrupted or complete as typed transitions with cancel cleanup.

## Source evidence

- codex protocol/src/protocol.rs:592, core/src/codex_thread.rs:82,320,332 typed Op over Thread; submit running interrupted complete.

## Observable contract

- Happy path, failure states, and resource bounds per test obligations AUTO-007-T01, AUTO-007-T02.
- No unbounded queue, no unbounded retained output; owner and cancel path defined.
