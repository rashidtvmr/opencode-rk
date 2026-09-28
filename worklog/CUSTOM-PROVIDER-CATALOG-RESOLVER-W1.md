# CUSTOM-PROVIDER-CATALOG-RESOLVER-W1

## Claim
Task: CUSTOM-PROVIDER-CATALOG-RESOLVER-W1
Owner session: ses_sub_custom_provider_resolver
Owned file: crates/catalog/src/lib.rs (additive; preserve existing behavior)
Status: completed

## Source Evidence
- `crates/catalog/src/lib.rs` lines 1-185: existing Catalog, ModelsDevProvider, ModelsDevModel, ModelSummary, CatalogQuery
- `crates/cli/src/main.rs` commit 96b4a33: project_custom_providers() projects config.provider.{id} into provider objects with fields: name, id, npm, env, options{baseURL, headers, body}, models.{key: {name, id (wire)}}
- `crates/cli/tests/custom_provider_config_e2e.rs`: config schema with $schema, provider.{id}.{name, npm, env, options{baseURL, headers, body}, models{key: {name, id}}}
- `crates/contracts/src/lib.rs`: ProviderId (validated slug), ModelId (validated, max 512), ContractError types
- E2E config example: `{"provider":{"acme":{"name":"Acme fixture","npm":"@ai-sdk/openai-compatible","env":["ACME_PRIMARY_KEY","ACME_FALLBACK_KEY"],"options":{"baseURL":"http://...","headers":{"x-acme-config":"header-from-project-config"},"body":{"acme_extension":"body-from-project-config"}},"models":{"model-key":{"name":"Acme model","id":"wire-model"}}}`

## Target Boundary
- Bounded resolver in crates/catalog/src/lib.rs taking projected custom provider config Value producing typed CustomProviderConfig + CustomModelConfig
- Bounds: MAX_CUSTOM_ENV_VARS=16, MAX_CUSTOM_HEADERS=16, MAX_CUSTOM_BODY_BYTES=8192, MAX_BASE_URL_BYTES=256, MAX_NPM_LEN=128, MAX_MODEL_WIRE_ID_LEN=512
- Fail closed on: missing name, non-object provider config, non-string fields, non-array env, env entries not valid ProviderId slugs, excess env/headers, oversized baseURL/npm/wire-id/body bytes
- body retains arbitrary serde_json::Value types with depth/container bounds (MAX_BODY_DEPTH=8, MAX_BODY_NODES=512, MAX_CUSTOM_CONTAINER_ITEMS=128, MAX_VALUE_BYTES=512)
- Do NOT read env credentials, expose secrets via ModelSummary, infer pricing, discover endpoints, accept unbounded objects
- Preserve existing Catalog behavior (from_models_dev_api_json, model, search)

## RED Tests (frozen - 6 tests, crs_t01..crs_t06)
1. crs_t01: resolve_valid_custom_provider - resolves name, npm, env, baseURL, headers, body, models
2. crs_t02: model_wire_id_mapping - model wire id from "id" field, defaults to model key
3. crs_t03: fail_closed_malformed - non-object provider, missing name, non-string fields reject
4. crs_t04: bounded_env_and_headers - env capped at MAX_CUSTOM_ENV_VARS, excess rejected
5. crs_t05: no_secret_leak_in_summary - ModelSummary excludes env/header/body/npm/base_url secrets
6. crs_t06: body_bounded_bytes - oversized body rejected at MAX_CUSTOM_BODY_BYTES

## RED Hash (before GREEN)
- File: crates/catalog/src/lib.rs at RED: 8e7199f50566b87584051d1bda35ba7f8f3af52d7f2f3f96dff752cddae5bb84

## GREEN Results
- All 43 tests pass: cargo test -p opencode-rk-catalog --lib (jobs=1, threads=1)
- cargo fmt --check: clean
- body field type: BTreeMap<String, serde_json::Value> (retains arbitrary JSON types)
- Fixed: provider_config receives full config {"acme": {...}}, extracts by provider_id key
- Fixed: baseURL made optional (unwrap_or("")), empty string valid for empty options {}
- ModelSummary excludes: npm, env_candidates, base_url, headers, body - only identity + capability flags

## E2E
- Frozen custom-provider E2E at crates/cli/tests/custom_provider_config_e2e.rs
- Expected: parent RED at server credential adapter; catalog resolver visible/functional

## Remaining
- Parent task CUSTOM-PROVIDER-CONFIG-INGEST-W1 blocked on unwired RuntimeWiring resolver
- Resolver does NOT wire into server/turn adapter
