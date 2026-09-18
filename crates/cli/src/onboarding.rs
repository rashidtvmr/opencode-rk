#![forbid(unsafe_code)]

//! First-run provider setup for the CLI (`APP-005`).
//!
//! Owns the onboarding state machine: [`SetupStep`] ordering, provider
//! selection, credential entry, model selection, OAuth PKCE challenge holding,
//! cancel-without-residue receipts, offline cached-settings start, and secret
//! redaction for logs/transcripts/exports.
//!
//! Security contract (see `docs/SECURITY.md`):
//! - Secrets live only in [`SecretString`] (redacted `Debug`/`Display`) and in
//!   the caller's closure via [`SecretString::with_exposed`]. They are never
//!   embedded in [`SetupError`], log lines, or `Debug` output.
//! - The PKCE holder stores the challenge as an opaque string produced by the
//!   external OAuth library. This module invents no crypto and performs no
//!   network or filesystem I/O.
//! - Cancel consumes the session and removes any staged-but-uncommitted
//!   account, so no half-authorized account survives (see [`CancelReceipt`]).
//! - All inputs are length-bounded (`MAX_*` constants); no unbounded retained
//!   output.
//!
//! Commit `5af7884`; new requirement (no prior `onboarding.rs` in
//! `crates/cli/src/`).

use std::collections::HashMap;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

/// Longest accepted provider id, in bytes.
pub const MAX_PROVIDER_ID_LEN: usize = 64;
/// Longest accepted model id, in bytes.
pub const MAX_MODEL_ID_LEN: usize = 128;
/// Longest accepted secret, in bytes (8 KiB bound on retained secret bytes).
pub const MAX_SECRET_LEN: usize = 8192;
/// Shortest credential the provider fixture accepts.
pub const MIN_CREDENTIAL_LEN: usize = 8;
/// Longest accepted opaque PKCE challenge string, in bytes.
pub const MAX_PKCE_OPAQUE_LEN: usize = 512;
/// Longest accepted PKCE `state` value, in bytes.
pub const MAX_PKCE_STATE_LEN: usize = 256;
/// Replacement text emitted wherever a secret would appear.
pub const REDACTED: &str = "[REDACTED]";

// ---------------------------------------------------------------------------
// Setup steps
// ---------------------------------------------------------------------------

/// Ordered first-run steps. Advance only via [`SetupStep::next`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupStep {
    Welcome,
    ProviderSelect,
    CredentialEntry,
    ModelSelect,
    Done,
}

impl SetupStep {
    /// Next step in the flow, or `None` once setup is [`SetupStep::Done`].
    pub fn next(self) -> Option<SetupStep> {
        match self {
            SetupStep::Welcome => Some(SetupStep::ProviderSelect),
            SetupStep::ProviderSelect => Some(SetupStep::CredentialEntry),
            SetupStep::CredentialEntry => Some(SetupStep::ModelSelect),
            SetupStep::ModelSelect => Some(SetupStep::Done),
            SetupStep::Done => None,
        }
    }

    /// True only for the terminal [`SetupStep::Done`] step.
    pub fn is_terminal(self) -> bool {
        matches!(self, SetupStep::Done)
    }
}

// ---------------------------------------------------------------------------
// Errors and retry actions
// ---------------------------------------------------------------------------

/// What the UI should offer after a [`SetupError`]. Variants carry no secret
/// bytes, so rendering them into logs is safe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryAction {
    RetryCredentialEntry,
    RetryModelSelect,
    ChangeProvider,
    RetryOffline,
}

/// Observable onboarding failures. No variant holds credential material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupError {
    InvalidCredential {
        reason: &'static str,
        action: RetryAction,
    },
    UnknownProvider {
        action: RetryAction,
    },
    PkceExpired {
        action: RetryAction,
    },
    NoCachedSettings {
        action: RetryAction,
    },
    WrongStep {
        action: RetryAction,
    },
    AlreadyCommitted,
}

