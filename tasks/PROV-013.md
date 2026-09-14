# PROV-013

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-038.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: PROV-013-T01, PROV-013-T02, PROV-013-T03, PROV-013-T04, PROV-013-T05.

## User-observable outcome

Provider-boundary tap with redaction - uid-keyed structured records at request, response, stream-final, error seam.

## Source evidence

- claude-code-reverse cli.js.patch:145-176 tap at beta.messages.create, uid input output error lines; parser.js:163-231 line protocol.

## Observable contract

- Happy path, failure states, and resource bounds per test obligations PROV-013-T01, PROV-013-T02.
- No unbounded queue, no unbounded retained output; owner and cancel path defined.
