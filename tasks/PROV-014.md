# PROV-014

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-038.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: PROV-014-T01, PROV-014-T02, PROV-014-T03, PROV-014-T04, PROV-014-T05.

## User-observable outcome

Debug export bundle - one redacted JSONL with session meta, deduped defs, turns, timings, errors; offline renderer.

## Source evidence

- claude-code-reverse README.md:59-64 parser.js plus viewer pipeline; offline redacted JSONL bundle, dependency-free renderer.

## Observable contract

- Happy path, failure states, and resource bounds per test obligations PROV-014-T01, PROV-014-T02.
- No unbounded queue, no unbounded retained output; owner and cancel path defined.
