//! Bounded, secret-free authentication command planning.
//!
//! This module owns metadata and lifecycle intent only.  Credential bytes,
//! OAuth callbacks, persistence, and provider execution remain caller-owned.
//! Every transition is synchronous and deterministic; `AuthState::now_ms` is
//! caller supplied and is never read from the system clock.

#![forbid(unsafe_code)]

use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::fmt;
use std::ops::Deref;

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

/// Maximum provider identifiers retained by one caller-owned state value.
pub const MAX_AUTH_PROVIDERS: usize = 128;
/// Maximum provider identifier length in bytes.
pub const MAX_PROVIDER_ID_BYTES: usize = 128;
/// Maximum non-secret account-label length in bytes.
pub const MAX_ACCOUNT_LABEL_BYTES: usize = 256;
/// Maximum auth mode/provenance/source length in bytes.
pub const MAX_METADATA_BYTES: usize = 128;
/// Maximum rows returned by [`list`].
pub const MAX_AUTH_STATUS_ENTRIES: usize = 32;
/// Deterministic fallback extension used when the caller did not provide a
/// provider-specific refresh expiry.
pub const DEFAULT_REFRESH_EXTENSION_MS: u64 = 3_600_000;

const LOGGED_OUT_MODE: &str = "none";
const LOGGED_OUT_PROVENANCE: &str = "none";

/// User-facing authentication command set.
///
/// There is deliberately no raw-key, access-token, refresh-token, or secret
/// bearing variant. `source` is an opaque caller label and is never retained
/// by the planner.
#[derive(Clone, Eq, PartialEq)]
pub enum AuthCommand {
    Connect { provider: String },
    Login { provider: String },
    Import { provider: String, source: String },
    List,
    Inspect { provider: String },
    Refresh { provider: String },
    Logout { provider: String },
}

impl fmt::Debug for AuthCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connect { provider } => f
                .debug_struct("Connect")
                .field("provider", &safe_text(provider))
                .finish(),
            Self::Login { provider } => f
                .debug_struct("Login")
                .field("provider", &safe_text(provider))
                .finish(),
            Self::Import { provider, .. } => f
                .debug_struct("Import")
                .field("provider", &safe_text(provider))
                .field("source", &"<redacted>")
                .finish(),
            Self::List => f.write_str("List"),
            Self::Inspect { provider } => f
                .debug_struct("Inspect")
                .field("provider", &safe_text(provider))
                .finish(),
            Self::Refresh { provider } => f
                .debug_struct("Refresh")
                .field("provider", &safe_text(provider))
                .finish(),
            Self::Logout { provider } => f
                .debug_struct("Logout")
                .field("provider", &safe_text(provider))
                .finish(),
        }
    }
}

impl Serialize for AuthCommand {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut out = match self {
            Self::Connect { .. } => serializer.serialize_struct("AuthCommand", 2)?,
            Self::Login { .. } => serializer.serialize_struct("AuthCommand", 2)?,
            Self::Import { .. } => serializer.serialize_struct("AuthCommand", 3)?,
            Self::List => return serializer.serialize_str("List"),
            Self::Inspect { .. } => serializer.serialize_struct("AuthCommand", 2)?,
            Self::Refresh { .. } => serializer.serialize_struct("AuthCommand", 2)?,
            Self::Logout { .. } => serializer.serialize_struct("AuthCommand", 2)?,
        };
        match self {
            Self::Connect { provider } => {
                out.serialize_field("command", "connect")?;
                out.serialize_field("provider", &safe_text(provider))?;
            }
            Self::Login { provider } => {
                out.serialize_field("command", "login")?;
                out.serialize_field("provider", &safe_text(provider))?;
            }
            Self::Import { provider, .. } => {
                out.serialize_field("command", "import")?;
                out.serialize_field("provider", &safe_text(provider))?;
                // Import paths/labels are caller-owned and must not become a
                // diagnostic or wire-secret channel.
                out.serialize_field("source", "<redacted>")?;
            }
            Self::Inspect { provider } => {
                out.serialize_field("command", "inspect")?;
                out.serialize_field("provider", &safe_text(provider))?;
            }
            Self::Refresh { provider } => {
                out.serialize_field("command", "refresh")?;
                out.serialize_field("provider", &safe_text(provider))?;
            }
            Self::Logout { provider } => {
                out.serialize_field("command", "logout")?;
                out.serialize_field("provider", &safe_text(provider))?;
            }
            Self::List => unreachable!("List was serialized above"),
        }
        out.end()
    }
}

