# Server stream-error fidelity candidate handoff

Package: current observed server failure (`crates/server/tests/session_turn_stream_api.rs::session_turn_stream_reports_provider_failure_without_persisting_assistant`)

Base: `43f5c8ccc403e90187ff8d5a45b6be67a6034e78`

Owned candidate artifact (not applied here): `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/server-stream-error-fidelity.patch`

Preimage SHA-256: `3a932b9e11bd6341682a9b9789d17d5fc3096bc44a2584fe33c4127f052b02cf` (`crates/server/src/lib.rs`)

Observed RED: the fixture emits `response.failed` with no provider error code and message `fixture stream failed`; the test requires SSE `code == "bad_gateway"`, but the current fallback emits `provider_stream_failed`.

Exact source change: in `provider_stream_code` (`crates/server/src/lib.rs:3239-3248`), change the catch-all `ResponsesError::StreamFailed` result from `provider_stream_failed` to `bad_gateway`. Existing recognized typed mappings (`bad_gateway`, `service_unavailable`, `timeout`, `rate_limit`/`rate_limited`) remain unchanged. The provider parser already bounds and control-character-filters the message (`crates/providers/src/responses.rs:521-540`), so the SSE retains the typed code/message without exposing raw payload.

Upstream evidence: pinned checkout is `sources/upstream.lock.json` commit `95daf90670b7c039c436c85537da5fbfe2205b41`; its OpenAI websocket treats `response.failed` as a terminal provider failure (`packages/opencode/src/plugin/openai/ws.ts:263-267`).

Verification: not run per delegation. Parent should apply the artifact and run the frozen focused test.

Parent integration: applied the one-line patch on `36daa6f1`, leaving the
existing regression source unchanged at SHA-256
`886cb9967a8861b7d66ed5f1d50700bc6a10272f27c619c64d6b8d92bd6129df`.
`cargo test -p opencode-rk-server --test session_turn_stream_api` passed both
tests with jobs/threads 1 and toolchain 1.98.1; actual output is retained in
`server-stream-error-fidelity-green.log`. Independent review approved the
exact source/test hashes. This is focused candidate evidence, not installed
release acceptance.
