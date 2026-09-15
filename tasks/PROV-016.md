# PROV-016

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-041.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: PROV-016-T01, PROV-016-T02, PROV-016-T03, PROV-016-T04, PROV-016-T05.
Ownership locks: crates/providers/src/codex_oauth.rs only; worker never edits shared lib.rs, Cargo.toml, schemas, migrations.
Suggested module: crates/providers/src/codex_oauth.rs (new module in crate opencode-rk-providers; lib.rs wiring left to integrator).

## User-observable outcome

Official OpenAI Codex OAuth connector with browser/device consent, token refresh, logout, and status; authenticated requests route through the native provider path. Consent is explicit and human-driven; tokens never logged.

## Source evidence

- crates/providers/src/auth.rs:6-18 AuthMethod (OAuth2 with access_token, refresh_token, expires_at).
- crates/providers/src/auth.rs:40-101 AuthHandler and in-memory refresh (validate checks expiry, refresh swaps access token).
- crates/providers/src/oauth_flow.rs:1-65 planner validates HTTPS metadata only and performs no network.
- crates/providers/src/integration.rs:1-5 explicitly stops before credential storage/external OAuth; :11-18 bounded constants.
- crates/providers/src/config.rs:7-22 ProviderConfig (provider_id, base_url, api_key_env, timeout_secs).
- crates/providers/src/lib.rs:4-48 provider modules (no codex_oauth yet).
- crates/providers/src/registry.rs and model_route.rs for provider/model routing.
- docs/SECURITY.md:14-32 brokered permissions, no direct secret access/unrestricted env; :62-72 human authority for OAuth.
- docs/research/REQ-041-provider-compatibility-research.md:1 REQ-041 official provider compatibility and CLI authentication scope.

## Observable contract

- `CodexAuthState { LoggedOut, PendingConsent { consent_url, device_code }, Ready { expires_at_ms }, Expired }`: explicit lifecycle; tokens held by caller-owned store, never inside status values.
- `begin_login() -> PendingConsent { consent_url: https-only, device_code }`: starts browser/device consent; no token material created.
- `complete_login(state, grant: HumanGrant) -> Ready`: consumes explicit human grant only; fabricated grants rejected.
- `refresh(state, grant_or_refresh: RefreshGrant) -> Ready`: bounded single refresh; updates expiry only.
- `logout(state) -> LoggedOut`: clears token references; status shows logged-out with zero secret bytes.
- `status(state) -> CodexStatus { provider: "openai-codex", auth_mode: "official-oauth", expires_at_ms: Option, error: Option }`: redacted, serializable, deterministic.
- `route_target() -> "openai-codex"`: native provider routing; no CLI impersonation, no spoofed headers, no undocumented endpoints.
- Pure state machine: no network, no I/O, no clock reads inside transitions (caller supplies now_ms); deterministic given same inputs.
- Suggested module boundary: crates/providers/src/codex_oauth.rs owning CodexAuthState, HumanGrant, CodexStatus, begin/complete/refresh/logout/status; shared lib.rs wiring left to integrator.

## Failure states

- Missing human grant on complete_login: `Err(CodexError::ConsentRequired)`; state stays PendingConsent.
- Non-HTTPS consent URL: `Err(CodexError::BadConsentUrl)`; never emitted.
- Refresh when LoggedOut: `Err(CodexError::NotLoggedIn)`; no state change.
- Expired without refresh grant: status shows Expired with error, never auto-fabricates tokens.
- Secret safety: status/debug/serialize outputs contain zero token bytes; errors carry codes only. No SQLite/OpenCode DB writes. Tests use disposable in-memory state and fake grant issuers only.

## Resource bounds

- Pure library: no Command, no thread, no I/O, no network; single state value retained by caller.
- Bounded strings: consent URL max 2048 bytes, device code max 128 bytes; over-limit rejected.
- No unbounded queue, no unbounded retained output; owner and cancel path defined per crates/providers/src/integration.rs:11-18 bounded-constants precedent.

## Acceptance criteria

- PROV-016-T01: consent happy path: begin_login yields https consent URL and device code; complete_login with fake human grant yields Ready with expires_at set; status shows provider openai-codex and mode official-oauth.
- PROV-016-T02: refresh and logout: expired state plus refresh grant yields Ready with later expiry; logout yields LoggedOut and status with no expiry and no secret bytes.
- PROV-016-T03: consent required: complete_login without grant yields ConsentRequired and state unchanged; refresh while LoggedOut yields NotLoggedIn.
- PROV-016-T04: redaction: Debug plus serde_json of every state and status contain neither fixture token bytes nor "sk-" nor "refresh_token" substrings.
- PROV-016-T05: determinism and isolation: same grant sequence yields byte-identical status; no network, no files outside disposable test dir, no DB writes.

## Test-first execution

- RED: author tests PROV-016-T01..T05 against crates/providers/src/codex_oauth.rs; establish compiling RED (fail: no codex_oauth module).
- GREEN: implement minimum native Rust consent/refresh/logout/status machine; GREEN, refactor, rerun; negative tests (no-grant, bad-url, refresh-logged-out, leak scan).
- Evidence: `cargo test -p opencode-rk-providers codex_oauth`; `cargo check --workspace`; frozen test hash plus command manifest; verifier decides acceptance.
