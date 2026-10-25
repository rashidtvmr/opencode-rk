# Independent Functional Review — Server Stream Error Fidelity

**Package:** context-controls-004 / server stream error fidelity (P2)
**Base:** `36daa6f1`
**Review scope:** diff-first review of the one-line fallback change in
`crates/server/src/lib.rs`, against frozen
`crates/server/tests/session_turn_stream_api.rs` (no tests/build executed).

## Verdict: APPROVE

The change replaces the generic `provider_stream_failed` fallback with
`bad_gateway` only when a `ResponsesError::StreamFailed` code is not one of
the recognized transport codes. The existing mappings for `bad_gateway`,
`service_unavailable`, `timeout`, and both `rate_limit` spellings remain
unchanged. Code-less/unknown provider stream failures therefore produce the
same coherent API category as the frozen failure journey.

The provider-error branch emits the error and transitions directly to `Done`;
it does not call assistant persistence. The assistant is persisted only after
a terminal completed event. Existing bounded/redacted message behavior is
outside this one-line change and remains unchanged.

No test execution claim is made; parent should run the focused test.

## Exact two-file SHA-256 manifest

```text
b6118642e0bafb3f03ce54f7673a3601aef9e108ccbb3c1fc200e1c5d07c013c  crates/server/src/lib.rs
886cb9967a8861b7d66ed5f1d50700bc6a10272f27c619c64d6b8d92bd6129df  crates/server/tests/session_turn_stream_api.rs
```
