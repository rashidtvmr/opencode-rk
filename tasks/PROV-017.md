# PROV-017

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-041.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: PROV-017-T01, PROV-017-T02, PROV-017-T03, PROV-017-T04, PROV-017-T05.
Ownership locks: crates/providers/src/claude_oauth.rs only; worker never edits shared lib.rs, Cargo.toml, schemas, migrations.
Suggested module: crates/providers/src/claude_oauth.rs (new module in crate opencode-rk-providers; lib.rs wiring left to integrator).

## User-observable outcome

Official Anthropic Claude Code OAuth connector with PKCE/loopback consent, token refresh, logout, and status; authenticated requests route through the native provider path. Consent is explicit and human-driven; tokens never logged.

## Source evidence

- crates/providers/src/auth.rs:6-18 AuthMethod (OAuth2 with access_token, refresh_token, expires_at).
- crates/providers/src/auth.rs:40-101 AuthHandler and in-memory refresh (validate checks expiry, refresh swaps access token).
- crates/providers/src/oauth_flow.rs:1-65 planner validates HTTPS metadata only and performs no network.
- crates/providers/src/integration.rs:1-5 explicitly stops before credential storage/external OAuth; :11-18 bounded constants.
- crates/providers/src/config.rs:7-22 ProviderConfig (provider_id, base_url, api_key_env, timeout_secs).
- crates/providers/src/lib.rs:4-48 provider modules (no claude_oauth yet).
- crates/providers/src/registry.rs and model_route.rs for provider/model routing.
- docs/SECURITY.md:14-32 brokered permissions, no direct secret access/unrestricted env; :62-72 human authority for OAuth.
- docs/research/REQ-041-provider-compatibility-research.md:1 REQ-041 official provider compatibility and CLI authentication scope.

## Observable contract

- `ClaudeAuthState { LoggedOut, PendingConsent { consent_url, pkce_challenge }, Ready { expires_at_ms }, Expired }`: explicit lifecycle; PKCE challenge is public metadata, tokens never inside status values.
- `begin_login(pkce) -> PendingConsent`: https-only loopback consent URL plus caller-supplied PKCE challenge; no token material created.
- `complete_login(state, grant: HumanGrant) -> Ready`: consumes explicit human grant only; fabricated grants rejected.
- `refresh(state, grant: RefreshGrant) -> Ready`: bounded single refresh; updates expiry only.
- `logout(state) -> LoggedOut`: clears token references; status shows logged-out with zero secret bytes.
- `status(state) -> ClaudeStatus { provider: "anthropic-claude-code", auth_mode: "official-oauth", expires_at_ms: Option, error: Option }`: redacted, serializable, deterministic.
- `route_target() -> "anthropic-claude-code"`: native provider routing; no CLI impersonation, no spoofed headers, no undocumented endpoints.
- Pure state machine: no network, no I/O, no clock reads inside transitions (caller supplies now_ms); deterministic given same inputs.
- Suggested module boundary: crates/providers/src/claude_oauth.rs owning ClaudeAuthState, HumanGrant, ClaudeStatus, begin/complete/refresh/logout/status; shared lib.rs wiring left to integrator.

## Failure states

- Missing human grant on complete_login: `Err(ClaudeError::ConsentRequired)`; state stays PendingConsent.
- Non-HTTPS or non-loopback consent URL: `Err(ClaudeError::BadConsentUrl)`; never emitted.
- Empty PKCE challenge: `Err(ClaudeError::BadPkce)`; no state change.
- Refresh when LoggedOut: `Err(ClaudeError::NotLoggedIn)`; no state change.
- Secret safety: status/debug/serialize outputs contain zero token bytes; errors carry codes only. No SQLite/OpenCode DB writes. Tests use disposable in-memory state and fake grant issuers only.

## Resource bounds

- Pure library: no Command, no thread, no I/O, no network; single state value retained by caller.
- Bounded strings: consent URL max 2048 bytes, PKCE challenge 43..=128 bytes; over-limit rejected.
- No unbounded queue, no unbounded retained output; owner and cancel path defined per crates/providers/src/integration.rs:11-18 bounded-constants precedent.

## Acceptance criteria

- PROV-017-T01: consent happy path: begin_login yields https loopback URL plus PKCE echo; complete_login with fake human grant yields Ready; status shows provider anthropic-claude-code and mode official-oauth.
- PROV-017-T02: refresh and logout: expired state plus refresh grant yields Ready with later expiry; logout yields LoggedOut with no expiry and no secret bytes.
- PROV-017-T03: validation failures: empty PKCE yields BadPkce; missing grant yields ConsentRequired; refresh while LoggedOut yields NotLoggedIn; state unchanged on each error.
- PROV-017-T04: redaction: Debug plus serde_json of every state and status contain neither fixture token bytes nor "sk-ant-" nor "refresh_token" substrings.
- PROV-017-T05: determinism and isolation: same grant sequence yields byte-identical status; no network, no files outside disposable test dir, no DB writes.

## Test-first execution

- RED: author tests PROV-017-T01..T05 against crates/providers/src/claude_oauth.rs; establish compiling RED (fail: no claude_oauth module).
- GREEN: implement minimum native Rust PKCE-consent/refresh/logout/status machine; GREEN, refactor, rerun; negative tests (bad-pkce, no-grant, bad-url, leak scan).
- Evidence: `cargo test -p opencode-rk-providers claude_oauth`; `cargo check --workspace`; frozen test hash plus command manifest; verifier decides acceptance.
