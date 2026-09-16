//! Pure Claude Code OAuth consent state.
//!
//! This module deliberately stops at the provider-neutral boundary. It does
//! not open a browser, bind a listener, perform HTTP, read credentials, or
//! retain credential material. A caller owns those operations and supplies a
//! validated human grant back to this state machine.

#![forbid(unsafe_code)]

use std::borrow::Borrow;
use std::fmt;

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use thiserror::Error;

/// Official native provider route. This is an identifier, not a CLI alias.
pub const CLAUDE_ROUTE_TARGET: &str = "anthropic-claude-code";
/// Compatibility alias for callers naming the provider rather than route.
pub const CLAUDE_PROVIDER: &str = CLAUDE_ROUTE_TARGET;
/// Stable auth mode exposed by [`ClaudeStatus`].
pub const CLAUDE_AUTH_MODE: &str = "official-oauth";
/// Maximum consent URL size in bytes.
pub const MAX_CONSENT_URL_BYTES: usize = 2_048;
/// Minimum RFC 7636 base64url PKCE challenge size.
pub const MIN_PKCE_CHALLENGE_BYTES: usize = 43;
/// Maximum PKCE challenge size retained by a pending state.
pub const MAX_PKCE_CHALLENGE_BYTES: usize = 128;
/// Maximum opaque credential size inspected from a caller-owned grant.
pub const MAX_GRANT_TOKEN_BYTES: usize = 4_096;
/// Fixed loopback redirect used by the caller's eventual OAuth adapter.
pub const LOOPBACK_REDIRECT_URI: &str = "http://127.0.0.1:1455/oauth/callback";

const CONSENT_ORIGIN: &str = "https://claude.ai/oauth/authorize";

/// Redacted lifecycle state. Token material is intentionally absent from every
/// variant. The caller may retain tokens in a separate protected store.
#[derive(Clone, Eq, PartialEq)]
pub enum ClaudeAuthState {
    /// No active Claude OAuth session.
    LoggedOut,
    /// Browser consent may be presented by the caller.
    PendingConsent {
        /// HTTPS provider URL containing the loopback redirect and challenge.
        consent_url: String,
        /// Public PKCE challenge. The verifier is never retained here.
        pkce_challenge: String,
    },
    /// A caller-owned credential is currently usable until this timestamp.
    Ready { expires_at_ms: u64 },
    /// The caller observed expiry and must refresh or consent again.
    Expired,
}

impl fmt::Debug for ClaudeAuthState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LoggedOut => f.write_str("ClaudeAuthState::LoggedOut"),
            Self::PendingConsent {
                consent_url,
                pkce_challenge,
            } => f
                .debug_struct("ClaudeAuthState::PendingConsent")
                .field("consent_url", consent_url)
                .field("pkce_challenge", pkce_challenge)
                .finish(),
            Self::Ready { expires_at_ms } => f
                .debug_struct("ClaudeAuthState::Ready")
                .field("expires_at_ms", expires_at_ms)
                .finish(),
            Self::Expired => f.write_str("ClaudeAuthState::Expired"),
        }
    }
}

impl Serialize for ClaudeAuthState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut out = serializer.serialize_struct("ClaudeAuthState", 3)?;
        match self {
            Self::LoggedOut => {
                out.serialize_field("state", "logged_out")?;
            }
            Self::PendingConsent {
                consent_url,
                pkce_challenge,
            } => {
                out.serialize_field("state", "pending_consent")?;
                out.serialize_field("consent_url", consent_url)?;
                out.serialize_field("pkce_challenge", pkce_challenge)?;
            }
            Self::Ready { expires_at_ms } => {
                out.serialize_field("state", "ready")?;
                out.serialize_field("expires_at_ms", expires_at_ms)?;
            }
            Self::Expired => {
                out.serialize_field("state", "expired")?;
            }
        }
        out.end()
    }
}

/// Explicit human-authorized OAuth result.
///
/// The secret fields are private and never rendered or serialized. `new` is
/// the boundary a trusted UI/caller invokes only after displaying consent;
/// [`Self::untrusted`] exists for negative tests and adapter rejection paths.
#[derive(Clone)]
pub struct HumanGrant {
    access_token: String,
    refresh_token: String,
    expires_at_ms: u64,
    human_consent: bool,
    pkce_challenge: Option<String>,
}

impl HumanGrant {
    /// Construct a grant asserted by the caller to follow human consent.
    ///
    /// This constructor performs no network or browser action. It only wraps
    /// caller-owned bytes for one transition; transitions never retain them.
    #[must_use]
    pub fn new(
        access_token: impl Into<String>,
        refresh_token: impl Into<String>,
        expires_at_ms: u64,
    ) -> Self {
        Self {
            access_token: access_token.into(),
            refresh_token: refresh_token.into(),
            expires_at_ms,
            human_consent: true,
            pkce_challenge: None,
        }
    }

