//! Secret-free provider authentication identity and status projection.
//!
//! This module intentionally stores only the declared authentication shape and
//! its provenance. Credential material belongs to the caller-owned auth
//! boundary in [`crate::auth`]; it cannot cross this API by type.

use std::fmt;

use serde::Serialize;
use thiserror::Error;

/// Maximum provider identifier length, measured in bytes.
pub const MAX_PROVIDER_ID_LEN: usize = 128;

/// Source from which a provider credential was obtained.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
pub enum AuthProvenance {
    /// A provider API key supplied explicitly by the user.
    #[serde(rename = "api-key")]
    ApiKey,
    /// An official provider OAuth flow completed with human authority.
    #[serde(rename = "official-oauth")]
    OfficialOAuth,
    /// A credential explicitly imported from a local credential file.
    #[serde(rename = "imported-local")]
    ImportedLocal,
    /// A credential resolved through an OS keyring.
    #[serde(rename = "keyring")]
    Keyring,
}

impl AuthProvenance {
    /// Stable status and serialization label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ApiKey => "api-key",
            Self::OfficialOAuth => "official-oauth",
            Self::ImportedLocal => "imported-local",
            Self::Keyring => "keyring",
        }
    }
}

impl fmt::Display for AuthProvenance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str((*self).as_str())
    }
}

/// Non-secret authentication method shape.
///
/// Unlike [`crate::auth::AuthMethod`], these variants carry no token or
/// expiration data. `Unknown` exists so callers can represent an unrecognized
/// external method at the validation boundary without accepting a string or
/// credential payload.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
pub enum AuthMethodKind {
    /// API-key authentication.
    #[serde(rename = "api-key")]
    ApiKeyKind,
    /// Bearer-token authentication.
    #[serde(rename = "bearer-token")]
    BearerTokenKind,
    /// OAuth2 authentication.
    #[serde(rename = "oauth2")]
    OAuth2Kind,
    /// Method kind not recognized by this version.
    #[serde(rename = "unknown")]
    Unknown,
}

impl Default for AuthMethodKind {
    fn default() -> Self {
        Self::Unknown
    }
}

impl AuthMethodKind {
    /// Stable status and serialization label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ApiKeyKind => "api-key",
            Self::BearerTokenKind => "bearer-token",
            Self::OAuth2Kind => "oauth2",
            Self::Unknown => "unknown",
        }
    }

    /// Whether this is a supported method shape.
    #[must_use]
    pub const fn is_known(self) -> bool {
        !matches!(self, Self::Unknown)
    }

    /// Compatibility spelling for an API-key method shape.
    #[allow(non_upper_case_globals)]
    pub const ApiKey: Self = Self::ApiKeyKind;
    /// Compatibility spelling for a bearer-token method shape.
    #[allow(non_upper_case_globals)]
    pub const BearerToken: Self = Self::BearerTokenKind;
    /// Compatibility spelling for an OAuth2 method shape.
    #[allow(non_upper_case_globals)]
    pub const OAuth2: Self = Self::OAuth2Kind;
    /// Compatibility spelling for an unknown method shape.
    #[allow(non_upper_case_globals)]
    pub const UnknownMethod: Self = Self::Unknown;
}

// Keep the names used by the task's method-kind examples available when the
// module is imported directly by a focused test.
pub use AuthMethodKind::{ApiKeyKind, BearerTokenKind, OAuth2Kind, Unknown};
pub use AuthProvenance::{ApiKey, ImportedLocal, Keyring, OfficialOAuth};

/// Compatibility alias for callers that use the shorter method-kind name.
pub type MethodKind = AuthMethodKind;

/// Compatibility alias for callers that use the generic auth-kind name.
pub type AuthKind = AuthMethodKind;

impl fmt::Display for AuthMethodKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str((*self).as_str())
    }
}

/// Provider authentication identity without credential material.
#[derive(Clone, Eq, PartialEq, Serialize)]
pub struct AuthProfile {
    /// Provider identifier.
    pub provider_id: String,
    /// Explicit credential provenance.
    pub provenance: AuthProvenance,
    /// Non-secret authentication method shape.
    pub method: AuthMethodKind,
}

impl fmt::Debug for AuthProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthProfile")
            .field("provider_id", &self.provider_id)
            .field("provenance", &self.provenance.to_string())
            .field("auth_mode", &self.method.as_str())
            .finish()
    }
}

/// Redacted provider authentication status.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuthDescription {
    /// Provider identifier.
    pub provider_id: String,
    /// Explicit credential provenance.
    pub provenance: AuthProvenance,
    /// Stable non-secret authentication mode label.
    pub auth_mode: &'static str,
    /// Whether the profile represents an available credential shape.
    pub has_credentials: bool,
}

impl AuthDescription {
    /// Whether this description represents a recognized credential shape.
    #[must_use]
    pub const fn has_credentials(&self) -> bool {
        self.has_credentials
    }
}

/// Validation failures when constructing an [`AuthProfile`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error, Serialize)]
pub enum AuthProfileError {
    /// Provider identifier was empty.
    #[error("provider id is empty")]
    EmptyProvider,
    /// Provider identifier exceeded [`MAX_PROVIDER_ID_LEN`].
    #[error("provider id is too long")]
    TooLong,
    /// Method kind was not recognized.
    #[error("unknown authentication method")]
    UnknownMethod,
}

/// Construct a secret-free provider authentication profile.
///
/// `method_kind` is an enum rather than a string or token-bearing type, so raw
/// credential bytes cannot be passed to this constructor. Validation happens
/// before allocation, leaving no partial profile on failure.
pub fn profile_of(
    provider_id: &str,
    provenance: AuthProvenance,
    method_kind: AuthMethodKind,
) -> Result<AuthProfile, AuthProfileError> {
    if provider_id.is_empty() {
        return Err(AuthProfileError::EmptyProvider);
    }
    if provider_id.len() > MAX_PROVIDER_ID_LEN {
        return Err(AuthProfileError::TooLong);
    }
    if !method_kind.is_known() {
        return Err(AuthProfileError::UnknownMethod);
    }

    Ok(AuthProfile {
        provider_id: provider_id.to_owned(),
        provenance,
        method: method_kind,
    })
}

/// Construct a profile using a method kind already classified by a caller.
/// This alias keeps the constructor discoverable for status-oriented callers.
pub fn new_profile(
    provider_id: &str,
    provenance: AuthProvenance,
    method_kind: AuthMethodKind,
) -> Result<AuthProfile, AuthProfileError> {
    profile_of(provider_id, provenance, method_kind)
}

/// Project a profile into a deterministic status-safe description.
#[must_use]
pub fn describe(profile: &AuthProfile) -> AuthDescription {
    AuthDescription {
        provider_id: profile.provider_id.clone(),
        provenance: profile.provenance,
        auth_mode: profile.method.as_str(),
        has_credentials: true,
    }
}

/// Render a profile for diagnostics without exposing credential material.
#[must_use]
pub fn redacted_debug(profile: &AuthProfile) -> String {
    format!(
        "AuthProfile {{ provider_id: {:?}, provenance: {}, auth_mode: {} }}",
        profile.provider_id, profile.provenance, profile.method
    )
}

impl fmt::Display for AuthProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&redacted_debug(self))
    }
}

impl fmt::Display for AuthDescription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AuthDescription {{ provider_id: {:?}, provenance: {}, auth_mode: {}, has_credentials: {} }}",
            self.provider_id, self.provenance, self.auth_mode, self.has_credentials
        )
    }
}