/// Redacted status projection exposed to callers and serializers.
#[derive(Clone, Eq, PartialEq)]
pub struct AuthStatus {
    pub provider: String,
    pub account_label: String,
    pub auth_mode: String,
    pub provenance: String,
    pub expires_at_ms: Option<u64>,
    pub error: Option<String>,
}

impl fmt::Debug for AuthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthStatus")
            .field("provider", &safe_text(&self.provider))
            .field("account_label", &safe_text(&self.account_label))
            .field("auth_mode", &safe_text(&self.auth_mode))
            .field("provenance", &safe_text(&self.provenance))
            .field("expires_at_ms", &self.expires_at_ms)
            .field("error", &self.error.as_deref().map(safe_text))
            .finish()
    }
}

impl Serialize for AuthStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut out = serializer.serialize_struct("AuthStatus", 6)?;
        out.serialize_field("provider", &safe_text(&self.provider))?;
        out.serialize_field("account_label", &safe_text(&self.account_label))?;
        out.serialize_field("auth_mode", &safe_text(&self.auth_mode))?;
        out.serialize_field("provenance", &safe_text(&self.provenance))?;
        out.serialize_field("expires_at_ms", &self.expires_at_ms)?;
        out.serialize_field("error", &self.error.as_deref().map(safe_text))?;
        out.end()
    }
}

impl AuthStatus {
    /// Construct a logged-out status without credential material.
    pub fn logged_out(provider: impl Into<String>) -> Result<Self, AuthCommandError> {
        let provider = provider.into();
        validate_provider(&provider)?;
        Ok(Self {
            provider,
            account_label: String::new(),
            auth_mode: LOGGED_OUT_MODE.to_owned(),
            provenance: LOGGED_OUT_PROVENANCE.to_owned(),
            expires_at_ms: None,
            error: None,
        })
    }

    /// Construct a ready status from non-secret metadata.
    pub fn ready(
        provider: impl Into<String>,
        account_label: impl Into<String>,
        auth_mode: impl Into<String>,
        provenance: impl Into<String>,
        expires_at_ms: u64,
    ) -> Result<Self, AuthCommandError> {
        let status = Self {
            provider: provider.into(),
            account_label: account_label.into(),
            auth_mode: auth_mode.into(),
            provenance: provenance.into(),
            expires_at_ms: Some(expires_at_ms),
            error: None,
        };
        validate_status(&status)?;
        Ok(status)
    }

    /// True when the status carries a usable non-secret auth shape.
    #[must_use]
    pub fn is_logged_in(&self) -> bool {
        self.expires_at_ms.is_some() && self.auth_mode != LOGGED_OUT_MODE
    }
}

/// Result of one planned command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuthEffect {
    pub status: AuthStatus,
    pub needs_consent: bool,
}

/// Bounded list projection. It dereferences to the returned status slice for
/// callers that only need the `Vec`-like behavior while retaining truncation
/// metadata required by the status surface.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct AuthList {
    pub statuses: Vec<AuthStatus>,
    pub truncated: bool,
}

impl Deref for AuthList {
    type Target = [AuthStatus];

    fn deref(&self) -> &Self::Target {
        &self.statuses
    }
}

impl AsRef<[AuthStatus]> for AuthList {
    fn as_ref(&self) -> &[AuthStatus] {
        &self.statuses
    }
}

impl<'a> IntoIterator for &'a AuthList {
    type Item = &'a AuthStatus;
    type IntoIter = std::slice::Iter<'a, AuthStatus>;

    fn into_iter(self) -> Self::IntoIter {
        self.statuses.iter()
    }
}

