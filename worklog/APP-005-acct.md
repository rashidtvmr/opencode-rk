# APP-005-acct worklog (account_setup.rs lane)

## Claim
Own only `crates/providers/src/account_setup.rs`. Defines account-setup types:
`AuthorizedAccount` (provider/model/effort + `SecureStoreMarker`), `CredentialImportScope`
(explicit `ImportConsent` holder), `PendingSetup` cancel path, `RemovalReceipt`
(secret-cleared), `OfflineFallback` (cached, no hosted login), typed `SetupError`
with retry/change-provider actions. `#![forbid(unsafe_code)]`, std only
(`std::error::Error`, `std::fmt`), no secret logging.

## Source evidence
- HEAD `5af7884` (verified via `git rev-parse --short HEAD`).
- `crates/providers/src/auth.rs:8-78`: `AuthMethod`, `ProviderAuth::new`,
  `AuthHandler{add_auth,get,validate,refresh}`. This lane's store is a separate
  setup boundary; it does not reuse those types (auth.rs holds secret-bearing
  `AuthMethod::ApiKey(String)` values; here no credential bytes exist).
- `crates/providers/src/lib.rs:1-57`: no `account_setup` module wired (lane owns
  only the file; integration left to controller).
- `crates/providers/src/local_credential_import.rs:45-81`: `UserConsent::Granted`
  / `ImportRequest` explicit-consent precedent; this lane mirrors it with
  `ImportConsent` and checks consent at both `begin` and `authorize`.
- `crates/providers/src/account_status.rs:1-90`: bounding precedent
  (`MAX_STATUS_ACCOUNTS=128`); this lane bounds with `MAX_SETUP_ACCOUNTS=16`
  and `MAX_ID_CHARS=64`.
- APP-005 card `tasks/completion/local.json:8`: tests mapped.
- `docs/TDD.md`, `docs/SECURITY.md` read; tests assert side-effect absence
  (cancel/deny/failure leave `is_empty`, `secret_count == 0`).

## Observed scenario
- RED run: `rustc --edition 2021 --test crates/providers/src/account_setup.rs
  -o /tmp/opencode/as2 && /tmp/opencode/as2`: compile OK, 8 FAILED / 2 passed
  (`authorize` returned `InvalidCredential`; `remove` returned `None`).
- GREEN run after implementing both methods: 10 passed, 0 failed, exit 0.

## Target boundary
- Pure caller-owned state; no fs/env/clock/net/threads; identifiers ASCII
  `[A-Za-z0-9._-]` so no shell/log-injection syntax; errors secret-free with
  `Display` + `Error`; accounts replace per provider (no half-authorized dup);
  `secret_count == len` invariant (`secret_held` only true for stored entries,
  `remove` drops entry + asserts absence, returns `secret_cleared: true`).

## Tests (frozen in owned file, `#[cfg(test)] mod tests`)
1. `cancel_leaves_none` — cancel stores nothing, no secret.
2. `invalid_shows_retry` — `InvalidCredential` + `Retry`, store empty.
3. `removal_leaves_no_secret` — receipt `secret_cleared`, count 0, empty.
4. `valid_authorize_marks_secure_store` — fields + `is_secure_stored` + marker.
5. `pending_complete_stores_secure_account` — begin/complete stores one secret.
6. `offline_cached_fallback_needs_no_login` — `from_cache`, provider/model.
7. `consent_required_for_import` — `begin` + `authorize` deny, store empty.
8. `endpoint_failure_shows_retry` — `EndpointFailure` + `Retry`, store empty.
9. `model_unavailable_suggests_change_provider` — `ModelUnavailable` +
   `ChangeProvider`, store empty.
10. `debug_carries_no_secret` — Debug/Display carry ids + `secure-store`,
    no `sk-`/`token=`/`bearer`/token-field markers.

## Decisions
- `scope.provider_id != provider_id` treated as `ConsentRequired` (grant bound
  to one provider; no cross-provider reuse).
- `remove` returns `Option<RemovalReceipt>`; unknown provider yields `None`.
- `ponytail`: single account per provider; upgrade path multi-account ids.
- Skipped: secure-storage backend, OAuth/PKCE transport, endpoint probing —
  all caller-owned; add when onboarding slice wires this module via `lib.rs`.

## Remaining unknowns
- `crates/cli/src/onboarding.rs` (other APP-005 path) shape unknown to this
  lane; controller wires module integration.
