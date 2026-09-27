# CUSTOM-PROVIDER-LIVE-W1 — RED: custom provider ID turns through HTTP API

## Claim
- Task ID: CUSTOM-PROVIDER-LIVE-W1
- Session: ses_f1eb9be44ffeXKOHfj92SpOLQM
- Status: in-progress
- Route verified: @vyce-deepseek-v41 (DeepSeek V4.1 via VyceAI direct)

## Source Evidence (current code)
- `crates/server/src/lib.rs:920-924`: `create_turn` rejects any provider != "openai":
  ```rust
  if provider_id != "openai" {
      return Err(ApiFailure::bad_request(format!(
          "provider '{provider_id}' does not have a native turn adapter yet"
      )));
  }
  ```
- `crates/server/src/lib.rs:1142-1145`: `create_turn_stream` rejects any provider != "openai":
  ```rust
  if provider_id != "openai" {
      return Err(ApiFailure::bad_request(format!(
          "provider '{provider_id}' does not have a native turn adapter yet"
      )));
  }
  ```
- `crates/providers/src/responses.rs:455-456`: `OpenAiResponsesClient::from_env()` hardcodes "openai":
  ```rust
  pub fn from_env() -> Result<Self, ResponsesError> {
      let config = ProviderConfig::from_env("openai");
  ```
- `crates/providers/src/config.rs:84-115`: `ProviderConfig::from_env(provider_id)` reads `{PROVIDER_ID}_BASE_URL`, `{PROVIDER_ID}_API_KEY_ENV`, etc.
- `crates/server/tests/runtime_wiring_http.rs`: pattern for axum oneshot tests with Router + ServiceExt.
- `crates/server/tests/loop_driver_live.rs`: pattern for scripted TCP provider fixture + spawn_http + stream_turn.

## Observable Contract
1. Distinct custom provider ID (e.g. `free/glm-5.3-flash`) not rejected as "no native turn adapter".
2. Authenticated bounded model inventory with `free/glm-5.3-flash`.
3. Selected Responses turn through real HTTP API persists assistant message.
4. 402 sends exactly one upstream request, no replay.
5. Absent key / malformed endpoint fail closed.
6. Fake secret absent from response / log URL.

## Target Boundary
- Owned file: `crates/server/tests/custom_provider_live.rs` (NEW)
- No product code edits, no frozen test edits, no controller/state edits.
- Bounded loopback fake OpenAI-compatible server fixture, fake key only.

## Test Plan (RED)
1. `custom_provider_distinct_from_openai`: Verify provider ID `free` is not the same as `openai` — assert a custom provider client can be constructed targeting a loopback fixture with env vars set.
2. `custom_provider_authenticated_model_inventory`: Authenticated request to `/api/models?provider=free` returns model inventory including `glm-5.3-flash`.
3. `custom_provider_turn_persists_assistant`: POST `/api/sessions/{id}/turns` with model `free/glm-5.3-flash` to a loopback OpenAI-compatible fixture returns 201 and persists assistant message.
4. `custom_provider_402_single_request_no_replay`: 402 Payment Required from upstream is sent exactly once, no retry/replay occurs.
5. `custom_provider_absent_key_fails_closed`: Empty API key env var results in failure, no request to upstream.
6. `custom_provider_malformed_endpoint_fails_closed`: Malformed base_url results in error, no request to upstream.
7. `custom_provider_secret_not_in_response`: API key never appears in HTTP response body or log lines.

## Remaining Unknowns
- Whether `OpenAiResponsesClient::from_env()` needs a `from_env_for(provider_id)` variant — this is the implementation gap to prove RED.
- Whether the server turn handlers need to resolve provider config dynamically — this is the core RED behavior.

---

## Session 2 — EXECUTION-FIRST TEST-AUTHOR (2026-09-26)

- Session: ses_f1e83487affeDZ3TOh0BBq8kiY
- Route: @xkiro-dsv41-flash-free (xkiro/deepseek/deepseek-v4.1-flash:free)
- Branch: red/CUSTOM-PROVIDER-LIVE-W1, base ecec045
- Prior workers only analyzed; required file was absent; claim reclaimed.

### Artifact
- NEW `crates/server/tests/custom_provider_live.rs` (owned, only product-adjacent file)
- 3 bounded real-HTTP tests against a fake loopback OpenAI-compatible TCP provider.
- Env-sensitive tests serialized through a process-wide `ENV_LOCK` mutex.

### Source evidence (verified on disk)
- `crates/server/src/lib.rs:920-924` — `create_turn` rejects `provider_id != "openai"` with 400.
- `crates/server/src/lib.rs:1142-1145` — `create_turn_stream` same guard.
- `crates/providers/src/responses.rs:455-456` — `OpenAiResponsesClient::from_env()` hardcodes `ProviderConfig::from_env("openai")`.
- `crates/providers/src/config.rs:84-115` — `ProviderConfig::from_env(provider_id)` reads `{PROVIDER_ID}_BASE_URL` / `{PROVIDER_ID}_API_KEY_ENV`; unset key => `MissingCredential` from `responses.rs:461`.

### Commands / results
- `cargo test -p opencode-rk-server --test custom_provider_live --no-run` => exit 0 (compiles).
- `cargo test -p opencode-rk-server --test custom_provider_live` => **RED, genuine**:
  - `custom_provider_free_model_reaches_adapter_and_persists` — 400 != 201, upstream requests seen: 0
  - `custom_provider_upstream_402_is_single_request_without_replay` — recorded 0 != 1
  - `custom_provider_absent_credential_fails_closed_without_upstream` — 400 != 503
  - `test result: FAILED. 0 passed; 3 failed`
- Frozen SHA-256: `fab95fcd61d3ef268b9a9045633cb8918e90ec19f4289b06ad42400c1456dc7f`
  (`shasum -a 256 crates/server/tests/custom_provider_live.rs` on this revision)

### Blockers (RED-only, exact)
1. Server guard at `lib.rs:920` (and `lib.rs:1142`) rejects any non-`openai` provider before any adapter is resolved.
2. `OpenAiResponsesClient::from_env()` cannot target a custom provider id; a `from_env_for(provider_id)` (or provider-resolved config) variant is required so `FREE_BASE_URL`/`FREE_API_KEY` are honored.
3. The 503-vs-400 mapping for a missing credential is not implemented on the custom-provider path.

Implementation lane must fix product code; this test file is frozen and must not be edited.