impl SetupError {
    /// UI action that resolves this error, if any.
    pub fn action(self) -> Option<RetryAction> {
        match self {
            SetupError::InvalidCredential { action, .. } => Some(action),
            SetupError::UnknownProvider { action } => Some(action),
            SetupError::PkceExpired { action } => Some(action),
            SetupError::NoCachedSettings { action } => Some(action),
            SetupError::WrongStep { action } => Some(action),
            SetupError::AlreadyCommitted => None,
        }
    }
}

impl fmt::Display for SetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            SetupError::InvalidCredential { reason, action } => {
                write!(f, "invalid credential: {}; next: {:?}", reason, action)
            }
            SetupError::UnknownProvider { action } => {
                write!(f, "unknown provider; next: {:?}", action)
            }
            SetupError::PkceExpired { action } => {
                write!(f, "oauth pkce challenge expired; next: {:?}", action)
            }
            SetupError::NoCachedSettings { action } => {
                write!(f, "no cached settings for offline start; next: {:?}", action)
            }
            SetupError::WrongStep { action } => {
                write!(f, "setup step out of order; next: {:?}", action)
            }
            SetupError::AlreadyCommitted => write!(f, "setup already committed"),
        }
    }
}

impl std::error::Error for SetupError {}

// ---------------------------------------------------------------------------
// Secrets
// ---------------------------------------------------------------------------

/// Credential bytes. `Debug` and `Display` always emit `[REDACTED]`; the raw
/// value is visible only inside [`SecretString::with_exposed`].
#[derive(Clone)]
pub struct SecretString(String);

impl SecretString {
    /// Wrap a secret, enforcing the [`MAX_SECRET_LEN`] byte budget.
    pub fn new(secret: String) -> Result<Self, SetupError> {
        if secret.len() > MAX_SECRET_LEN {
            return Err(SetupError::InvalidCredential {
                reason: "secret exceeds size bound",
                action: RetryAction::RetryCredentialEntry,
            });
        }
        Ok(SecretString(secret))
    }

    /// Run `f` with the raw secret. Use for the provider exchange call only;
    /// never for logging.
    pub fn with_exposed<R>(&self, f: impl FnOnce(&str) -> R) -> R {
        f(&self.0)
    }

    /// Secret length in bytes (safe: reveals size, not content).
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// True when the secret holds zero bytes.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", REDACTED)
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", REDACTED)
    }
}

/// Replace every occurrence of each known secret in `text` with `[REDACTED]`.
///
/// Pass every log line, transcript row, QR payload, and exported config
/// through this before emitting. Secrets longer than [`MAX_SECRET_LEN`] or
/// empty are skipped (empty would erase the whole line).
pub fn redact_secrets_in(text: &str, secrets: &[&str]) -> String {
    let mut out = text.to_string();
    for secret in secrets {
        if secret.is_empty() || secret.len() > MAX_SECRET_LEN {
            continue;
        }
        out = out.replace(secret, REDACTED);
    }
    out
}

// ---------------------------------------------------------------------------
// OAuth PKCE challenge holder
// ---------------------------------------------------------------------------

/// Opaque OAuth PKCE challenge plus `state`, as produced by the external OAuth
/// library. This type invents no crypto: it only holds the opaque string,
/// binds it to a provider, and enforces expiry.
///
/// `Debug` is hand-written: it shows the provider, the deadline, and whether
/// a challenge is held, never the challenge or `state` bytes.
pub struct PkceChallenge {
    provider_id: String,
    opaque_challenge: String,
    state: String,
    expires_at_unix: u64,
}

impl Clone for PkceChallenge {
    fn clone(&self) -> Self {
        PkceChallenge {
            provider_id: self.provider_id.clone(),
            opaque_challenge: self.opaque_challenge.clone(),
            state: self.state.clone(),
            expires_at_unix: self.expires_at_unix,
        }
    }
}

impl PartialEq for PkceChallenge {
    fn eq(&self, other: &Self) -> bool {
        self.provider_id == other.provider_id
            && self.opaque_challenge == other.opaque_challenge
            && self.state == other.state
            && self.expires_at_unix == other.expires_at_unix
    }
}

