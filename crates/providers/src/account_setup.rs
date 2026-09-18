//! In-app provider account setup (APP-005).
//!
//! Pure, bounded setup boundary: authorize accounts whose secrets live in
//! secure storage (this module retains only a [`SecureStoreMarker`], never
//! credential bytes), gate credential import on explicit consent, prove
//! removal clears the secret, and mark offline-cached fallback that needs no
//! hosted login. No I/O, no clock, no env, no threads; the caller owns
//! persistence and transport.
//!
//! Evidence: leased HEAD 5af7884, `crates/providers/src/auth.rs:8-78`
//! (`AuthMethod` / `ProviderAuth` / `AuthHandler`), `docs/TDD.md`,
//! `docs/SECURITY.md`, APP-005 card (`tasks/completion/local.json`).

#![forbid(unsafe_code)]

use std::error::Error;
use std::fmt;

/// Upper bound on authorized accounts held by one store.
pub const MAX_SETUP_ACCOUNTS: usize = 16;
/// Upper bound on provider/model/effort identifier length, in chars.
pub const MAX_ID_CHARS: usize = 64;

/// Marker proving the account secret lives in secure storage.
///
/// Carries no credential bytes; safe to log, clone, and retain.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct SecureStoreMarker;

impl SecureStoreMarker {
    /// Fresh secure-store marker.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

/// Authorized provider account: provider/model/effort plus the secure-store
/// marker. Holds no credential bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizedAccount {
    provider_id: String,
    model_id: String,
    effort: String,
    secure: SecureStoreMarker,
}

impl AuthorizedAccount {
    /// Stored provider identifier.
    #[must_use]
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// Stored model identifier.
    #[must_use]
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    /// Stored effort level.
    #[must_use]
    pub fn effort(&self) -> &str {
        &self.effort
    }

    /// Secure-store marker handle.
    #[must_use]
    pub fn secure_marker(&self) -> SecureStoreMarker {
        self.secure
    }

    /// Always true: the secret lives in secure storage, never here.
    #[must_use]
    pub fn is_secure_stored(&self) -> bool {
        true
    }
}

impl fmt::Display for AuthorizedAccount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}/{} [{}] [secure-store]",
            self.provider_id, self.model_id, self.effort
        )
    }
}

/// Explicit human consent for a credential import. Only [`ImportConsent::Granted`]
/// permits planning or storing; selection of a source is never implicit approval.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum ImportConsent {
    /// No explicit human grant exists.
    #[default]
    NotGranted,
    /// An explicit human grant exists for this import.
    Granted,
}

/// Credential-import scope holder: binds one provider to its explicit consent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialImportScope {
    provider_id: String,
    consent: ImportConsent,
}

impl CredentialImportScope {
    /// Bind a provider to its consent state. Retains no credential bytes.
    #[must_use]
    pub fn new(provider_id: &str, consent: ImportConsent) -> Self {
        Self {
            provider_id: provider_id.to_owned(),
            consent,
        }
    }

    /// Scoped provider identifier.
    #[must_use]
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// Consent state.
    #[must_use]
    pub fn consent(&self) -> ImportConsent {
        self.consent
    }

    /// True only for an explicit grant.
    #[must_use]
    pub fn is_granted(&self) -> bool {
        self.consent == ImportConsent::Granted
    }
}

/// Pending setup: consent checked at [`PendingSetup::begin`], nothing stored
/// until [`PendingSetup::complete`]. Dropping or cancelling stores nothing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingSetup {
    provider_id: String,
    consent: ImportConsent,
}

impl PendingSetup {
    /// Open a pending setup after the explicit-consent gate.
    pub fn begin(scope: &CredentialImportScope) -> Result<Self, SetupError> {
        if !scope.is_granted() {
            return Err(SetupError::ConsentRequired);
        }
        if !is_valid_id(scope.provider_id()) {
            return Err(SetupError::BadId);
        }
        Ok(Self {
            provider_id: scope.provider_id().to_owned(),
            consent: scope.consent(),
        })
    }

    /// Complete the pending setup into the store. `credential_valid`,
    /// `endpoint_ok`, and `model_available` are caller-supplied booleans;
    /// no credential bytes cross this boundary.
    pub fn complete(
        self,
        store: &mut AccountSetupStore,
        model_id: &str,
        effort: &str,
        credential_valid: bool,
        endpoint_ok: bool,
        model_available: bool,
    ) -> Result<(), SetupError> {
        let scope = CredentialImportScope::new(&self.provider_id, self.consent);
        store.authorize(
            &scope,
            &self.provider_id,
            model_id,
            effort,
            credential_valid,
            endpoint_ok,
            model_available,
        )
    }