    /// Construct a grant explicitly bound to the pending PKCE challenge.
    pub fn for_pkce(
        access_token: impl Into<String>,
        refresh_token: impl Into<String>,
        expires_at_ms: u64,
        pkce_challenge: &str,
    ) -> Result<Self, ClaudeError> {
        validate_pkce(pkce_challenge)?;
        Ok(Self {
            access_token: access_token.into(),
            refresh_token: refresh_token.into(),
            expires_at_ms,
            human_consent: true,
            pkce_challenge: Some(pkce_challenge.to_owned()),
        })
    }

    /// Construct a grant that must be rejected as lacking human authority.
    /// Intended for deterministic adapter negative tests, not production use.
    #[must_use]
    pub fn untrusted(
        access_token: impl Into<String>,
        refresh_token: impl Into<String>,
        expires_at_ms: u64,
    ) -> Self {
        Self {
            access_token: access_token.into(),
            refresh_token: refresh_token.into(),
            expires_at_ms,
            human_consent: false,
            pkce_challenge: None,
        }
    }

    /// Construct an explicit human grant from a caller-supplied OAuth result.
    #[must_use]
    pub fn from_human_consent(
        access_token: impl Into<String>,
        refresh_token: impl Into<String>,
        expires_at_ms: u64,
    ) -> Self {
        Self::new(access_token, refresh_token, expires_at_ms)
    }

    /// Compatibility spelling for an explicit human approval.
    #[must_use]
    pub fn approved(
        access_token: impl Into<String>,
        refresh_token: impl Into<String>,
        expires_at_ms: u64,
    ) -> Self {
        Self::new(access_token, refresh_token, expires_at_ms)
    }

    /// Compatibility spelling for isolated fake-grant tests.
    #[must_use]
    pub fn for_test(
        access_token: impl Into<String>,
        refresh_token: impl Into<String>,
        expires_at_ms: u64,
    ) -> Self {
        Self::new(access_token, refresh_token, expires_at_ms)
    }

    /// Construct a deliberately rejected grant for negative-path tests.
    #[must_use]
    pub fn rejected(
        access_token: impl Into<String>,
        refresh_token: impl Into<String>,
        expires_at_ms: u64,
    ) -> Self {
        Self::untrusted(access_token, refresh_token, expires_at_ms)
    }

    /// Expiry supplied by the caller-owned credential authority.
    #[must_use]
    pub const fn expires_at_ms(&self) -> u64 {
        self.expires_at_ms
    }

    fn valid_for(&self, pending_pkce: &str) -> Result<(), ClaudeError> {
        if !self.human_consent {
            return Err(ClaudeError::InvalidGrant);
        }
        validate_token(&self.access_token)?;
        validate_token(&self.refresh_token)?;
        if self.expires_at_ms == 0 {
            return Err(ClaudeError::InvalidExpiry);
        }
        if let Some(bound_pkce) = &self.pkce_challenge {
            if bound_pkce != pending_pkce {
                return Err(ClaudeError::InvalidGrant);
            }
        }
        Ok(())
    }
}

impl fmt::Debug for HumanGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HumanGrant")
            .field("access_secret", &"[REDACTED]")
            .field("refresh_secret", &"[REDACTED]")
            .field("expires_at_ms", &self.expires_at_ms)
            .field("human_consent", &self.human_consent)
            .field("pkce_bound", &self.pkce_challenge.is_some())
            .finish()
    }
}

impl Serialize for HumanGrant {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut out = serializer.serialize_struct("HumanGrant", 3)?;
        out.serialize_field("kind", "human_grant")?;
        out.serialize_field("expires_at_ms", &self.expires_at_ms)?;
        out.serialize_field("human_consent", &self.human_consent)?;
        out.end()
    }
}

/// Caller-supplied bounded refresh result. Secret material is optional because
/// the native state machine only records its expiry; the caller owns tokens.
#[derive(Clone)]
pub struct RefreshGrant {
    expires_at_ms: u64,
    refresh_token: Option<String>,
    human_consent: bool,
}

impl RefreshGrant {
    /// Construct a refresh result with a caller-owned token store.
    #[must_use]
    pub fn new(expires_at_ms: u64) -> Self {
        Self {
            expires_at_ms,
            refresh_token: None,
            human_consent: true,
        }
    }

