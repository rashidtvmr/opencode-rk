# PROV-002

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes. Requirements: REQ-038.
Dependencies: none. Test obligations: PROV-002-T01, PROV-002-T02, PROV-002-T03, PROV-002-T04, PROV-002-T05.

## User-observable outcome

Provider configuration system with individual configs and sets for managing provider settings via environment variables.

## Source evidence

- crates/providers/src/auth.rs:20-28 ProviderAuth struct pattern
- crates/providers/src/registry.rs:6-20 Provider struct pattern with base_url, api_key_env, timeout_secs
- crates/foundation/src/lib.rs:45-62 validate() returning Result<(), String> pattern

## Observable contract

- ProviderConfig with provider_id, base_url, api_key_env, timeout_secs, max_tokens, temperature
- load_default() returns configured defaults based on provider_id
- from_env(provider_id) loads from environment variables
- validate() returns Ok(()) for valid configs, Err(String) for invalid
- ProviderConfigSet manages a HashMap of configs with add, get, merge, to_json

## Test obligations

- PROV-002-T01: default_config - load_default returns valid config
- PROV-002-T02: from_env_loads - from_env reads environment variables
- PROV-002-T03: validate_valid - validate returns Ok for valid config
- PROV-002-T04: validate_invalid_url - validate returns Err for invalid URL
- PROV-002-T05: merge_combines - merge combines configs correctly