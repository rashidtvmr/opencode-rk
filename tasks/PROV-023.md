# PROV-023

Status: NOT STARTED. Kind: docs. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-041.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: PROV-023-T01, PROV-023-T02, PROV-023-T03, PROV-023-T04, PROV-023-T05.
Ownership locks: docs/provider-compatibility.json only; worker never edits product code, schemas, or migrations.
Suggested module: docs/provider-compatibility.json (new versioned catalog file; validation test owns schema checks).

## User-observable outcome

Versioned documentation-backed provider compatibility catalog: per provider lists source dates, documented endpoints, required auth headers, model aliases, refresh behavior, and limitations. Users and PROV-019 profiles resolve only documented entries.

## Source evidence

- crates/providers/src/config.rs:7-22 ProviderConfig (provider_id, base_url, api_key_env, timeout_secs).
- crates/providers/src/registry.rs for provider/model routing (Provider, ProviderRegistry, priority ordering).
- crates/providers/src/model_route.rs for provider/model routing (ModelRoute, compatible-provider prefixes, MAX_COMBO_MODELS).
- crates/providers/src/oauth_flow.rs:1-65 planner validates HTTPS metadata only and performs no network.
- crates/providers/src/integration.rs:1-5 explicitly stops before credential storage/external OAuth; :11-18 bounded constants.
- crates/providers/src/lib.rs:4-48 provider modules.
- docs/SECURITY.md:14-32 brokered permissions, no direct secret access/unrestricted env.
- docs/research/REQ-041-provider-compatibility-research.md:1 REQ-041 official provider compatibility and CLI authentication scope.

## Observable contract

- `catalog_version: "1"` plus `updated: YYYY-MM-DD` at top level; consumers reject missing version.
- `providers[]`: each `{ id, display_name, doc_source, doc_date, endpoints[]: { kind, url_template (https), auth_headers[] }, model_aliases[], refresh: { supported, method }, limitations[] }`.
- Minimum coverage: openai, anthropic, plus one additional supported provider; every endpoint https-only; every auth header a documented name (never a secret value).
- `lookup(catalog, provider_id, endpoint_kind) -> entry | null`: exact id match; unknown yields null, never a guessed endpoint.
- Bounded: providers max 32, endpoints per provider max 16, aliases per provider max 32, file max 256 KiB.
- Deterministic: canonical key order and sorted arrays; same content yields byte-identical file.
- Suggested file boundary: docs/provider-compatibility.json owning the catalog; a small Rust or Python validation test reads and checks it without network.

## Failure states

- Missing version or date: validation fails with `bad-version`; catalog rejected by consumers.
- Non-HTTPS endpoint: validation fails with `bad-endpoint`; entry rejected.
- Unknown provider lookup: returns null; callers surface UnknownProvider, never synthesize endpoints.
- Over-cap file (providers/endpoints/bytes): validation fails with `over-cap`; catalog rejected whole, never partially applied.
- Secret safety: catalog holds header names and URL templates only; validation scans file for secret-like values (`sk-`, `Bearer ` plus token) and fails closed on match. No live credentials anywhere.

## Resource bounds

- Static file max 256 KiB; validation reads once, O(entries); no network, no clock, no retained state.
- No unbounded queue, no unbounded retained output; owner is the docs tree, no runtime cost.

## Acceptance criteria

- PROV-023-T01: schema valid: JSON parses, version is "1", updated is a valid date, openai and anthropic entries present with https endpoints and named auth headers.
- PROV-023-T02: model aliases: each provider lists at least one alias; PROV-019-style lookup of openai chat-completions and anthropic messages resolves documented URLs.
- PROV-023-T03: unknown lookup: lookup of unknown provider or endpoint kind yields null; no guessed URL present in file.
- PROV-023-T04: prohibitions and caps: file contains no "undocumented", no spoof header names, no secret values; counts within caps and bytes within 256 KiB.
- PROV-023-T05: determinism: canonical re-serialization byte-identical; validation test passes offline with no network.

## Test-first execution

- RED: author tests PROV-023-T01..T05 as catalog validation checks; establish compiling RED (fail: no docs/provider-compatibility.json).
- GREEN: write minimum versioned catalog meeting schema and caps; GREEN, refactor, rerun; negative tests (bad-version, bad-endpoint, secret-scan, over-cap).
- Evidence: catalog validation test passes; `git status --short -- docs/provider-compatibility.json tasks/PROV-023.md`; frozen test hash plus command manifest; verifier decides acceptance.
