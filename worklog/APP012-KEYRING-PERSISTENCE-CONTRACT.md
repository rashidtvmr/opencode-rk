# APP-012 Keyring Persistence Contract

## Claim
- Task/session: `APP012-KEYRING-PERSISTENCE-CONTRACT` / `ses_f2cd8fde2ffedwvgJYma5yueaJ`.
- Owned file: `worklog/APP012-KEYRING-PERSISTENCE-CONTRACT.md` (this file).
- Other permitted file: own claim row in `tasks/completion/claims.json` only.
- Branch: `plan/APP012-KEYRING` (worktree `plan-app012-keyring`).
- No product source, test, Cargo.toml, Cargo.lock, or frozen-test edits in this lane.

## Source evidence (authoritative)
- `PLAN.md:72-77` (controller decision): authorize pinned `keyring` 3.6.3;
  workspace MSRV is 1.85 so `keyring` 4.2.0 (requires 1.88) is forbidden; use the
  crate's mock credential builder in tests; reserve real backend proof for
  disposable platform acceptance.
- `crates/cli/src/daemon_client.rs:775-791`: `creds_configured` currently
  env-var-only (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `GOOGLE_API_KEY`,
  `GEMINI_API_KEY`); any non-empty value counts as configured. No keyring read.
- `crates/cli/src/app_start.rs:287-321`: `plan_default_launch` routes TTY +
  `!creds_configured` to `StartupView::Setup` (in-app onboarding).
- `crates/cli/src/onboarding.rs`: `SecretString` (redacted Debug/Display emits
  `[REDACTED]`, raw via `with_exposed`), `AccountStore` trait,
  `MemoryAccountStore` (test/in-memory), `OnboardingSession` state machine
  (`Welcome -> ProviderSelect -> CredentialEntry -> ModelSelect -> Done`),
  `redact_secrets_in` for log/transcript redaction.
- `crates/providers/src/auth_store.rs` (PROV-022 seam): `StoreBackend::{Keyring,
  File0600}`, `StoreError::KeyringUnavailable` (code `"keyring_unavailable"`),
  `StoreRequest { provider_id, backend, dest: AllowlistedPath }`,
  `StorePlan { provider_id, backend, dest_label, mode }`,
  `RefreshPlan { provider_id, atomic }`, `StoreStatus { provider_id, backend,
  exists, error }`, `validate_dest`, `validate_secret`,
  `status_with_keyring(provider_id, backend, keyring_available)`.
  Planning-only: no keyring open, no file I/O, no env reads, no secret bytes
  retained. `SealedSecret<'a>` borrows caller-owned buffer.
- `crates/providers/src/auth_profile.rs`: `AuthProvenance::{ApiKey,
  OfficialOAuth, ImportedLocal, Keyring}` (`as_str` -> `"keyring"`).
- `crates/providers/src/config.rs`: `ProviderConfig.api_key_env` (e.g.
  `"OPENAI_API_KEY"`); `get_api_key()` reads `env::var(&self.api_key_env)`.
- `crates/providers/src/responses.rs:519-522`: `OpenAiResponsesClient::from_env`
  calls `config.get_api_key()` for live provider requests.
- `crates/providers/src/auth.rs:6-18`: `AuthMethod::{ApiKey(String),
  BearerToken(String), OAuth2{...}}` (holds secret bytes).
- `crates/security/src/credentials.rs`: in-memory `CredentialStore` (no
  persistence), `Credential { id, provider, token, expires_at }`.
- `crates/server/tests/app012_tool_journey_red.rs`: frozen APP-012 RED, SHA-256
  `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`
  (verified unchanged on disk).

## Call graph: setup -> store -> retrieve -> restart

```
OnboardingSession::select_model (commit_account)
    |
    | [RED lane: APP-012 keyring persistence]
    v
KeyringSecretStore::save(provider_id, secret)
    |
    | 1. plan_store(StoreRequest{provider_id, Keyring, dest})  [PROV-022]
    | 2. validate_secret(secret)                              [PROV-022]
    | 3. keyring::Entry::new(SERVICE, account_key).set_password(secret)
    |    - failure -> status_with_keyring(.., false) -> KeyringUnavailable
    |    - fail closed: no File0600 plaintext fallback in this path
    v
creds_configured (daemon_client.rs:776) -> reads OS keyring
    |
    v
StartupView::Setup (app_start.rs:273)  -- when no keyring entry / unavailable
    |
    v
OnboardingSession (onboarding.rs:424)
    |
    v
KeyringSecretStore::load(provider_id) -> Option<SecretString>
    |
    v
ProviderConfig::get_api_key  -- env-first, keyring fallback
    |
    v
responses.rs:519  OpenAiResponsesClient::from_env (provider request)
```