impl IntoIterator for AuthList {
    type Item = AuthStatus;
    type IntoIter = std::vec::IntoIter<AuthStatus>;

    fn into_iter(self) -> Self::IntoIter {
        self.statuses.into_iter()
    }
}

impl PartialEq<Vec<AuthStatus>> for AuthList {
    fn eq(&self, other: &Vec<AuthStatus>) -> bool {
        self.statuses == *other
    }
}

/// Non-secret completion material supplied by a trusted caller after consent.
/// This type intentionally has no token, key, code, or file-content field.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuthGrant {
    pub provider: String,
    pub account_label: String,
    pub auth_mode: String,
    pub provenance: String,
    pub expires_at_ms: u64,
}

pub type AuthCompletion = AuthGrant;

impl AuthGrant {
    #[must_use]
    pub fn new(
        provider: impl Into<String>,
        account_label: impl Into<String>,
        auth_mode: impl Into<String>,
        provenance: impl Into<String>,
        expires_at_ms: u64,
    ) -> Self {
        Self {
            provider: provider.into(),
            account_label: account_label.into(),
            auth_mode: auth_mode.into(),
            provenance: provenance.into(),
            expires_at_ms,
        }
    }

    #[must_use]
    pub fn oauth(
        provider: impl Into<String>,
        account_label: impl Into<String>,
        expires_at_ms: u64,
    ) -> Self {
        Self::new(
            provider,
            account_label,
            "oauth",
            "official-oauth",
            expires_at_ms,
        )
    }

    #[must_use]
    pub fn api_key(
        provider: impl Into<String>,
        account_label: impl Into<String>,
        expires_at_ms: u64,
    ) -> Self {
        Self::new(provider, account_label, "api-key", "api-key", expires_at_ms)
    }

    #[must_use]
    pub fn imported(
        provider: impl Into<String>,
        account_label: impl Into<String>,
        auth_mode: impl Into<String>,
        expires_at_ms: u64,
    ) -> Self {
        Self::new(
            provider,
            account_label,
            auth_mode,
            "imported-local",
            expires_at_ms,
        )
    }
}

/// Typed planner failures. Variants carry no caller text or credential bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, thiserror::Error)]
pub enum AuthCommandError {
    #[error("provider id is empty")]
    EmptyProvider,
    #[error("provider id is invalid")]
    InvalidProvider,
    #[error("unknown provider")]
    UnknownProvider,
    #[error("authorization consent required")]
    ConsentRequired,
    #[error("provider is not logged in")]
    NotLoggedIn,
    #[error("account metadata is invalid")]
    InvalidMetadata,
    #[error("expiry is invalid")]
    InvalidExpiry,
    #[error("too many providers")]
    TooManyProviders,
}

#[derive(Clone, Debug)]
struct AuthRecord {
    status: AuthStatus,
    next_refresh_expiry_ms: Option<u64>,
}

/// Caller-owned bounded auth metadata state.
///
/// It stores status metadata only. No credential material, callback, task,
/// filesystem handle, database handle, or network resource is retained.
#[derive(Clone, Default)]
pub struct AuthState {
    records: BTreeMap<String, AuthRecord>,
    now_ms: u64,
}

impl fmt::Debug for AuthState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthState")
            .field("statuses", &list(self))
            .field("now_ms", &self.now_ms)
            .finish()
    }
}

