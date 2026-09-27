# CUSTOM-PROVIDER-CLIENT-W1

Status: in progress; implementation candidate is intentionally partial pending the separate server-caller lane.

## Claim and scope
- Claimed as `CUSTOM-PROVIDER-CLIENT-W1` by session `ses_f1e6e3ab4ffeG84rY5B3BEoBaT`.
- Candidate starts at frozen parent RED commit `434cddebed7a1d5c21965b395fb0d8c687a395ba`, branch `lane/CUSTOM-PROVIDER-CLIENT-W1`.
- Own product file: `crates/providers/src/responses.rs` only. No server/test/controller edits.

## Evidence and contract
- `crates/providers/src/responses.rs`, `OpenAiResponsesClient::from_env` (baseline lines 455-474): hardcoded `ProviderConfig::from_env("openai")`, validates, rejects blank credentials, disables redirects and applies bounded timeout/token limits.
- `crates/providers/src/config.rs`, `ProviderConfig::from_env` / `validate` (lines 84-150): provider prefixed environment settings and empty ID validation; custom IDs receive default config.
- Add `from_env_for(id)` preserving these guarantees; `from_env()` remains the OpenAI compatibility alias. Credential bytes remain private and missing-credential diagnostics identify only the configured env variable.
- Client has no persistence or long-lived external task ownership; instance owns its reqwest client and credential string, released on drop. Request timeout remains clamped 1..300 seconds, max output tokens 1..65536, redirects disabled. Payload/body existing caps unchanged.

## Security / convergence
- No direct secret-file access, no unrestricted env reads beyond `ProviderConfig`'s existing named variables. Tests use synthetic credentials only; no logs or credential formatting.
- `python3 tools/convergence_gate.py` ran at baseline and is blocked by 84 existing off-plan/parent-note findings including off-plan completed tasks. This lane is not permission to alter controller state.
- Per delegation, parent custom-provider live journey remains RED until independent server wiring. Therefore this worker will report blocked/partial, not completed.

## Tests and decisions
- Added source-local unit coverage for custom-provider config, missing credential and empty ID. No frozen parent test modified.
- Pending test/format evidence.