    /// Cancel before completion. Stores nothing by construction.
    #[must_use]
    pub fn cancel(self) -> CancelledSetup {
        CancelledSetup {
            provider_id: self.provider_id,
        }
    }
}

/// Cancel receipt: proves the flow ended without storing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CancelledSetup {
    provider_id: String,
}

impl CancelledSetup {
    /// Provider the cancelled flow was bound to.
    #[must_use]
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }
}

/// Removal receipt: proves the account and its secure secret handle are gone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemovalReceipt {
    provider_id: String,
    secret_cleared: bool,
}

impl RemovalReceipt {
    /// Removed provider identifier.
    #[must_use]
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// True when an account existed and its secret handle was dropped.
    #[must_use]
    pub fn secret_cleared(&self) -> bool {
        self.secret_cleared
    }
}

/// Offline-cached fallback marker: serves a cached account with no hosted login.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfflineFallback {
    provider_id: String,
    model_id: String,
    from_cache: bool,
}

impl OfflineFallback {
    /// Mark a stored account as the offline-cached fallback.
    #[must_use]
    pub fn from_cached(account: &AuthorizedAccount) -> Self {
        Self {
            provider_id: account.provider_id().to_owned(),
            model_id: account.model_id().to_owned(),
            from_cache: true,
        }
    }

    /// Cached provider identifier.
    #[must_use]
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// Cached model identifier.
    #[must_use]
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    /// Always true when built via [`OfflineFallback::from_cached`].
    #[must_use]
    pub fn is_from_cache(&self) -> bool {
        self.from_cache
    }
}

/// Next action shown for a setup failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetupAction {
    /// Stay on this provider and retry entry.
    Retry,
    /// Offer switching to another provider.
    ChangeProvider,
}

/// Secret-free setup failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetupError {
    /// No explicit human grant for this provider.
    ConsentRequired,
    /// Credentials rejected; show retry.
    InvalidCredential,
    /// Endpoint unreachable; show retry.
    EndpointFailure,
    /// Model unavailable; offer change-provider.
    ModelUnavailable,
    /// Malformed provider, model, or effort identifier.
    BadId,
    /// Store already holds [`MAX_SETUP_ACCOUNTS`] accounts.
    TooManyAccounts,
}

impl SetupError {
    /// UI action for credential/endpoint/model failures; `None` for
    /// consent, identifier, and capacity denials.
    #[must_use]
    pub fn action(&self) -> Option<SetupAction> {
        match self {
            SetupError::InvalidCredential | SetupError::EndpointFailure => {
                Some(SetupAction::Retry)
            }
            SetupError::ModelUnavailable => Some(SetupAction::ChangeProvider),
            SetupError::ConsentRequired
            | SetupError::BadId
            | SetupError::TooManyAccounts => None,
        }
    }
}

impl fmt::Display for SetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            SetupError::ConsentRequired => "explicit credential-import consent is required",
            SetupError::InvalidCredential => "invalid credentials; retry",
            SetupError::EndpointFailure => "endpoint failure; retry",
            SetupError::ModelUnavailable => "model unavailable; change provider",
            SetupError::BadId => "invalid provider, model, or effort identifier",
            SetupError::TooManyAccounts => "too many authorized accounts",
        };
        f.write_str(text)
    }
}

impl Error for SetupError {}

/// Bounded in-memory setup store. Secrets live in secure storage; each entry
/// holds exactly one [`SecureStoreMarker`] handle, so `secret_count == len`.
#[derive(Clone, Debug, Default)]
pub struct AccountSetupStore {
    accounts: Vec<AuthorizedAccount>,
}

