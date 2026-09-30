# G2 native provider live candidate

Base: `710a410e0ce32c716c40217696b515caa6c52310`.

This candidate uses the pinned upstream auth shape (`type: api`, `key`), the
existing Rust `OpenAiResponsesClient::from_persisted_env`, authenticated
daemon middleware, and the durable `/api/sessions/{id}/turns` engine.

The native `/connect` flow sends the provider key through an authenticated
daemon route, persists bounded auth JSON asynchronously with atomic replacement
and mode `0600`, preserves other entries, and uses the selected model in the
real authenticated turn request. No key is rendered or logged.

The provider auth route is `/api/auth/{provider}` and remains inside the
existing `/api/` bearer middleware. It accepts the protocol-shaped API payload
(`type`, `key`) and returns only `{ "success": true }`.

## Candidate-local validation

The focused native daemon-flow command completed with 4/4 GREEN. The frozen G2
provider fixture has not yet been run on a committed release candidate in this
handoff; parent must independently verify the exact candidate SHA with the
immutable fixture.

Known open limits: provider streaming remains a separate G5 concern; native
input remains byte-oriented without full UTF-8/escape/resize/mouse parity.