impl Eq for PkceChallenge {}

impl fmt::Debug for PkceChallenge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PkceChallenge")
            .field("provider_id", &self.provider_id)
            .field("opaque_challenge", &REDACTED)
            .field("state", &REDACTED)
            .field("expires_at_unix", &self.expires_at_unix)
            .finish()
    }
}

impl PkceChallenge {
    /// Hold an externally produced challenge. Rejects malformed holders and
    /// already-dead (`expires_at_unix == 0`) ones.
    pub fn new(
        provider_id: &str,
        opaque_challenge: String,
        state: String,
        expires_at_unix: u64,
    ) -> Result<Self, SetupError> {
        if provider_id.is_empty() || provider_id.len() > MAX_PROVIDER_ID_LEN {
            return Err(SetupError::UnknownProvider {
                action: RetryAction::ChangeProvider,
            });
        }
        if opaque_challenge.is_empty() || opaque_challenge.len() > MAX_PKCE_OPAQUE_LEN {
            return Err(SetupError::InvalidCredential {
                reason: "pkce challenge malformed",
                action: RetryAction::ChangeProvider,
            });
        }
        if state.is_empty() || state.len() > MAX_PKCE_STATE_LEN {
            return Err(SetupError::InvalidCredential {
                reason: "pkce state malformed",
                action: RetryAction::ChangeProvider,
            });
        }
        if expires_at_unix == 0 {
            return Err(SetupError::PkceExpired {
                action: RetryAction::ChangeProvider,
            });
        }
        Ok(PkceChallenge {
            provider_id: provider_id.to_string(),
            opaque_challenge,
            state,
            expires_at_unix,
        })
    }

    /// Provider this challenge was issued for.
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// Opaque `state` (CSRF) value.
    pub fn state(&self) -> &str {
        &self.state
    }

    /// Unix timestamp after which the challenge must not be used.
    pub fn expires_at_unix(&self) -> u64 {
        self.expires_at_unix
    }

    /// Run `f` with the opaque challenge for the token-exchange call only.
    pub fn with_challenge<R>(&self, f: impl FnOnce(&str) -> R) -> R {
        f(&self.opaque_challenge)
    }

    /// True when `now_unix` has reached the challenge deadline.
    pub fn is_expired(&self, now_unix: u64) -> bool {
        now_unix >= self.expires_at_unix
    }

    /// Current Unix time in seconds; `0` when the clock is unavailable.
    pub fn now_unix() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// Account store
// ---------------------------------------------------------------------------

/// Minimal account persistence surface the session commits to. Staged means
/// "seen during setup"; committed means "authorized". Only `commit_account`
/// creates an authorized account; `cancel` removes staged-but-uncommitted
/// entries so no half-authorized account survives.
pub trait AccountStore {
    fn has_account(&self, provider_id: &str) -> bool;
    fn is_committed(&self, provider_id: &str) -> bool;
    fn stage_account(&mut self, provider_id: &str);
    fn commit_account(&mut self, provider_id: &str);
    fn remove_account(&mut self, provider_id: &str);
}

/// In-memory [`AccountStore`] for tests and the offline fixture path.
/// Production wiring swaps this for secure storage without touching the
/// session logic.
#[derive(Debug, Default)]
pub struct MemoryAccountStore {
    accounts: HashMap<String, bool>,
}

impl MemoryAccountStore {
    /// Empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of staged or committed accounts.
    pub fn account_count(&self) -> usize {
        self.accounts.len()
    }
}

impl AccountStore for MemoryAccountStore {
    fn has_account(&self, provider_id: &str) -> bool {
        self.accounts.contains_key(provider_id)
    }

    fn is_committed(&self, provider_id: &str) -> bool {
        self.accounts.get(provider_id).copied().unwrap_or(false)
    }

    fn stage_account(&mut self, provider_id: &str) {
        self.accounts.entry(provider_id.to_string()).or_insert(false);
    }

