# PROV-015

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-041.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: PROV-015-T01, PROV-015-T02, PROV-015-T03, PROV-015-T04, PROV-015-T05.
Ownership locks: crates/providers/src/auth_profile.rs only; worker never edits shared lib.rs, Cargo.toml, schemas, migrations.
Suggested module: crates/providers/src/auth_profile.rs (new module in crate opencode-rk-providers; lib.rs wiring left to integrator).

## User-observable outcome

Provider auth profile carrying explicit provenance (API key, official OAuth, imported local credential, keyring). Status and diagnostics show provider, auth mode, and provenance; tokens are never logged or serialized.

## Source evidence

- crates/providers/src/auth.rs:6-18 AuthMethod (ApiKey, BearerToken, OAuth2 with access_token, refresh_token, expires_at).
- crates/providers/src/auth.rs:40-101 AuthHandler and in-memory refresh (add_auth, get, validate, refresh).
- crates/providers/src/config.rs:7-22 ProviderConfig (provider_id, base_url, api_key_env, timeout_secs, max_tokens, temperature).
- crates/providers/src/lib.rs:4-48 provider modules (auth, config, registry, oauth_flow declared; no auth_profile yet).
- crates/providers/src/oauth_flow.rs:1-65 planner validates HTTPS metadata only and performs no network.
- crates/providers/src/integration.rs:1-5 explicitly stops before credential storage/external OAuth; :11-18 bounded constants.
- docs/SECURITY.md:14-32 brokered permissions, no direct secret access/unrestricted env; :62-72 human authority for OAuth.
- docs/research/REQ-041-provider-compatibility-research.md:1 REQ-041 official provider compatibility and CLI authentication scope.

## Observable contract

- `AuthProvenance { ApiKey, OfficialOAuth, ImportedLocal, Keyring }`: explicit source of every credential; constructed only with provenance stated.
- `AuthProfile { provider_id: String, provenance: AuthProvenance, method: AuthMethod-kind without secret bytes }`: identity plus non-secret shape only.
- `profile_of(provider_id, provenance, method_kind) -> Result<AuthProfile, AuthProfileError>`: rejects empty provider id and unknown method kind; never accepts raw token bytes.
- `describe(profile) -> AuthDescription { provider_id, provenance, auth_mode, has_credentials: bool }`: status-safe view; contains zero secret material.
- `redacted_debug(profile) -> String`: Debug/Display output contains provider id, provenance, and mode only; secret bytes never appear.
- Serialization of AuthProfile/AuthDescription emits provenance and mode only; token fields are not serializable by construction (no Serialize on secret holders, custom redacted impl where needed).
- Deterministic: same inputs yield byte-identical description; no wall-clock, no I/O, no network.
- Suggested module boundary: crates/providers/src/auth_profile.rs owning AuthProvenance, AuthProfile, AuthDescription, AuthProfileError, profile_of, describe; shared lib.rs wiring left to integrator.

## Failure states

- Empty provider id: `Err(AuthProfileError::EmptyProvider)`; no profile returned.
- Unknown method kind: `Err(AuthProfileError::UnknownMethod)`; no profile returned.
- Token bytes passed where kind expected: rejected at type level; constructor takes kind enum, never String secrets.
- Debug/Serialize output containing token-like bytes: test failure, treated as secret leak.
- Secret safety: nothing secret is logged; no file bodies, no credentials in errors. No SQLite/OpenCode DB writes. No changes to the user's existing OpenCode database; tests use disposable in-memory profiles only.

## Resource bounds

- Pure library: no Command spawn, no thread, no I/O, no network, no retained state beyond caller-owned profile value.
- Bounded strings: provider id 1..=128 chars; longer rejected with `TooLong`.
- No unbounded queue, no unbounded retained output; owner and cancel path defined per crates/providers/src/integration.rs:11-18 bounded-constants precedent.

## Acceptance criteria

- PROV-015-T01: api-key profile happy path: `profile_of("openai", ApiKey, ApiKeyKind)` returns provenance ApiKey and mode api-key; `describe` shows provider openai, has_credentials true, zero secret bytes.
- PROV-015-T02: provenance preserved: one profile per provenance (OfficialOAuth, ImportedLocal, Keyring) round-trips provenance through describe; all four provenances distinct in output.
- PROV-015-T03: no-log no-serialize: Debug and serde_json output of profile and description contain provider id and provenance but contain neither fixture token bytes nor substrings "sk-" or "refresh".
- PROV-015-T04: validation failures: empty provider id yields EmptyProvider; unknown method yields UnknownMethod; over-128-char id yields TooLong; each leaves no partial profile.
- PROV-015-T05: determinism and isolation: repeated profile_of/describe calls byte-identical; no filesystem or network touched (assert no files created outside disposable test dir, no DB path writes).

## Test-first execution

- RED: author tests PROV-015-T01..T05 against crates/providers/src/auth_profile.rs; establish compiling RED (fail: no auth_profile module).
- GREEN: implement minimum native Rust profile/provenance/redaction; GREEN, refactor, rerun; negative tests (empty/unknown/too-long, leak scan).
- Evidence: `cargo test -p opencode-rk-providers auth_profile`; `cargo check --workspace`; frozen test hash plus command manifest; verifier decides acceptance.
