# PROV-020

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-041.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: PROV-020-T01, PROV-020-T02, PROV-020-T03, PROV-020-T04, PROV-020-T05.
Ownership locks: crates/providers/src/auth_commands.rs only; worker never edits shared lib.rs, Cargo.toml, schemas, migrations.
Suggested module: crates/providers/src/auth_commands.rs (new module in crate opencode-rk-providers; lib.rs wiring left to integrator).

## User-observable outcome

Auth connector commands and status surface: connect, login, import, list, inspect, refresh, and logout. Status shows provider, account, auth mode, expiry, and errors; secrets never shown.

## Source evidence

- crates/providers/src/auth.rs:6-18 AuthMethod; :40-101 AuthHandler and in-memory refresh.
- crates/providers/src/oauth_flow.rs:1-65 planner validates HTTPS metadata only and performs no network.
- crates/providers/src/integration.rs:1-5 explicitly stops before credential storage/external OAuth; :11-18 bounded constants.
- crates/providers/src/config.rs:7-22 ProviderConfig (provider_id, base_url, api_key_env, timeout_secs).
- crates/providers/src/lib.rs:4-48 provider modules (no auth_commands yet).
- crates/providers/src/registry.rs and model_route.rs for provider/model routing.
- docs/SECURITY.md:14-32 brokered permissions, no direct secret access/unrestricted env; :62-72 human authority for OAuth.
- docs/research/REQ-041-provider-compatibility-research.md:1 REQ-041 official provider compatibility and CLI authentication scope.

## Observable contract

- `AuthCommand { Connect { provider }, Login { provider }, Import { provider, source }, List, Inspect { provider }, Refresh { provider }, Logout { provider } }`: closed command enum; no raw-secret variant exists.
- `dispatch(cmd, state) -> AuthEffect { status: AuthStatus, needs_consent: bool }`: pure transition planner; consent-gated commands report needs_consent true and change nothing until grant arrives.
- `AuthStatus { provider, account_label, auth_mode, provenance, expires_at_ms: Option, error: Option }`: redacted, serializable; account_label is non-secret identifier only.
- `list(state) -> Vec<AuthStatus>`: deterministic provider-id order; bounded length.
- `inspect(state, provider) -> Result<AuthStatus, AuthCommandError>`: unknown provider yields UnknownProvider.
- Limits: provider id 1..=128 chars; list capped at 32 entries with truncated flag beyond.
- Deterministic: same command sequence yields byte-identical statuses; no I/O, no network, no clock reads (caller supplies now_ms).
- Suggested module boundary: crates/providers/src/auth_commands.rs owning AuthCommand, AuthEffect, AuthStatus, AuthCommandError, dispatch, list, inspect; shared lib.rs wiring left to integrator.

## Failure states

- Unknown provider on inspect/refresh/logout: `Err(AuthCommandError::UnknownProvider)`; state unchanged.
- Import or login without consent: effect with needs_consent true and unchanged state; never proceeds silently.
- Empty provider id: `Err(AuthCommandError::EmptyProvider)`; state unchanged.
- List beyond cap: returns first 32 plus truncated true; never exceeds cap.
- Secret safety: every status, effect, error, and log contains zero credential bytes. Tests use fake grants and disposable in-memory state only.

## Resource bounds

- Pure planner: no Command, no thread, no I/O, no network; caller owns state lifetime.
- Bounded commands and statuses; list cap 32; strings capped (provider 128, account label 256).
- No unbounded queue, no unbounded retained output; owner and cancel path defined per crates/providers/src/integration.rs:11-18 bounded-constants precedent.

## Acceptance criteria

- PROV-020-T01: connect plus login flow: dispatch Connect then Login yields needs_consent true with unchanged state; grant-backed completion yields status with provider, mode, and expiry set.
- PROV-020-T02: list and inspect: two providers yield list of 2 in provider-id order; inspect of each matches list entry; inspect unknown yields UnknownProvider.
- PROV-020-T03: refresh and logout: refresh on ready updates expiry; logout yields logged-out status with no expiry; refresh while logged out yields NotLoggedIn.
- PROV-020-T04: redaction: Debug plus serde_json of every command, effect, and status contain no fixture secrets and no "sk-" substrings.
- PROV-020-T05: bounds and isolation: 40 providers list-truncates at 32 with flag; same sequence byte-identical; no files outside disposable test dir; no DB writes.

## Test-first execution

- RED: author tests PROV-020-T01..T05 against crates/providers/src/auth_commands.rs; establish compiling RED (fail: no auth_commands module).
- GREEN: implement minimum native Rust command planner and redacted status; GREEN, refactor, rerun; negative tests (unknown, no-consent, empty, truncation, leak scan).
- Evidence: `cargo test -p opencode-rk-providers auth_commands`; `cargo check --workspace`; frozen test hash plus command manifest; verifier decides acceptance.