impl AccountSetupStore {
    /// Empty store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            accounts: Vec::new(),
        }
    }

    /// Number of authorized accounts (equals secrets held).
    #[must_use]
    pub fn len(&self) -> usize {
        self.accounts.len()
    }

    /// True when no account is stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.accounts.is_empty()
    }

    /// True when the provider has an authorized account.
    #[must_use]
    pub fn contains(&self, provider_id: &str) -> bool {
        self.accounts
            .iter()
            .any(|account| account.provider_id() == provider_id)
    }

    /// Stored account for the provider, if any.
    #[must_use]
    pub fn get(&self, provider_id: &str) -> Option<&AuthorizedAccount> {
        self.accounts
            .iter()
            .find(|account| account.provider_id() == provider_id)
    }

    /// True when a secure secret handle is held for the provider.
    #[must_use]
    pub fn secret_held(&self, provider_id: &str) -> bool {
        self.contains(provider_id)
    }

    /// Number of secure secret handles held.
    #[must_use]
    pub fn secret_count(&self) -> usize {
        self.accounts.len()
    }

    /// Authorize (or re-authorize) an account after consent, identifier,
    /// capacity, endpoint, model, and credential gates pass. Re-authorizing
    /// replaces the entry, so no half-authorized duplicate remains.
    /// ponytail: single-provider replacement keeps one secret per provider;
    /// upgrade path is multi-account ids when onboarding needs them.
    pub fn authorize(
        &mut self,
        scope: &CredentialImportScope,
        provider_id: &str,
        model_id: &str,
        effort: &str,
        credential_valid: bool,
        endpoint_ok: bool,
        model_available: bool,
    ) -> Result<(), SetupError> {
        if !scope.is_granted() || scope.provider_id() != provider_id {
            return Err(SetupError::ConsentRequired);
        }
        if !is_valid_id(provider_id) || !is_valid_id(model_id) || !is_valid_id(effort) {
            return Err(SetupError::BadId);
        }
        if !credential_valid {
            return Err(SetupError::InvalidCredential);
        }
        if !endpoint_ok {
            return Err(SetupError::EndpointFailure);
        }
        if !model_available {
            return Err(SetupError::ModelUnavailable);
        }
        if self.get(provider_id).is_none() && self.accounts.len() >= MAX_SETUP_ACCOUNTS {
            return Err(SetupError::TooManyAccounts);
        }
        let account = AuthorizedAccount {
            provider_id: provider_id.to_owned(),
            model_id: model_id.to_owned(),
            effort: effort.to_owned(),
            secure: SecureStoreMarker::new(),
        };
        match self
            .accounts
            .iter()
            .position(|existing| existing.provider_id() == provider_id)
        {
            Some(index) => self.accounts[index] = account,
            None => self.accounts.push(account),
        }
        Ok(())
    }

    /// Remove the account and drop its secure secret handle. Returns `None`
    /// when nothing was stored.
    pub fn remove(&mut self, provider_id: &str) -> Option<RemovalReceipt> {
        let index = self
            .accounts
            .iter()
            .position(|account| account.provider_id() == provider_id)?;
        let dropped = self.accounts.remove(index);
        debug_assert!(dropped.is_secure_stored());
        drop(dropped);
        debug_assert!(!self.secret_held(provider_id));
        Some(RemovalReceipt {
            provider_id: provider_id.to_owned(),
            secret_cleared: true,
        })
    }

    /// Offline fallback for a cached account. Needs no hosted login; it only
    /// re-marks what is already stored.
    #[must_use]
    pub fn offline_cached(&self, provider_id: &str) -> Option<OfflineFallback> {
        self.get(provider_id).map(OfflineFallback::from_cached)
    }
}