Restart retrieval: `creds_configured` reads keyring on daemon start; if an entry
exists and backend is available, `Some(true)` -> `StartupView::Main`. If no entry
or keyring unavailable, `Some(false)`/`None` -> `StartupView::Setup`.

Overwrite: `save` is idempotent; `keyring::set_password` replaces the prior entry
under the same (SERVICE, account_key). The prior `SecretString` in memory is
zeroed when dropped (Rust `String` semantics; no explicit zeroize yet per
`credentials.rs:93` ponytail note).

## Service/account identity

- **Service name**: `"opencode-rk"` (workspace-anchored, deterministic).
  Not user-scoped: the singleton is per-OS-user + data dir (PLAN ADR-002).
- **Account key**: `format!("{provider_id}")` (e.g. `"openai"`). Single account
  per provider in the baseline contract; one `AuthorizedAccount` per
  `AccountSetupStore` entry (account_setup.rs:386 replacement semantics).
- **Dependency boundary**: `keyring` 3.6.3 added to
  `crates/providers/Cargo.toml` `[dependencies]` and `Cargo.lock` by the
  implementer lane. This research lane does NOT touch Cargo.

## State matrix

| State | Trigger | Keyring op | Result | Next state |
|-------|---------|------------|--------|------------|
| Missing | restart / `creds_configured` | `get_password(SERVICE, "openai")` | `None` | Setup |
| Present | restart / `creds_configured` | `get_password(SERVICE, "openai")` | `Some(secret)` | Main |
| First save | onboarding commit | `set_password(SERVICE, "openai", secret)` | Ok | Present |
| Overwrite | re-onboard same provider | `set_password(...)` replaces | Ok | Present (rotated) |
| Delete | user-initiated remove | `delete_password(SERVICE, "openai")` | Ok | Missing |
| Locked DB | any read/write | `get/set_password` error | Err | Setup with fallback status |
| Unsupported | headless/CI | backend init fails | Err(KeyringUnavailable) | Setup or env-only |

## Error/failure semantics

### Missing keyring entry
- `get_password` returns `None` (not an error).
- `creds_configured` returns `Some(false)`.
- App routes to `StartupView::Setup`.

### Locked keyring (OS-level lock, e.g. user logged out)
- `keyring` crate returns `Err(GetPasswordError)` or similar.
- Caller maps to `StoreError::KeyringUnavailable` (code: `"keyring_unavailable"`).
- No plaintext fallback in the APP-012 keyring path (PLAN controller decision
  reserves 0600-file fallback to PROV-022; APP-012 keyring persistence is
  keyring-or-deny).
- `creds_configured` returns `None` (unknown) -> Setup.

### Unsupported platform / headless CI
- macOS: Keychain (native).
- Linux: Secret Service / freedesktop (daemon must be running).
- Windows: Windows Credential Manager (GNU ABI only per PLAN:72-77).
- Headless CI / no GUI session: keyring backend init returns error.
  No real keyring write during tests; use keyring's mock credential builder.
- Test environment: `keyring` 3.6.3 provides a mock credential builder for
  `set_password`/`get_password`/`delete_password` without OS backend.

### Oversize secret
- `validate_secret` enforces `MAX_SECRET_BYTES = 64 * 1024` (auth_store.rs:24).
- Returns `Err(StoreError::TooLarge)` (`"secret_too_large"`).
- Nothing written.

### Invalid provider ID
- `validate_provider` (auth_store.rs:268-276): empty or `> 128 bytes` or
  contains NUL.
- Returns `Err(StoreError::EmptyProvider)` / `StoreError::ProviderTooLong`.
- Nothing written.

## Platform boundaries

| Platform | Backend | Availability | Note |
|----------|---------|-------------|------|
| macOS | Keychain | Native, MSRV-safe | PIN prompt on first access |
| Linux | Secret Service / freedesktop | Requires running daemon | Headless CI: unavailable -> Setup |
| Windows | Credential Manager | x86_64-pc-windows-gnu only | No MSVC (PLAN:72-77) |
| Test | `keyring` mock builder | Deterministic | No OS write; disposable |
| Frozen RED test | env vars | No keyring | `app012_tool_journey_red.rs:324-327` sets `OPENAI_API_KEY` |

## Deletion semantics

- `delete_password(SERVICE, account_key)` removes the OS entry.
- If entry absent: keyring returns `Err` (`PasswordItemNotFound`-style); caller
  treats as success (idempotent deletion), matching
  `AccountSetupStore::remove` semantics (account_setup.rs:433-446 returns
  `Option<RemovalReceipt>`).
- `creds_configured` after deletion returns `Some(false)` -> Setup.
- No partial state: `delete_password` is atomic at the OS layer.