impl AuthState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the caller's logical time. No system clock is read by this module.
    pub fn set_now_ms(&mut self, now_ms: u64) {
        self.now_ms = now_ms;
    }

    #[must_use]
    pub fn now_ms(&self) -> u64 {
        self.now_ms
    }

    /// Register a provider as logged out. Duplicate registration is harmless.
    pub fn register_provider(
        &mut self,
        provider: impl Into<String>,
    ) -> Result<(), AuthCommandError> {
        let provider = provider.into();
        validate_provider(&provider)?;
        if self.records.contains_key(&provider) {
            return Ok(());
        }
        if self.records.len() >= MAX_AUTH_PROVIDERS {
            return Err(AuthCommandError::TooManyProviders);
        }
        let status = AuthStatus::logged_out(provider.clone())?;
        self.records.insert(
            provider,
            AuthRecord {
                status,
                next_refresh_expiry_ms: None,
            },
        );
        Ok(())
    }

    /// Alias for callers that model connection as provider registration.
    pub fn connect(&mut self, provider: impl Into<String>) -> Result<AuthStatus, AuthCommandError> {
        let provider = provider.into();
        self.register_provider(provider.clone())?;
        self.status(&provider)
            .ok_or(AuthCommandError::UnknownProvider)
    }

    /// Apply a caller-supplied, already-authorized non-secret grant.
    pub fn apply_grant(&mut self, grant: AuthGrant) -> Result<AuthStatus, AuthCommandError> {
        validate_grant(&grant, self.now_ms)?;
        self.register_provider(grant.provider.clone())?;
        let status = AuthStatus::ready(
            grant.provider.clone(),
            grant.account_label,
            grant.auth_mode,
            grant.provenance,
            grant.expires_at_ms,
        )?;
        let record = self
            .records
            .get_mut(&grant.provider)
            .ok_or(AuthCommandError::UnknownProvider)?;
        record.status = status.clone();
        Ok(status)
    }

    /// Alias for grant-backed login completion.
    pub fn complete_login(&mut self, grant: AuthGrant) -> Result<AuthStatus, AuthCommandError> {
        self.apply_grant(grant)
    }

    /// Alias for callers that call completion simply `complete`.
    pub fn complete(&mut self, grant: AuthGrant) -> Result<AuthStatus, AuthCommandError> {
        self.apply_grant(grant)
    }

    /// Explicit name for a caller completing a consent flow.
    pub fn complete_grant(&mut self, grant: AuthGrant) -> Result<AuthStatus, AuthCommandError> {
        self.apply_grant(grant)
    }

    /// Alias matching completion-oriented callers.
    pub fn apply_completion(
        &mut self,
        grant: AuthCompletion,
    ) -> Result<AuthStatus, AuthCommandError> {
        self.apply_grant(grant)
    }

    /// Seed or replace a status using only redacted metadata.
    pub fn insert_status(&mut self, status: AuthStatus) -> Result<(), AuthCommandError> {
        validate_status(&status)?;
        if !self.records.contains_key(&status.provider) && self.records.len() >= MAX_AUTH_PROVIDERS
        {
            return Err(AuthCommandError::TooManyProviders);
        }
        self.records.insert(
            status.provider.clone(),
            AuthRecord {
                status,
                next_refresh_expiry_ms: None,
            },
        );
        Ok(())
    }

    /// Insert or replace a redacted status.
    pub fn insert(&mut self, status: AuthStatus) -> Result<(), AuthCommandError> {
        self.insert_status(status)
    }

    /// Build state from caller-owned redacted statuses.
    pub fn from_statuses<I>(statuses: I) -> Result<Self, AuthCommandError>
    where
        I: IntoIterator<Item = AuthStatus>,
    {
        let mut state = Self::new();
        for status in statuses {
            state.insert_status(status)?;
        }
        Ok(state)
    }

    /// Alias for [`AuthState::from_statuses`].
    pub fn with_statuses<I>(statuses: I) -> Result<Self, AuthCommandError>
    where
        I: IntoIterator<Item = AuthStatus>,
    {
        Self::from_statuses(statuses)
    }

    /// Set the expiry that the next `Refresh` command should use.
    ///
    /// This is caller-owned provider output, not a refresh operation. It makes
    /// the subsequent planner transition deterministic without a clock or
    /// network callback.
    pub fn set_refresh_expiry(
        &mut self,
        provider: &str,
        expires_at_ms: u64,
    ) -> Result<(), AuthCommandError> {
        validate_provider(provider)?;
        if expires_at_ms <= self.now_ms {
            return Err(AuthCommandError::InvalidExpiry);
        }
        let record = self
            .records
            .get_mut(provider)
            .ok_or(AuthCommandError::UnknownProvider)?;
        record.next_refresh_expiry_ms = Some(expires_at_ms);
        Ok(())
    }

    /// Alias for [`AuthState::set_refresh_expiry`].
    pub fn refresh_expiry(
        &mut self,
        provider: &str,
        expires_at_ms: u64,
    ) -> Result<(), AuthCommandError> {
        self.set_refresh_expiry(provider, expires_at_ms)
    }

    /// Apply logout directly, without constructing an [`AuthCommand`].
    pub fn logout(&mut self, provider: &str) -> Result<AuthStatus, AuthCommandError> {
        let effect = dispatch(
            AuthCommand::Logout {
                provider: provider.to_owned(),
            },
            self,
        )?;
        Ok(effect.status)
    }

    /// Read one cloned status projection.
    #[must_use]
    pub fn status(&self, provider: &str) -> Option<AuthStatus> {
        self.records
            .get(provider)
            .map(|record| record.status.clone())
    }

    /// Number of registered provider identities.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

