# PROV-019

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-041.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: PROV-019-T01, PROV-019-T02, PROV-019-T03, PROV-019-T04, PROV-019-T05.
Ownership locks: crates/providers/src/request_profile.rs only; worker never edits shared lib.rs, Cargo.toml, schemas, migrations.
Suggested module: crates/providers/src/request_profile.rs (new module in crate opencode-rk-providers; lib.rs wiring left to integrator).

## User-observable outcome

Documented provider request profiles: every request declares explicit provider identity, sends only documented auth headers, and uses bounded retries/timeouts with redacted wire diagnostics. CLI impersonation, detection evasion, undocumented endpoints, and spoofed headers are prohibited by construction.

## Source evidence

- crates/providers/src/config.rs:7-22 ProviderConfig (provider_id, base_url, api_key_env, timeout_secs, max_tokens, temperature).
- crates/providers/src/registry.rs for provider/model routing (Provider, ProviderRegistry, priority ordering).
- crates/providers/src/model_route.rs for provider/model routing (ModelRoute, compatible-provider prefixes, MAX_COMBO_MODELS).
- crates/providers/src/auth.rs:6-18 AuthMethod; :40-101 AuthHandler and in-memory refresh.
- crates/providers/src/oauth_flow.rs:1-65 planner validates HTTPS metadata only and performs no network.
- crates/providers/src/integration.rs:1-5 explicitly stops before credential storage/external OAuth; :11-18 bounded constants.
- crates/providers/src/lib.rs:4-48 provider modules (no request_profile yet).
- docs/SECURITY.md:14-32 brokered permissions, no direct secret access/unrestricted env.
- docs/research/REQ-041-provider-compatibility-research.md:1 REQ-041 official provider compatibility and CLI authentication scope.

## Observable contract

- `RequestProfile { provider_id: String, endpoint: String, auth_headers: Vec<AuthHeaderKind>, timeout_ms: u64, max_retries: u32 }`: auth_headers uses kind enum (Bearer, ApiKeyHeader) never raw secrets; endpoint must be https and allowlisted per provider.
- `profile_for(provider_id, endpoint_kind) -> Result<RequestProfile, RequestError>`: resolves documented endpoint plus required headers from the compatibility catalog; unknown provider or endpoint yields error.
- `headers_for(profile) -> Vec<(String, RedactedMarker)>`: returns header names with redaction markers only; secret values never produced by this API.
- `with_diagnostics(profile, attempt) -> RedactedDiagnostic { provider_id, endpoint, attempt, error_code: Option }`: wire diagnostics carry identity and codes only, zero header values.
- Bounds: timeout_ms 1_000..=120_000 default 30_000; max_retries 0..=3 default 1; endpoint bytes max 2048.
- Prohibitions enforced by API shape: no user-agent spoof field, no undocumented-endpoint escape hatch, no raw-header constructor; spoof/undocumented inputs rejected.
- Deterministic: same provider plus endpoint yields byte-identical profile; no I/O, no network, no clock.
- Suggested module boundary: crates/providers/src/request_profile.rs owning RequestProfile, AuthHeaderKind, RequestError, RedactedDiagnostic, profile_for, headers_for, with_diagnostics; shared lib.rs wiring left to integrator.

## Failure states

- Unknown provider: `Err(RequestError::UnknownProvider)`; no profile returned.
- Undocumented endpoint: `Err(RequestError::UndocumentedEndpoint)`; no profile returned.
- Spoofed or custom raw headers requested: `Err(RequestError::HeaderNotAllowed)`; never emitted.
- Non-HTTPS endpoint: `Err(RequestError::BadEndpoint)`; never emitted.
- Over-bound timeout/retries: `Err(RequestError::BadBounds)`; defaults documented in error.
- Secret safety: profiles, diagnostics, errors, and logs contain header names only, never values. No live credentials in tests; disposable fixtures only.

## Resource bounds

- Pure library: no Command, no thread, no I/O, no network; caller-owned profile values only.
- Bounded: max 8 auth headers per profile, endpoint max 2048 bytes, retries max 3, timeout capped.
- No unbounded queue, no unbounded retained output; owner and cancel path defined per crates/providers/src/integration.rs:11-18 bounded-constants precedent.

## Acceptance criteria

- PROV-019-T01: openai happy path: profile_for openai chat-completions yields https endpoint, Bearer kind, bounded timeout/retries; headers_for returns names with redaction markers and no secret bytes.
- PROV-019-T02: anthropic happy path: profile_for anthropic messages yields https endpoint plus documented auth header kinds; diagnostics carry provider and endpoint with zero header values.
- PROV-019-T03: prohibitions: undocumented endpoint yields UndocumentedEndpoint; raw-header request yields HeaderNotAllowed; non-https endpoint yields BadEndpoint.
- PROV-019-T04: bounds: timeout 0 and retries 99 yield BadBounds; unknown provider yields UnknownProvider; errors carry codes only.
- PROV-019-T05: determinism and redaction: repeated profile_for byte-identical; Debug plus serialize of profiles and diagnostics contain no fixture secrets; no network touched.

## Test-first execution

- RED: author tests PROV-019-T01..T05 against crates/providers/src/request_profile.rs; establish compiling RED (fail: no request_profile module).
- GREEN: implement minimum native Rust documented-profile builder with prohibitions; GREEN, refactor, rerun; negative tests (undocumented, spoof, bad-endpoint, bad-bounds, leak scan).
- Evidence: `cargo test -p opencode-rk-providers request_profile`; `cargo check --workspace`; frozen test hash plus command manifest; verifier decides acceptance.
