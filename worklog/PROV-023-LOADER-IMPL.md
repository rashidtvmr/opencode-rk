# PROV-023 bounded runtime catalog loader implementation

## Claim and boundary

- Candidate branch: `lane/PHASE1-product-spine-20260923`.
- Frozen loader RED integrated at `fefe6e1`; implementation claim at `2028af2`.
- Frozen test: `crates/providers/tests/prov_023_loader_red.rs`.
- Frozen SHA-256: `d6aa3a01ad4e6a4d09c148672d29dbc608c739e5bbf7cb574f8b9ad16becc2d8`.
- Product file: `crates/providers/src/request_profile.rs`. No test/catalog/controller file was edited.

## Source evidence and implementation

- `request_profile::profile_for_with_options` previously called a two-provider hardcoded `documented_entry`; Google in the versioned catalog returned `UnknownProvider` and configured invalid catalogs were ignored.
- The runtime now embeds the exact checked-in `docs/provider-compatibility.json` by default. `OPENCODE_RK_PROVIDER_CATALOG_PATH`, when explicitly configured and nonempty, replaces that input; missing or invalid configured input fails closed and never falls back.
- Reads are capped at 256 KiB before JSON parsing. The complete version-1 schema is deserialized with unknown fields denied and validated before any lookup.
- Validation bounds providers (32), endpoints/provider (16), aliases/provider (32), limitations/provider (32), auth headers/endpoint (8), identifiers (128 bytes), descriptive fields (4 KiB), and endpoints (2 KiB). Duplicate provider IDs, endpoint kinds, aliases, and auth headers fail closed.
- Dates require `YYYY-MM-DD` shape with month 1..12 and day 1..31. IDs use bounded lowercase ASCII/digit/dash spellings.
- URLs require HTTPS, a host, no userinfo, no fragment, no credential-like query keys, and only the documented `{model=models/*}` placeholder. Unknown providers/endpoints remain typed and no URL is guessed.
- Auth headers map only documented names to the existing secret-free kinds. Secret-looking raw catalog bytes (`sk-`, `sk-ant-`, `Bearer `) are rejected before deserialization and never appear in errors.
- The loader performs no network access and retains no unbounded state.

## Verification

All Cargo commands used `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1` and serial test threads.

- Frozen loader target: 21 passed, 0 failed.
- Existing PROV-019 request profile: 5 passed, 0 failed.
- Existing PROV-023 runtime catalog: 4 passed, 0 failed.
- Existing PROV-023 static catalog: 5 passed, 0 failed.
- Providers library: 73 passed, 0 failed.
- `git diff --check`: passed.
- Frozen SHA-256 re-read unchanged.

Pre-existing warnings remain in unrelated provider/security files.

## Remaining blocker

This is a GREEN candidate, not parent acceptance. Repository search previously found no production transport caller of `request_profile::profile_for`; the public request-profile boundary is now catalog-backed, but a real provider transport must consume it before claiming end-to-end user routing. Independent verification is also pending. PROV-024 remains separately blocked on its own fixture/root contract.