    fn commit_account(&mut self, provider_id: &str) {
        if let Some(slot) = self.accounts.get_mut(provider_id) {
            *slot = true;
        }
    }

    fn remove_account(&mut self, provider_id: &str) {
        self.accounts.remove(provider_id);
    }
}

// ---------------------------------------------------------------------------
// Onboarding session
// ---------------------------------------------------------------------------

/// Live first-run session. Borrowed store is released when the session is
/// consumed by [`OnboardingSession::cancel`] or dropped after `Done`.
#[derive(Debug)]
pub struct OnboardingSession<'a, S: AccountStore> {
    store: &'a mut S,
    step: SetupStep,
    provider_id: Option<String>,
    pkce_attached: bool,
    credential_ok: bool,
    committed: bool,
}

fn action_for_step(step: SetupStep) -> RetryAction {
    match step {
        SetupStep::Welcome | SetupStep::ProviderSelect => RetryAction::ChangeProvider,
        SetupStep::CredentialEntry => RetryAction::RetryCredentialEntry,
        SetupStep::ModelSelect | SetupStep::Done => RetryAction::RetryModelSelect,
    }
}

impl<'a, S: AccountStore> OnboardingSession<'a, S> {
    /// Start at [`SetupStep::Welcome`].
    pub fn begin(store: &'a mut S) -> Self {
        OnboardingSession {
            store,
            step: SetupStep::Welcome,
            provider_id: None,
            pkce_attached: false,
            credential_ok: false,
            committed: false,
        }
    }

    /// Current step.
    pub fn step(&self) -> SetupStep {
        self.step
    }

    /// Selected provider, if any.
    pub fn provider_id(&self) -> Option<&str> {
        self.provider_id.as_deref()
    }

    /// True once [`OnboardingSession::select_model`] committed the account.
    pub fn committed(&self) -> bool {
        self.committed
    }

    /// True once a live [`PkceChallenge`] was attached for this provider.
    pub fn pkce_attached(&self) -> bool {
        self.pkce_attached
    }

    fn require_step(&self, expected: SetupStep) -> Result<(), SetupError> {
        if self.step == expected {
            Ok(())
        } else {
            Err(SetupError::WrongStep {
                action: action_for_step(expected),
            })
        }
    }

    /// `Welcome -> ProviderSelect`.
    pub fn advance_from_welcome(&mut self) -> Result<(), SetupError> {
        self.require_step(SetupStep::Welcome)?;
        self.step = SetupStep::ProviderSelect;
        Ok(())
    }

    /// Pick a provider and stage (not authorize) its account.
    /// `ProviderSelect -> CredentialEntry`.
    pub fn select_provider(&mut self, provider_id: &str) -> Result<(), SetupError> {
        self.require_step(SetupStep::ProviderSelect)?;
        if self.committed {
            return Err(SetupError::AlreadyCommitted);
        }
        if provider_id.is_empty() || provider_id.len() > MAX_PROVIDER_ID_LEN {
            return Err(SetupError::UnknownProvider {
                action: RetryAction::ChangeProvider,
            });
        }
        self.store.stage_account(provider_id);
        self.provider_id = Some(provider_id.to_string());
        self.step = SetupStep::CredentialEntry;
        Ok(())
    }

    /// Attach a live challenge for the selected provider. Expired or
    /// mismatched challenges are rejected and authorize nothing.
    pub fn attach_pkce(&mut self, challenge: PkceChallenge) -> Result<(), SetupError> {
        self.require_step(SetupStep::CredentialEntry)?;
        let provider = self.provider_id.as_deref().ok_or(SetupError::WrongStep {
            action: RetryAction::ChangeProvider,
        })?;
        if challenge.provider_id() != provider {
            return Err(SetupError::UnknownProvider {
                action: RetryAction::ChangeProvider,
            });
        }
        if challenge.is_expired(PkceChallenge::now_unix()) {
            return Err(SetupError::PkceExpired {
                action: RetryAction::ChangeProvider,
            });
        }
        self.pkce_attached = true;
        Ok(())
    }