/// Plan one command against caller-owned state.
///
/// `Login` and `Import` are consent-gated and never mutate state. A trusted
/// caller completes either flow separately with [`AuthState::apply_grant`].
/// `Connect`, `Refresh`, and `Logout` mutate only redacted in-memory metadata.
pub fn dispatch<C>(cmd: C, state: &mut AuthState) -> Result<AuthEffect, AuthCommandError>
where
    C: Borrow<AuthCommand>,
{
    match cmd.borrow() {
        AuthCommand::Connect { provider } => {
            let status = state.connect(provider.clone())?;
            Ok(AuthEffect {
                status,
                needs_consent: false,
            })
        }
        AuthCommand::Login { provider } => {
            let provider = checked_provider(provider)?;
            // Login is a consent request, not a state mutation. The provider
            // may be new; the grant completion registers it later.
            let status = state.status(&provider).unwrap_or(AuthStatus {
                provider,
                account_label: String::new(),
                auth_mode: LOGGED_OUT_MODE.to_owned(),
                provenance: LOGGED_OUT_PROVENANCE.to_owned(),
                expires_at_ms: None,
                error: None,
            });
            Ok(AuthEffect {
                status,
                needs_consent: true,
            })
        }
        AuthCommand::Import { provider, source } => {
            let provider = checked_provider(provider)?;
            validate_source(source)?;
            // Import is also a consent request. Source bytes/path are not
            // read here, and a new provider remains absent until completion.
            let status = state.status(&provider).unwrap_or(AuthStatus {
                provider,
                account_label: String::new(),
                auth_mode: LOGGED_OUT_MODE.to_owned(),
                provenance: LOGGED_OUT_PROVENANCE.to_owned(),
                expires_at_ms: None,
                error: None,
            });
            Ok(AuthEffect {
                status,
                needs_consent: true,
            })
        }
        AuthCommand::List => {
            let statuses = list(state);
            let status = statuses
                .statuses
                .first()
                .cloned()
                .unwrap_or_else(|| AuthStatus {
                    provider: "list".to_owned(),
                    account_label: String::new(),
                    auth_mode: LOGGED_OUT_MODE.to_owned(),
                    provenance: LOGGED_OUT_PROVENANCE.to_owned(),
                    expires_at_ms: None,
                    error: None,
                });
            Ok(AuthEffect {
                status,
                needs_consent: false,
            })
        }
        AuthCommand::Inspect { provider } => {
            let provider = checked_provider(provider)?;
            inspect(state, &provider).map(|status| AuthEffect {
                status,
                needs_consent: false,
            })
        }
        AuthCommand::Refresh { provider } => {
            let provider = checked_provider(provider)?;
            let record = state
                .records
                .get_mut(&provider)
                .ok_or(AuthCommandError::UnknownProvider)?;
            if !record.status.is_logged_in() {
                return Err(AuthCommandError::NotLoggedIn);
            }
            let current_expiry = record
                .status
                .expires_at_ms
                .ok_or(AuthCommandError::NotLoggedIn)?;
            let next_expiry = record
                .next_refresh_expiry_ms
                .unwrap_or_else(|| current_expiry.saturating_add(DEFAULT_REFRESH_EXTENSION_MS));
            if next_expiry <= state.now_ms {
                return Err(AuthCommandError::InvalidExpiry);
            }
            record.next_refresh_expiry_ms = None;
            record.status.expires_at_ms = Some(next_expiry);
            record.status.error = None;
            Ok(AuthEffect {
                status: record.status.clone(),
                needs_consent: false,
            })
        }
        AuthCommand::Logout { provider } => {
            let provider = checked_provider(provider)?;
            let record = state
                .records
                .get_mut(&provider)
                .ok_or(AuthCommandError::UnknownProvider)?;
            record.status = AuthStatus::logged_out(provider)?;
            record.next_refresh_expiry_ms = None;
            Ok(AuthEffect {
                status: record.status.clone(),
                needs_consent: false,
            })
        }
    }
}