/// Identifiers are short ASCII tokens (`[A-Za-z0-9._-]`); anything else is a
/// `BadId` so values can never smuggle shell or log-injection syntax.
fn is_valid_id(value: &str) -> bool {
    if value.is_empty() || value.chars().count() > MAX_ID_CHARS {
        return false;
    }
    value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn granted(provider: &str) -> CredentialImportScope {
        CredentialImportScope::new(provider, ImportConsent::Granted)
    }

    #[test]
    fn cancel_leaves_none() {
        let store = AccountSetupStore::new();
        let scope = granted("acme");
        let pending = PendingSetup::begin(&scope).expect("begin");
        let cancelled = pending.cancel();
        assert_eq!(cancelled.provider_id(), "acme");
        assert!(store.is_empty());
        assert_eq!(store.secret_count(), 0);
        assert!(!store.contains("acme"));
        assert!(!store.secret_held("acme"));
    }

    #[test]
    fn invalid_shows_retry() {
        let mut store = AccountSetupStore::new();
        let scope = granted("acme");
        let err = store
            .authorize(&scope, "acme", "m1", "high", false, true, true)
            .expect_err("invalid credential must fail");
        assert_eq!(err, SetupError::InvalidCredential);
        assert_eq!(err.action(), Some(SetupAction::Retry));
        assert!(store.is_empty());
        assert_eq!(store.secret_count(), 0);
    }

    #[test]
    fn removal_leaves_no_secret() {
        let mut store = AccountSetupStore::new();
        let scope = granted("acme");
        store
            .authorize(&scope, "acme", "m1", "high", true, true, true)
            .expect("authorize");
        assert!(store.secret_held("acme"));
        let receipt = store.remove("acme").expect("receipt");
        assert_eq!(receipt.provider_id(), "acme");
        assert!(receipt.secret_cleared());
        assert!(!store.secret_held("acme"));
        assert_eq!(store.secret_count(), 0);
        assert!(store.is_empty());
    }

    #[test]
    fn valid_authorize_marks_secure_store() {
        let mut store = AccountSetupStore::new();
        let scope = granted("acme");
        store
            .authorize(&scope, "acme", "m1", "high", true, true, true)
            .expect("authorize");
        let account = store.get("acme").expect("stored");
        assert_eq!(account.provider_id(), "acme");
        assert_eq!(account.model_id(), "m1");
        assert_eq!(account.effort(), "high");
        assert!(account.is_secure_stored());
        assert_eq!(account.secure_marker(), SecureStoreMarker::new());
    }

    #[test]
    fn pending_complete_stores_secure_account() {
        let mut store = AccountSetupStore::new();
        let scope = granted("acme");
        let pending = PendingSetup::begin(&scope).expect("begin");
        pending
            .complete(&mut store, "m1", "high", true, true, true)
            .expect("complete");
        assert!(store.secret_held("acme"));
        assert_eq!(store.secret_count(), 1);
    }

    #[test]
    fn offline_cached_fallback_needs_no_login() {
        let mut store = AccountSetupStore::new();
        let scope = granted("acme");
        store
            .authorize(&scope, "acme", "m1", "low", true, true, true)
            .expect("authorize");
        let fallback = store.offline_cached("acme").expect("cached");
        assert!(fallback.is_from_cache());
        assert_eq!(fallback.provider_id(), "acme");
        assert_eq!(fallback.model_id(), "m1");
    }

    #[test]
    fn consent_required_for_import() {
        let mut store = AccountSetupStore::new();
        let scope = CredentialImportScope::new("acme", ImportConsent::NotGranted);
        assert!(!scope.is_granted());
        assert_eq!(
            PendingSetup::begin(&scope).expect_err("consent"),
            SetupError::ConsentRequired
        );
        assert_eq!(
            store
                .authorize(&scope, "acme", "m1", "high", true, true, true)
                .expect_err("consent"),
            SetupError::ConsentRequired
        );
        assert!(store.is_empty());
    }

    #[test]
    fn endpoint_failure_shows_retry() {
        let mut store = AccountSetupStore::new();
        let scope = granted("acme");
        let err = store
            .authorize(&scope, "acme", "m1", "high", true, false, true)
            .expect_err("endpoint");
        assert_eq!(err, SetupError::EndpointFailure);
        assert_eq!(err.action(), Some(SetupAction::Retry));
        assert!(store.is_empty());
    }

    #[test]
    fn model_unavailable_suggests_change_provider() {
        let mut store = AccountSetupStore::new();
        let scope = granted("acme");
        let err = store
            .authorize(&scope, "acme", "nope", "high", true, true, false)
            .expect_err("model");
        assert_eq!(err, SetupError::ModelUnavailable);
        assert_eq!(err.action(), Some(SetupAction::ChangeProvider));
        assert!(store.is_empty());
    }

    #[test]
    fn debug_carries_no_secret() {
        let mut store = AccountSetupStore::new();
        let scope = granted("acme");
        store
            .authorize(&scope, "acme", "m1", "high", true, true, true)
            .expect("authorize");
        let account = store.get("acme").expect("stored");
        let rendered = format!("{account:?} {account}");
        assert!(rendered.contains("acme"));
        assert!(rendered.contains("secure-store"));
        for leak in ["sk-", "token=", "bearer", "access_token", "refresh_token"] {
            assert!(
                !rendered.to_lowercase().contains(leak),
                "leak marker present: {leak}"
            );
        }
    }
}