    /// Construct a refresh result carrying an opaque token only for validation.
    #[must_use]
    pub fn with_token(refresh_token: impl Into<String>, expires_at_ms: u64) -> Self {
        Self {
            expires_at_ms,
            refresh_token: Some(refresh_token.into()),
            human_consent: true,
        }
    }

    /// Construct a refresh result lacking trusted caller authority.
    #[must_use]
    pub fn untrusted(expires_at_ms: u64) -> Self {
        Self {
            expires_at_ms,
            refresh_token: None,
            human_consent: false,
        }
    }

    /// Compatibility spelling for an explicit refresh approval.
    #[must_use]
    pub const fn approved(expires_at_ms: u64) -> Self {
        Self {
            expires_at_ms,
            refresh_token: None,
            human_consent: true,
        }
    }

    /// Compatibility spelling for isolated fake-refresh tests.
    #[must_use]
    pub const fn for_test(expires_at_ms: u64) -> Self {
        Self {
            expires_at_ms,
            refresh_token: None,
            human_consent: true,
        }
    }

    /// Construct a deliberately rejected refresh grant.
    #[must_use]
    pub const fn rejected(expires_at_ms: u64) -> Self {
        Self {
            expires_at_ms,
            refresh_token: None,
            human_consent: false,
        }
    }

    /// Expiry supplied by the caller-owned credential authority.
    #[must_use]
    pub const fn expires_at_ms(&self) -> u64 {
        self.expires_at_ms
    }

    fn validate(&self) -> Result<(), ClaudeError> {
        if !self.human_consent {
            return Err(ClaudeError::InvalidGrant);
        }
        if let Some(token) = &self.refresh_token {
            validate_token(token)?;
        }
        if self.expires_at_ms == 0 {
            return Err(ClaudeError::InvalidExpiry);
        }
        Ok(())
    }
}

impl fmt::Debug for RefreshGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RefreshGrant")
            .field("refresh_secret", &"[REDACTED]")
            .field("has_refresh_secret", &self.refresh_token.is_some())
            .field("expires_at_ms", &self.expires_at_ms)
            .field("human_consent", &self.human_consent)
            .finish()
    }
}

impl Serialize for RefreshGrant {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut out = serializer.serialize_struct("RefreshGrant", 3)?;
        out.serialize_field("kind", "refresh_grant")?;
        out.serialize_field("expires_at_ms", &self.expires_at_ms)?;
        out.serialize_field("human_consent", &self.human_consent)?;
        out.end()
    }
}

/// Stable redacted auth status. It contains no token, URL, PKCE verifier, or
/// provider response text.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ClaudeStatus {
    pub provider: &'static str,
    pub auth_mode: &'static str,
    pub expires_at_ms: Option<u64>,
    pub error: Option<&'static str>,
}

/// Pure transition and validation failures. Display text carries no input.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error, Serialize)]
pub enum ClaudeError {
    #[error("pkce challenge is invalid")]
    BadPkce,
    #[error("consent is required")]
    ConsentRequired,
    #[error("consent URL is invalid")]
    BadConsentUrl,
    #[error("not logged in")]
    NotLoggedIn,
    #[error("invalid grant")]
    InvalidGrant,
    #[error("grant does not match pending consent")]
    GrantMismatch,
    #[error("invalid expiry")]
    InvalidExpiry,
    #[error("invalid auth state")]
    InvalidState,
    #[error("credential material is invalid")]
    InvalidCredential,
}

/// Trait allowing a transition to accept a grant, `Some(grant)`, or a
/// borrowed equivalent without making absence implicit.
pub trait HumanGrantInput {
    fn human_grant(&self) -> Option<&HumanGrant>;
}

impl HumanGrantInput for HumanGrant {
    fn human_grant(&self) -> Option<&HumanGrant> {
        Some(self)
    }
}

impl HumanGrantInput for Option<HumanGrant> {
    fn human_grant(&self) -> Option<&HumanGrant> {
        self.as_ref()
    }
}

impl HumanGrantInput for &HumanGrant {
    fn human_grant(&self) -> Option<&HumanGrant> {
        Some(self)
    }
}

/// Trait equivalent for refresh grants and optional refresh grants.
pub trait RefreshGrantInput {
    fn refresh_grant(&self) -> Option<&RefreshGrant>;
}

impl RefreshGrantInput for RefreshGrant {
    fn refresh_grant(&self) -> Option<&RefreshGrant> {
        Some(self)
    }
}

impl RefreshGrantInput for Option<RefreshGrant> {
    fn refresh_grant(&self) -> Option<&RefreshGrant> {
        self.as_ref()
    }
}

impl RefreshGrantInput for &RefreshGrant {
    fn refresh_grant(&self) -> Option<&RefreshGrant> {
        Some(self)
    }
}