    /// Validate a credential without persisting it. Invalid input returns
    /// [`SetupError::InvalidCredential`] with a retry action and leaves the
    /// session on `CredentialEntry` so the user can retry. The secret is
    /// never copied into the error.
    pub fn submit_credential(&mut self, secret: &SecretString) -> Result<(), SetupError> {
        self.require_step(SetupStep::CredentialEntry)?;
        if self.committed {
            return Err(SetupError::AlreadyCommitted);
        }
        let mut ok = false;
        secret.with_exposed(|raw| {
            // Fixture provider rule: keys are `sk-`-prefixed tokens without
            // whitespace/control bytes, within the size bound.
            ok = raw.len() >= MIN_CREDENTIAL_LEN
                && raw.len() <= MAX_SECRET_LEN
                && raw.starts_with("sk-")
                && !raw.chars().any(|c| c.is_control() || c.is_whitespace());
        });
        if !ok {
            return Err(SetupError::InvalidCredential {
                reason: "credential rejected",
                action: RetryAction::RetryCredentialEntry,
            });
        }
        self.credential_ok = true;
        self.step = SetupStep::ModelSelect;
        Ok(())
    }

    /// Pick a model and commit (authorize) the staged account.
    /// `ModelSelect -> Done`.
    pub fn select_model(&mut self, model_id: &str) -> Result<(), SetupError> {
        self.require_step(SetupStep::ModelSelect)?;
        if !self.credential_ok {
            return Err(SetupError::WrongStep {
                action: RetryAction::RetryCredentialEntry,
            });
        }
        if model_id.is_empty() || model_id.len() > MAX_MODEL_ID_LEN {
            return Err(SetupError::InvalidCredential {
                reason: "unknown model",
                action: RetryAction::RetryModelSelect,
            });
        }
        if let Some(provider) = self.provider_id.clone() {
            self.store.commit_account(&provider);
        }
        self.committed = true;
        self.step = SetupStep::Done;
        Ok(())
    }

    /// Abort setup. Consumes the session so no further step can run, removes
    /// any staged-but-uncommitted account, and returns a receipt proving no
    /// half-authorized account remains. A previously committed account is
    /// left untouched.
    pub fn cancel(self) -> CancelReceipt {
        let mut staged_removed = false;
        if !self.committed {
            if let Some(provider) = self.provider_id.as_deref() {
                if self.store.has_account(provider) && !self.store.is_committed(provider) {
                    self.store.remove_account(provider);
                    staged_removed = self.store.has_account(provider);
                    staged_removed = !staged_removed;
                }
            }
        }
        CancelReceipt {
            provider_id: self.provider_id.clone(),
            staged_removed,
            committed: self.committed,
        }
    }
}

// ---------------------------------------------------------------------------
// Cancel receipt
// ---------------------------------------------------------------------------

/// Proof that [`OnboardingSession::cancel`] left no half-authorized account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelReceipt {
    /// Provider that was staged when cancel ran, if any.
    pub provider_id: Option<String>,
    /// True when a staged-but-uncommitted account was removed.
    pub staged_removed: bool,
    /// True when the session had already committed (account kept by design).
    pub committed: bool,
}