## Redaction rules

1. `SecretString` Debug/Display always emit `[REDACTED]` (onboarding.rs:159,192).
2. `AccountSetupStore` debug assertions verify no secret markers leak
   (account_setup.rs:600-616): `sk-`, `token=`, `bearer`, `access_token`,
   `refresh_token` must not appear in rendered output.
3. `redact_secrets_in` applied to all log lines, transcript rows, QR payloads,
   exported config (onboarding.rs:206-215).
4. `StoreError` variants carry fixed codes only, never OS error text or credential
   bytes (auth_store.rs:97-110).
5. `StorePlan`/`StoreStatus` carry `dest_label` (relative path only), never the
   storage root or secret bytes (auth_store.rs:244-266, 220-239).
6. `AuthProvenance::Keyring` serializes as `"keyring"` with no token material
   (auth_profile.rs:27-29).
7. `native_status.rs:402-406`: keyring credential context entry is
   `Visibility::Secret` -- never rendered.

## Injectable disposable backend (mock)

- `keyring` 3.6.3 provides a mock credential builder:
  `keyring::set_default_mock!()` / `MockEntryBuilder` for tests.
- This lane does NOT write tests; the implementer lane (PROV-022 + APP-012
  keyring persistence) will use the mock builder to assert save/retrieve/delete
  without touching the OS keyring.
- Frozen RED test `app012_tool_journey_red.rs` currently uses env vars
  (`OPENAI_API_KEY`, line 324) and does NOT exercise keyring persistence. The
  restart/retrieval path is the documented remaining gap (APP-012.md:98-100).

## RED/source ownership boundaries

### Existing frozen RED (must remain byte-identical)
- `crates/server/tests/app012_tool_journey_red.rs`: SHA-256
  `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`
  (verified on disk). This test exercises provider tool dispatch but NOT keyring
  persistence.

### Future RED (specified here, authored by implementer lane)
- **File**: `crates/providers/tests/keyring_persistence_red.rs` (new).
- **Scope**: asserts the missing keyring persistence behavior against the
  real `keyring` 3.6.3 mock builder.
- **Test cases**:
  - T01: first save then retrieve returns the same secret (mock backend).
  - T02: overwrite replaces prior secret (mock backend).
  - T03: missing entry returns `None` / `Some(false)` (creds_configured).
  - T04: locked keyring maps to `StoreError::KeyringUnavailable`.
  - T05: delete removes entry; retrieval returns `None` (idempotent).
  - T06: `AuthProvenance::Keyring` round-trips through `describe` with no
    secret leak in Debug/serde.
- **RED expectation**: fails to compile (module `keyring_persistence` not yet
  wired) or fails at runtime (no keyring save/load implementation).
- **Frozen before GREEN**: implementer lane freezes SHA-256 of this RED before
  implementing.

### Source ownership
- **Research lane (this)**: `worklog/APP012-KEYRING-PERSISTENCE-CONTRACT.md`
  only. No source/test/Cargo edits.
- **Implementer lane**:
  - Add `keyring = "=3.6.3"` to `crates/providers/Cargo.toml`.
  - New module `crates/providers/src/keyring_persistence.rs` exposing
    `KeyringSecretStore` with `save`, `load`, `delete`, `exists` methods.
  - Wire into `crates/cli/src/daemon_client.rs` `creds_configured` (replace
    env-only check with env-then-keyring, or add `creds_configured_keyring`).
  - Wire into `crates/providers/src/responses.rs` `OpenAiResponsesClient`
    fallback: env -> keyring (or document env-first with keyring as secondary).
  - `lib.rs` `pub mod keyring_persistence;` (integrator-owned prewire).
  - Frozen tests: `crates/providers/tests/keyring_persistence_red.rs`.

### Shared-file changes (require integration proposal)
- `crates/cli/src/daemon_client.rs` (creds_configured) - caller wiring.
- `crates/providers/src/lib.rs` (mod declaration).
- `crates/providers/Cargo.toml` (dependency).
- These are NOT owned by this research lane; an integration proposal must
  precede any edit.

## Dependency acceptance boundary

- `keyring` 3.6.3: MSRV 1.75, compatible with workspace `rust-version = "1.85"`
  (Cargo.toml:23).
- `keyring` 4.2.0: requires Rust 1.88, FORBIDDEN by workspace MSRV; never added.
- `Cargo.lock` will be regenerated by the implementer lane after adding the
  dependency; this lane does not touch the lockfile.
- No secret-bearing fields in `ProviderConfig`, `AuthProfile`, `AuthMethod`, or
  `CredentialStore` ever accept a `keyring` handle object (secrets are
  `SecretString`/`SealedSecret<'a>` borrow types only).