/// Build the official HTTPS consent URL and enter pending consent.
pub fn begin_login(pkce_challenge: impl AsRef<str>) -> Result<ClaudeAuthState, ClaudeError> {
    let pkce_challenge = pkce_challenge.as_ref();
    validate_pkce(pkce_challenge)?;
    let consent_url = format!(
        "{CONSENT_ORIGIN}?redirect_uri={LOOPBACK_REDIRECT_URI}&code_challenge={pkce_challenge}&code_challenge_method=S256"
    );
    begin_login_with_url(pkce_challenge, consent_url)
}

/// Validate a caller-provided HTTPS consent URL while preserving the same
/// loopback and byte bounds as [`begin_login`].
pub fn begin_login_with_url(
    pkce_challenge: impl AsRef<str>,
    consent_url: impl AsRef<str>,
) -> Result<ClaudeAuthState, ClaudeError> {
    let pkce_challenge = pkce_challenge.as_ref();
    validate_pkce(pkce_challenge)?;
    let consent_url = consent_url.as_ref();
    validate_consent_url(consent_url)?;
    Ok(ClaudeAuthState::PendingConsent {
        consent_url: consent_url.to_owned(),
        pkce_challenge: pkce_challenge.to_owned(),
    })
}

/// Complete pending consent with an explicit human grant. Input state remains
/// unchanged on every error; returned ready state retains only expiry metadata.
pub fn complete_login<S, G>(state: S, grant: G) -> Result<ClaudeAuthState, ClaudeError>
where
    S: Borrow<ClaudeAuthState>,
    G: HumanGrantInput,
{
    let grant = grant.human_grant().ok_or(ClaudeError::ConsentRequired)?;
    let ClaudeAuthState::PendingConsent {
        consent_url,
        pkce_challenge,
    } = state.borrow()
    else {
        return Err(ClaudeError::InvalidState);
    };
    validate_consent_url(consent_url)?;
    grant.valid_for(pkce_challenge)?;
    Ok(ClaudeAuthState::Ready {
        expires_at_ms: grant.expires_at_ms,
    })
}

/// Refresh a ready/expired state once, retaining expiry metadata only.
pub fn refresh<S, G>(state: S, grant: G) -> Result<ClaudeAuthState, ClaudeError>
where
    S: Borrow<ClaudeAuthState>,
    G: RefreshGrantInput,
{
    let current = state.borrow();
    if matches!(current, ClaudeAuthState::LoggedOut) {
        return Err(ClaudeError::NotLoggedIn);
    }
    let grant = grant.refresh_grant().ok_or(ClaudeError::ConsentRequired)?;
    grant.validate()?;
    if let ClaudeAuthState::Ready { expires_at_ms } = current {
        if grant.expires_at_ms <= *expires_at_ms {
            return Err(ClaudeError::InvalidExpiry);
        }
    } else if !matches!(current, ClaudeAuthState::Expired) {
        return Err(ClaudeError::InvalidState);
    }
    Ok(ClaudeAuthState::Ready {
        expires_at_ms: grant.expires_at_ms,
    })
}

/// Clear all auth references by returning the token-free logged-out state.
pub fn logout<S>(state: S) -> ClaudeAuthState
where
    S: Borrow<ClaudeAuthState>,
{
    let _ = state.borrow();
    ClaudeAuthState::LoggedOut
}

/// Project a deterministic status without reading a clock.
pub fn status<S>(state: S) -> ClaudeStatus
where
    S: Borrow<ClaudeAuthState>,
{
    match state.borrow() {
        ClaudeAuthState::LoggedOut => ClaudeStatus {
            provider: CLAUDE_ROUTE_TARGET,
            auth_mode: CLAUDE_AUTH_MODE,
            expires_at_ms: None,
            error: None,
        },
        ClaudeAuthState::PendingConsent { .. } => ClaudeStatus {
            provider: CLAUDE_ROUTE_TARGET,
            auth_mode: CLAUDE_AUTH_MODE,
            expires_at_ms: None,
            error: Some("consent-pending"),
        },
        ClaudeAuthState::Ready { expires_at_ms } => ClaudeStatus {
            provider: CLAUDE_ROUTE_TARGET,
            auth_mode: CLAUDE_AUTH_MODE,
            expires_at_ms: Some(*expires_at_ms),
            error: None,
        },
        ClaudeAuthState::Expired => ClaudeStatus {
            provider: CLAUDE_ROUTE_TARGET,
            auth_mode: CLAUDE_AUTH_MODE,
            expires_at_ms: None,
            error: Some("expired"),
        },
    }
}