impl CancelReceipt {
    /// True when no half-authorized account remains in `store`: nothing
    /// staged for a cancelled session, or a fully committed account for a
    /// session that had already completed.
    pub fn left_no_half_account<S: AccountStore>(&self, store: &S) -> bool {
        match self.provider_id.as_deref() {
            None => true,
            Some(provider) => {
                if self.committed {
                    store.is_committed(provider)
                } else {
                    !store.has_account(provider)
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Offline cached-settings path
// ---------------------------------------------------------------------------

/// Previously stored provider/model pair reused without hosted login.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedSettings {
    provider_id: String,
    model_id: String,
}

impl CachedSettings {
    /// Load cached settings; empty or over-bound ids are rejected.
    pub fn new(provider_id: &str, model_id: &str) -> Result<Self, SetupError> {
        if provider_id.is_empty()
            || provider_id.len() > MAX_PROVIDER_ID_LEN
            || model_id.is_empty()
            || model_id.len() > MAX_MODEL_ID_LEN
        {
            return Err(SetupError::NoCachedSettings {
                action: RetryAction::RetryOffline,
            });
        }
        Ok(CachedSettings {
            provider_id: provider_id.to_string(),
            model_id: model_id.to_string(),
        })
    }

    /// Cached provider id.
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// Cached model id.
    pub fn model_id(&self) -> &str {
        &self.model_id
    }
}

/// Offline session pinned to cached settings. Never requires hosted login.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineSession {
    provider_id: String,
    model_id: String,
}

impl OfflineSession {
    /// Provider served from cache.
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// Model served from cache.
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    /// Offline start never requires hosted login.
    pub fn hosted_login_required(&self) -> bool {
        false
    }
}

/// Start offline from cached settings/catalog and local models. `None` (no
/// cache) is a routing error back to online setup, not a login prompt.
pub fn start_offline(cached: Option<CachedSettings>) -> Result<OfflineSession, SetupError> {
    match cached {
        Some(settings) => Ok(OfflineSession {
            provider_id: settings.provider_id,
            model_id: settings.model_id,
        }),
        None => Err(SetupError::NoCachedSettings {
            action: RetryAction::RetryOffline,
        }),
    }
}

// ---------------------------------------------------------------------------
// Tests (frozen RED: must compile, must fail for the missing behavior)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_secret() -> SecretString {
        SecretString::new("sk-test-0123456789".to_string()).unwrap()
    }

    fn session_to_credential<'a>(
        store: &'a mut MemoryAccountStore,
    ) -> OnboardingSession<'a, MemoryAccountStore> {
        let mut session = OnboardingSession::begin(store);
        session.advance_from_welcome().unwrap();
        session.select_provider("openai").unwrap();
        session
    }

    #[test]
    fn setup_step_order() {
        assert_eq!(SetupStep::Welcome.next(), Some(SetupStep::ProviderSelect));
        assert_eq!(
            SetupStep::ProviderSelect.next(),
            Some(SetupStep::CredentialEntry)
        );
        assert_eq!(
            SetupStep::CredentialEntry.next(),
            Some(SetupStep::ModelSelect)
        );
        assert_eq!(SetupStep::ModelSelect.next(), Some(SetupStep::Done));
        assert_eq!(SetupStep::Done.next(), None);
        assert!(!SetupStep::Welcome.is_terminal());
        assert!(SetupStep::Done.is_terminal());
    }

    #[test]
    fn cancel_leaves_no_half_account() {
        let mut store = MemoryAccountStore::new();
        let receipt = {
            let session = session_to_credential(&mut store);
            assert_eq!(session.provider_id(), Some("openai"));
            assert!(!session.committed());
            session.cancel()
        };
        assert_eq!(receipt.provider_id.as_deref(), Some("openai"));
        assert!(receipt.staged_removed);
        assert!(!receipt.committed);
        assert!(receipt.left_no_half_account(&store));
        assert!(!store.has_account("openai"));
        assert_eq!(store.account_count(), 0);
    }

    #[test]
    fn cancel_before_provider_selection_leaves_clean() {
        let mut store = MemoryAccountStore::new();
        let receipt = {
            let session = OnboardingSession::begin(&mut store);
            session.cancel()
        };
        assert_eq!(receipt.provider_id, None);
        assert!(!receipt.staged_removed);
        assert!(!receipt.committed);
        assert!(receipt.left_no_half_account(&store));
        assert_eq!(store.account_count(), 0);
    }

    #[test]
    fn complete_flow_commits_account() {
        let mut store = MemoryAccountStore::new();
        {
            let mut session = session_to_credential(&mut store);
            session.submit_credential(&valid_secret()).unwrap();
            assert_eq!(session.step(), SetupStep::ModelSelect);
            session.select_model("gpt-4o-mini").unwrap();
            assert_eq!(session.step(), SetupStep::Done);
            assert!(session.committed());
        }
        assert!(store.is_committed("openai"));
    }

    #[test]
    fn invalid_credential_surfaces_retry() {
        let mut store = MemoryAccountStore::new();
        let mut session = session_to_credential(&mut store);
        let bad = SecretString::new("abc".to_string()).unwrap();
        let err = session.submit_credential(&bad).unwrap_err();
        assert_eq!(
            err,
            SetupError::InvalidCredential {
                reason: "credential rejected",
                action: RetryAction::RetryCredentialEntry,
            }
        );
        assert_eq!(err.action(), Some(RetryAction::RetryCredentialEntry));
        // Still on credential entry so the user can retry; nothing authorized.
        assert_eq!(session.step(), SetupStep::CredentialEntry);
        assert!(!session.committed());
        // Retry with a valid credential proceeds.
        session.submit_credential(&valid_secret()).unwrap();
        assert_eq!(session.step(), SetupStep::ModelSelect);
        let receipt = session.cancel();
        assert!(receipt.left_no_half_account(&store));
    }

    #[test]
    fn offline_cached_settings_path() {
        let cached = CachedSettings::new("local", "llama3").unwrap();
        let session = start_offline(Some(cached)).unwrap();
        assert!(!session.hosted_login_required());
        assert_eq!(session.provider_id(), "local");
        assert_eq!(session.model_id(), "llama3");
        let err = start_offline(None).unwrap_err();
        assert_eq!(
            err,
            SetupError::NoCachedSettings {
                action: RetryAction::RetryOffline,
            }
        );
        assert_eq!(err.action(), Some(RetryAction::RetryOffline));
    }

    #[test]
    fn secret_never_logged() {
        let raw = "sk-live-supersecret-zz9";
        let secret = SecretString::new(raw.to_string()).unwrap();
        assert!(!secret.is_empty());
        assert_eq!(secret.len(), raw.len());
        assert_eq!(format!("{:?}", secret), "[REDACTED]");
        assert_eq!(format!("{}", secret), "[REDACTED]");
        let line = format!("login failed for key {}", raw);
        let clean = redact_secrets_in(&line, &[raw]);
        assert!(!clean.contains(raw));
        assert!(clean.contains("[REDACTED]"));
        // Error values carry no secret bytes either.
        let mut store = MemoryAccountStore::new();
        let mut session = session_to_credential(&mut store);
        let bad = SecretString::new("zz9-bad-key".to_string()).unwrap();
        let err = session.submit_credential(&bad).unwrap_err();
        let rendered = format!("{} {:?}", err, err);
        assert!(!rendered.contains("zz9-bad-key"));
        let _ = session.cancel();
    }

    #[test]
    fn pkce_expiry_rejected() {
        let expired =
            PkceChallenge::new("openai", "opaque-abc".to_string(), "state-xyz".to_string(), 1)
                .unwrap();
        assert_eq!(expired.provider_id(), "openai");
        assert_eq!(expired.state(), "state-xyz");
        assert_eq!(expired.expires_at_unix(), 1);
        assert!(expired.is_expired(u64::MAX));
        assert!(expired.is_expired(PkceChallenge::now_unix()));
        let held = expired.with_challenge(|c| c.to_string());
        assert_eq!(held, "opaque-abc");
        // Holder Debug must not leak challenge or state bytes.
        let dbg = format!("{:?}", expired);
        assert!(!dbg.contains("opaque-abc"));
        assert!(!dbg.contains("state-xyz"));
        // Attaching an expired challenge authorizes nothing.
        let mut store = MemoryAccountStore::new();
        let mut session = session_to_credential(&mut store);
        assert!(!session.pkce_attached());
        let err = session.attach_pkce(expired).unwrap_err();
        assert_eq!(err.action(), Some(RetryAction::ChangeProvider));
        assert!(!session.pkce_attached());
        assert!(!session.committed());
        let receipt = session.cancel();
        assert!(receipt.left_no_half_account(&store));
    }
}
