# Server stream-error fidelity candidate handoff

Package: current observed server failure (`crates/server/tests/session_turn_stream_api.rs::session_turn_stream_reports_provider_failure_without_persisting_assistant`)

Base observed failure: `8bb45ef5b` (parent-reported final source). Local source was verified against the requested current tree before editing.

Owned candidate artifact (not applied here): `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/server-stream-error-fidelity.patch`

Pre-edit SHA-256 (source): `b6118642e0bafb3f03ce54f7673a3601aef9e108ccbb3c1fc200e1c5d07c013c` (`crates/server/src/lib.rs`). Pre-edit frozen target hashes: `session_turn_stream_api.rs` `886cb9967a8861b7d66ed5f1d50700bc6a10272f27c619c64d6b8d92bd6129df`; `session_child_transient_retry.rs` `75923464c8c666dd31246a9ad9685cf5474bb9d0b43f4813aeead02e2cf3e439`; `subagent_builtin_lifecycle.rs` `f988643ed9ae73ea5e7952acba9079e61202a57633144dc7799d78deff9cb38e`.

Observed RED: the fixture emits `response.failed` with no provider error code and message `fixture stream failed`; the test requires SSE `code == "bad_gateway"`, but the current fallback emits `provider_stream_failed`.

Exact source change: in `provider_stream_code` (`crates/server/src/lib.rs:3239-3250`), preserve explicit `bad_request`, map only the provider parser's code-less `"unknown"` sentinel to `bad_gateway`, and retain `provider_stream_failed` for other unrecognized typed codes. This keeps permanent bad-request failures non-transient while satisfying the code-less stream-failure contract. The provider parser already bounds and control-character-filters the message (`crates/providers/src/responses.rs:521-540`), so the SSE retains the typed code/message without exposing raw payload.

Upstream evidence: pinned checkout is `sources/upstream.lock.json` commit `95daf90670b7c039c436c85537da5fbfe2205b41`; its OpenAI websocket treats `response.failed` as a terminal provider failure (`packages/opencode/src/plugin/openai/ws.ts:263-267`).

Verification command: `CARGO_TARGET_DIR=target/native-usability-candidate-20261024-0542 CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 DYLD_LIBRARY_PATH=target/native-usability-candidate-20261024-0542/debug/deps cargo +1.98.1 test -p opencode-rk-server --test session_turn_stream_api --test session_child_transient_retry --test subagent_builtin_lifecycle`. Result: PASS, 8+2+7 tests; exact log SHA-256 `d29d6f4b31ec878d496c52658b10be54f073836bb99476b22f13748e57437305`. Post-edit source SHA-256: `d5cd494974006490eaf25ace825a6cc06526af12e986cd9fef6e87f8ff625127`. No tests were modified.

Parent integration: applied the one-line patch on `36daa6f1`, leaving the
existing regression source unchanged at SHA-256
`886cb9967a8861b7d66ed5f1d50700bc6a10272f27c619c64d6b8d92bd6129df`.
`cargo test -p opencode-rk-server --test session_turn_stream_api` passed both
tests with jobs/threads 1 and toolchain 1.98.1; actual output is retained in
`server-stream-error-fidelity-green.log`. Independent review approved the
exact source/test hashes. This is focused candidate evidence, not installed
release acceptance.

## Completion evidence readback

Actual verified log path:
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/server-stream-error-fidelity-focused-exact.log`

Readback SHA-256: `d29d6f4b31ec878d496c52658b10be54f073836bb99476b22f13748e57437305`.
The log contains 17 passing tests: 8 `session_child_transient_retry`, 2
`session_turn_stream_api`, and 7 `subagent_builtin_lifecycle`. No Cargo command
was rerun during this evidence correction. Actual source SHA-256 remains
`d5cd494974006490eaf25ace825a6cc06526af12e986cd9fef6e87f8ff625127`.

The exact command and target are recorded above. Frozen RED target hashes
actually recorded before repair (not a separate manifest file) are:
`session_turn_stream_api.rs` `886cb9967a8861b7d66ed5f1d50700bc6a10272f27c619c64d6b8d92bd6129df`;
`session_child_transient_retry.rs` `75923464c8c666dd31246a9ad9685cf5474bb9d0b43f4813aeead02e2cf3e439`;
`subagent_builtin_lifecycle.rs` `f988643ed9ae73ea5e7952acba9079e61202a57633144dc7799d78deff9cb38e`.
No standalone frozen RED manifest was created; this handoff does not claim one.