/// Apply caller-supplied time to a ready state without reading a clock.
pub fn expire_at<S>(state: S, now_ms: u64) -> ClaudeAuthState
where
    S: Borrow<ClaudeAuthState>,
{
    match state.borrow() {
        ClaudeAuthState::Ready { expires_at_ms } if *expires_at_ms <= now_ms => {
            ClaudeAuthState::Expired
        }
        other => other.clone(),
    }
}

/// Status projection with caller-supplied expiry observation.
pub fn status_at<S>(state: S, now_ms: u64) -> ClaudeStatus
where
    S: Borrow<ClaudeAuthState>,
{
    status(expire_at(state, now_ms))
}

/// Return the native provider target. No CLI impersonation or endpoint choice
/// is performed here.
#[must_use]
pub const fn route_target() -> &'static str {
    CLAUDE_ROUTE_TARGET
}

/// Validate a consent URL at the boundary where a caller would retain it.
pub fn validate_consent_url(url: &str) -> Result<(), ClaudeError> {
    if url.is_empty()
        || url.len() > MAX_CONSENT_URL_BYTES
        || !url.starts_with("https://")
        || url
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte == b' ')
        || !contains_loopback_redirect(url)
    {
        return Err(ClaudeError::BadConsentUrl);
    }
    let authority = url["https://".len()..]
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        return Err(ClaudeError::BadConsentUrl);
    }
    Ok(())
}

fn contains_loopback_redirect(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    ["127.0.0.1", "localhost", "[::1]", "%5b%3a%3a1%5d"]
        .iter()
        .any(|marker| {
            let mut offset = 0;
            while let Some(index) = lower[offset..].find(marker) {
                let start = offset + index;
                let end = start + marker.len();
                let before = lower.as_bytes().get(start.wrapping_sub(1)).copied();
                let after = lower.as_bytes().get(end).copied();
                let before_ok = before.is_none_or(|byte| {
                    matches!(byte, b'/' | b'=' | b':' | b'%' | b'&' | b'?' | b'\n')
                });
                let after_ok = after
                    .is_none_or(|byte| matches!(byte, b'/' | b':' | b'%' | b'&' | b'?' | b'#'));
                if before_ok && after_ok {
                    return true;
                }
                offset = end;
                if offset >= lower.len() {
                    break;
                }
            }
            false
        })
}

fn validate_pkce(pkce_challenge: &str) -> Result<(), ClaudeError> {
    if !(MIN_PKCE_CHALLENGE_BYTES..=MAX_PKCE_CHALLENGE_BYTES).contains(&pkce_challenge.len())
        || !pkce_challenge
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(ClaudeError::BadPkce);
    }
    Ok(())
}

fn validate_token(token: &str) -> Result<(), ClaudeError> {
    if token.is_empty()
        || token.len() > MAX_GRANT_TOKEN_BYTES
        || token.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(ClaudeError::InvalidCredential);
    }
    Ok(())
}

impl ClaudeAuthState {
    /// Associated-function convenience wrapper for [`begin_login`].
    pub fn begin_login(pkce_challenge: impl AsRef<str>) -> Result<Self, ClaudeError> {
        begin_login(pkce_challenge)
    }

    /// Transition convenience wrapper for [`complete_login`].
    pub fn complete_login<G: HumanGrantInput>(&self, grant: G) -> Result<Self, ClaudeError> {
        complete_login(self, grant)
    }

    /// Mutating-style completion for callers holding the state in place.
    pub fn complete_login_mut<G: HumanGrantInput>(
        &mut self,
        grant: G,
    ) -> Result<Self, ClaudeError> {
        let next = complete_login(&mut *self, grant)?;
        *self = next.clone();
        Ok(next)
    }

    /// Transition convenience wrapper for [`refresh`].
    pub fn refresh<G: RefreshGrantInput>(&self, grant: G) -> Result<Self, ClaudeError> {
        refresh(self, grant)
    }

    /// Mutating-style refresh for callers holding the state in place.
    pub fn refresh_mut<G: RefreshGrantInput>(&mut self, grant: G) -> Result<Self, ClaudeError> {
        let next = refresh(&mut *self, grant)?;
        *self = next.clone();
        Ok(next)
    }

    /// Transition convenience wrapper for [`logout`].
    #[must_use]
    pub fn logout(&self) -> Self {
        logout(self)
    }

    /// Mutating logout operation.
    pub fn logout_mut(&mut self) -> Self {
        *self = Self::LoggedOut;
        self.clone()
    }

    /// Projection convenience wrapper for [`status`].
    #[must_use]
    pub fn status(&self) -> ClaudeStatus {
        status(self)
    }
}