/// Return deterministic provider-id ordered statuses, capped at 32 entries.
#[must_use]
pub fn list(state: &AuthState) -> AuthList {
    let mut statuses: Vec<AuthStatus> = state
        .records
        .values()
        .map(|record| record.status.clone())
        .collect();
    let truncated = statuses.len() > MAX_AUTH_STATUS_ENTRIES;
    statuses.truncate(MAX_AUTH_STATUS_ENTRIES);
    AuthList {
        statuses,
        truncated,
    }
}

/// Inspect one provider without exposing internal state or credential data.
pub fn inspect(state: &AuthState, provider: &str) -> Result<AuthStatus, AuthCommandError> {
    let provider = checked_provider(provider)?;
    state
        .records
        .get(&provider)
        .map(|record| record.status.clone())
        .ok_or(AuthCommandError::UnknownProvider)
}

fn checked_provider(provider: &str) -> Result<String, AuthCommandError> {
    validate_provider(provider)?;
    Ok(provider.to_owned())
}

fn validate_provider(provider: &str) -> Result<(), AuthCommandError> {
    if provider.is_empty() {
        return Err(AuthCommandError::EmptyProvider);
    }
    if provider.len() > MAX_PROVIDER_ID_BYTES {
        return Err(AuthCommandError::InvalidProvider);
    }
    Ok(())
}

fn validate_source(source: &str) -> Result<(), AuthCommandError> {
    if source.len() > MAX_METADATA_BYTES {
        return Err(AuthCommandError::InvalidMetadata);
    }
    Ok(())
}

fn validate_metadata(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_METADATA_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_graphic() || byte == b' ')
        && !value.contains("sk-")
        && !value.contains("refresh_token")
}

fn validate_status(status: &AuthStatus) -> Result<(), AuthCommandError> {
    validate_provider(&status.provider)?;
    if status.account_label.len() > MAX_ACCOUNT_LABEL_BYTES {
        return Err(AuthCommandError::InvalidMetadata);
    }
    if status.auth_mode == LOGGED_OUT_MODE && status.expires_at_ms.is_some() {
        return Err(AuthCommandError::InvalidMetadata);
    }
    if status.expires_at_ms.is_some() {
        if !validate_metadata(&status.account_label)
            || !validate_metadata(&status.auth_mode)
            || !validate_metadata(&status.provenance)
        {
            return Err(AuthCommandError::InvalidMetadata);
        }
    }
    if let Some(error) = status.error.as_deref() {
        if error.len() > MAX_METADATA_BYTES || error.contains("sk-") {
            return Err(AuthCommandError::InvalidMetadata);
        }
    }
    Ok(())
}

fn validate_grant(grant: &AuthGrant, now_ms: u64) -> Result<(), AuthCommandError> {
    let status = AuthStatus {
        provider: grant.provider.clone(),
        account_label: grant.account_label.clone(),
        auth_mode: grant.auth_mode.clone(),
        provenance: grant.provenance.clone(),
        expires_at_ms: Some(grant.expires_at_ms),
        error: None,
    };
    validate_status(&status)?;
    if grant.expires_at_ms <= now_ms {
        return Err(AuthCommandError::InvalidExpiry);
    }
    Ok(())
}

fn safe_text(value: &str) -> String {
    if value.contains("sk-") || value.contains("refresh_token") {
        "<redacted>".to_owned()
    } else {
        value.to_owned()
    }
}
