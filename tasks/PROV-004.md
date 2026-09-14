# PROV-004

Status: IN PROGRESS. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-007.
Dependencies: none.
Test obligations: PROV-004-T01, PROV-004-T02, PROV-004-T03, PROV-004-T04, PROV-004-T05.

## User-observable outcome

Streaming session handling for LLM provider responses: buffered per-session event history with a start/emit/end lifecycle.

## Source evidence

- crates/providers/src/streaming.rs: current implementation.
- crates/providers/src/health.rs: sibling module pattern (module + inline test module).

## Observable contract

- StreamEvent enum: Data(String), Done, Error(String)
- StreamingHandler struct: sessions HashMap<String, Vec<StreamEvent>>, pending_tokens Vec<String>
- Methods: start_session(id), emit(id, event), end_session(id), stream(id) -> Vec<StreamEvent>
- start_session creates an empty buffer; duplicate starts are idempotent and do not re-pend.
- emit appends an event, creating the session on demand.
- end_session appends Done once (idempotent; Error also counts as terminal) and removes the id from pending_tokens.
- stream returns a copy of the full buffered history; unknown ids return an empty Vec.

## Test obligations

- PROV-004-T01: start_and_stream - start creates an empty session and marks it pending.
- PROV-004-T02: emit_data - emitted Data events are returned in order.
- PROV-004-T03: end_completes - end appends Done and clears pending.
- PROV-004-T04: multiple_sessions - sessions are isolated by id.
- PROV-004-T05: stream_empty_after_end - ended empty session yields single Done; unknown ids stream empty.

## Verification

- cargo test -p opencode-rk-providers (5 streaming tests pass)
- cargo check --workspace clean