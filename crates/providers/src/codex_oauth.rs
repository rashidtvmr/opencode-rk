//! Secret-free OpenAI Codex OAuth lifecycle planner.
//!
//! This is deliberately the native boundary only. It does not open a browser,
//! make an HTTP request, read a credential file, persist credentials, or hold
//! token bytes. A trusted caller owns the provider credential store and passes
//! an explicit, caller-issued grant after human consent.

use std::fmt;

use serde::Serialize;

/// Stable provider route selected by this connector.
pub const CODEX_PROVIDER: &str = "openai-codex";
/// Stable authentication mode shown in status.
pub const CODEX_AUTH_MODE: &str = "official-oauth";
/// Maximum consent URL length in bytes.
pub const MAX_CONSENT_URL_BYTES: usize = 2_048;
/// Maximum device-code length in bytes.
pub const MAX_DEVICE_CODE_BYTES: usize = 128;
/// Fixed public endpoint used by the offline login planner.
pub const CODEX_CONSENT_URL: &str = "https://auth.openai.com/authorize";
/// Fixed deterministic device-code label. It is not a credential.
pub const CODEX_DEVICE_CODE: &str = "codex-device-v1";

/// Explicit lifecycle of the Codex OAuth connector.
///
/// Credential material is intentionally absent from every variant. The caller
/// keeps access and refresh credentials in its own protected store.
#[derive(Clone, Eq, PartialEq, Serialize)]
pub enum CodexAuthState {
    /// No active Codex authorization exists.
    #[serde(rename = "logged_out")]
    LoggedOut,
    /// Human browser/device consent is still required.
    #[serde(rename = "pending_consent")]
    PendingConsent {
        /// HTTPS authorization endpoint, never a token-bearing URL.
        consent_url: String,
        /// Public device-flow correlation code.
        device_code: String,
    },
    /// A caller-owned credential is usable until this supplied timestamp.
    #[serde(rename = "ready")]
    Ready { expires_at_ms: u64 },
    /// The previous credential expired and needs an explicit refresh.
    #[serde(rename = "expired")]
    Expired,
}

impl fmt::Debug for CodexAuthState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LoggedOut => f.write_str("CodexAuthState::LoggedOut"),
            Self::PendingConsent { .. } => f
                .debug_struct("CodexAuthState::PendingConsent")
                .field("consent_url", &"https://***")
                .field("device_code", &"device-***")
                .finish(),
            Self::Ready { expires_at_ms } => f
                .debug_struct("CodexAuthState::Ready")
                .field("expires_at_ms", expires_at_ms)
                .finish(),
            Self::Expired => f.write_str("CodexAuthState::Expired"),
        }
    }
}

impl fmt::Display for CodexAuthState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::LoggedOut => "logged-out",
            Self::PendingConsent { .. } => "pending-consent",
            Self::Ready { .. } => "ready",
            Self::Expired => "expired",
        })
    }
}

/// Human approval material returned by a trusted consent boundary.
///
/// This value contains only a device-flow correlation code and expiry. It does
/// not contain access or refresh token bytes. `approved` is private so callers
/// cannot mutate an issued grant into another state; a rejected test fixture is
/// available through [`HumanGrant::rejected`].
#[derive(Clone, Eq, PartialEq)]
pub struct HumanGrant {
    device_code: String,
    expires_at_ms: u64,
    approved: bool,
}

impl HumanGrant {
    /// Issue an explicit grant for the deterministic device-flow attempt.
    ///
    /// The caller must invoke this only after its human-consent boundary has
    /// returned. The module cannot authenticate a human or perform OAuth.
    pub fn new(expires_at_ms: u64) -> Self {
        Self {
            device_code: CODEX_DEVICE_CODE.to_owned(),
            expires_at_ms,
            approved: true,
        }
    }

    /// Build an explicit grant from a caller-owned device code and approval.
    pub fn from_device_code(
        device_code: &str,
        expires_at_ms: u64,
        approved: bool,
    ) -> Result<Self, CodexError> {
        validate_device_code(device_code)?;
        Ok(Self {
            device_code: device_code.to_owned(),
            expires_at_ms,
            approved,
        })
    }

    /// Compatibility spelling for an explicit human approval.
    #[must_use]
    pub fn approved(expires_at_ms: u64) -> Self {
        Self::new(expires_at_ms)
    }

    /// Issue a grant tied to a caller-supplied pending device code.
    pub fn for_device(device_code: &str, expires_at_ms: u64) -> Result<Self, CodexError> {
        validate_device_code(device_code)?;
        Ok(Self {
            device_code: device_code.to_owned(),
            expires_at_ms,
            approved: true,
        })
    }

    /// Compatibility spelling for a fake grant issuer in isolated tests.
    #[must_use]
    pub fn for_test(expires_at_ms: u64) -> Self {
        Self::new(expires_at_ms)
    }

    /// Construct a deliberately rejected grant for negative-path tests.
    #[must_use]
    pub fn rejected(expires_at_ms: u64) -> Self {
        Self {
            device_code: CODEX_DEVICE_CODE.to_owned(),
            expires_at_ms,
            approved: false,
        }
    }

    /// Expiry supplied by the caller-owned credential authority.
    #[must_use]
    pub const fn expires_at_ms(&self) -> u64 {
        self.expires_at_ms
    }
}

impl fmt::Debug for HumanGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HumanGrant")
            .field("device_code", &"device-***")
            .field("expires_at_ms", &self.expires_at_ms)
            .field("approved", &self.approved)
            .finish()
    }
}

impl fmt::Display for HumanGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HumanGrant { device_code: device-*** }")
    }
}

/// Caller-issued grant for one bounded refresh operation.
///
/// As with [`HumanGrant`], this is only an approval and expiry marker. Refresh
/// token bytes remain in the caller-owned credential store.
#[derive(Clone, Eq, PartialEq)]
pub struct RefreshGrant {
    expires_at_ms: u64,
    approved: bool,
}

impl RefreshGrant {
    /// Issue an explicit refresh grant.
    #[must_use]
    pub const fn new(expires_at_ms: u64) -> Self {
        Self {
            expires_at_ms,
            approved: true,
        }
    }

    /// Build an explicit refresh grant from a caller-owned approval value.
    #[must_use]
    pub const fn from_approval(expires_at_ms: u64, approved: bool) -> Self {
        Self {
            expires_at_ms,
            approved,
        }
    }

    /// Compatibility spelling for an explicit refresh approval.
    #[must_use]
    pub const fn approved(expires_at_ms: u64) -> Self {
        Self::new(expires_at_ms)
    }

    /// Compatibility spelling for a fake refresh issuer in isolated tests.
    #[must_use]
    pub const fn for_test(expires_at_ms: u64) -> Self {
        Self::new(expires_at_ms)
    }

    /// Construct a deliberately rejected refresh grant for negative-path tests.
    #[must_use]
    pub const fn rejected(expires_at_ms: u64) -> Self {
        Self {
            expires_at_ms,
            approved: false,
        }
    }

    /// Expiry supplied by the caller-owned credential authority.
    #[must_use]
    pub const fn expires_at_ms(&self) -> u64 {
        self.expires_at_ms
    }
}

impl fmt::Debug for RefreshGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RefreshGrant")
            .field("expires_at_ms", &self.expires_at_ms)
            .field("approved", &self.approved)
            .finish()
    }
}

impl fmt::Display for RefreshGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RefreshGrant { token: refresh-*** }")
    }
}

/// Redacted, serializable status view.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CodexStatus {
    /// Always [`CODEX_PROVIDER`].
    pub provider: &'static str,
    /// Always [`CODEX_AUTH_MODE`].
    pub auth_mode: &'static str,
    /// Expiry when the state is ready; absent for all other states.
    pub expires_at_ms: Option<u64>,
    /// Stable non-secret error code, if the state needs attention.
    pub error: Option<&'static str>,
}

/// Fixed error codes. No variant carries caller input, token bytes, or URLs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum CodexError {
    /// Completion requires an explicit human grant.
    ConsentRequired,
    /// A consent endpoint was not HTTPS or exceeded its bound.
    BadConsentUrl,
    /// A device-flow correlation code was empty or too large.
    BadDeviceCode,
    /// The supplied grant was not issued by the consent boundary.
    InvalidGrant,
    /// The grant belongs to a different pending device-flow attempt.
    GrantMismatch,
    /// The caller attempted to use a logged-out connector.
    NotLoggedIn,
    /// The supplied expiry is not usable.
    InvalidExpiry,
    /// The current state does not accept the requested operation.
    InvalidState,
}

impl fmt::Display for CodexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = match self {
            Self::ConsentRequired => "consent required",
            Self::BadConsentUrl => "bad consent url",
            Self::BadDeviceCode => "bad device code",
            Self::InvalidGrant => "invalid grant",
            Self::GrantMismatch => "grant mismatch",
            Self::NotLoggedIn => "not logged in",
            Self::InvalidExpiry => "invalid expiry",
            Self::InvalidState => "invalid auth state",
        };
        f.write_str(code)
    }
}

impl std::error::Error for CodexError {}

/// Begin the deterministic, caller-driven device consent flow.
///
/// No browser, socket, clock, token, or file is touched.
#[must_use]
pub fn begin_login() -> CodexAuthState {
    // Constants are checked by the same validator used by the configurable
    // planner. Keeping this path infallible gives callers a direct state value.
    begin_login_with(CODEX_CONSENT_URL, CODEX_DEVICE_CODE)
        .expect("built-in Codex consent metadata is valid")
}

/// Build a bounded consent state from caller-supplied public metadata.
pub fn begin_login_with(
    consent_url: &str,
    device_code: &str,
) -> Result<CodexAuthState, CodexError> {
    validate_consent_url(consent_url)?;
    validate_device_code(device_code)?;
    Ok(CodexAuthState::PendingConsent {
        consent_url: consent_url.to_owned(),
        device_code: device_code.to_owned(),
    })
}

/// Complete pending consent and return a new state.
///
/// `None` is intentionally accepted so the missing-consent path is explicit
/// and leaves the input state unchanged. The state contains no credentials.
pub fn complete_login<G>(state: &CodexAuthState, grant: G) -> Result<CodexAuthState, CodexError>
where
    G: IntoHumanGrant,
{
    let Some(grant) = grant.into_human_grant() else {
        return Err(CodexError::ConsentRequired);
    };
    let CodexAuthState::PendingConsent { device_code, .. } = state else {
        return match state {
            CodexAuthState::LoggedOut => Err(CodexError::NotLoggedIn),
            CodexAuthState::Ready { .. } | CodexAuthState::Expired => Err(CodexError::InvalidState),
            CodexAuthState::PendingConsent { .. } => unreachable!("pending state matched above"),
        };
    };
    if !grant.approved {
        return Err(CodexError::InvalidGrant);
    }
    if grant.device_code != *device_code {
        return Err(CodexError::GrantMismatch);
    }
    if grant.expires_at_ms == 0 {
        return Err(CodexError::InvalidExpiry);
    }
    Ok(CodexAuthState::Ready {
        expires_at_ms: grant.expires_at_ms,
    })
}

/// Complete consent while applying a caller-supplied time boundary.
pub fn complete_login_at<G>(
    state: &CodexAuthState,
    grant: G,
    now_ms: u64,
) -> Result<CodexAuthState, CodexError>
where
    G: IntoHumanGrant,
{
    let next = complete_login(state, grant)?;
    match next {
        CodexAuthState::Ready { expires_at_ms } if expires_at_ms <= now_ms => {
            Err(CodexError::InvalidExpiry)
        }
        _ => Ok(next),
    }
}

/// Apply one explicit refresh grant and return a new state.
pub fn refresh<G>(state: &CodexAuthState, grant: G) -> Result<CodexAuthState, CodexError>
where
    G: IntoRefreshGrant,
{
    let Some(grant) = grant.into_refresh_grant() else {
        return Err(CodexError::InvalidGrant);
    };
    match state {
        CodexAuthState::LoggedOut => Err(CodexError::NotLoggedIn),
        CodexAuthState::PendingConsent { .. } => Err(CodexError::ConsentRequired),
        CodexAuthState::Ready { .. } | CodexAuthState::Expired => {
            if !grant.approved {
                return Err(CodexError::InvalidGrant);
            }
            if grant.expires_at_ms == 0 {
                return Err(CodexError::InvalidExpiry);
            }
            Ok(CodexAuthState::Ready {
                expires_at_ms: grant.expires_at_ms,
            })
        }
    }
}

/// Apply one refresh grant with an explicit caller-supplied time boundary.
pub fn refresh_at<G>(
    state: &CodexAuthState,
    grant: G,
    now_ms: u64,
) -> Result<CodexAuthState, CodexError>
where
    G: IntoRefreshGrant,
{
    let next = refresh(state, grant)?;
    match next {
        CodexAuthState::Ready { expires_at_ms } if expires_at_ms <= now_ms => {
            Err(CodexError::InvalidExpiry)
        }
        _ => Ok(next),
    }
}

/// Return a logged-out state. No token material is accepted or retained.
#[must_use]
pub fn logout(_: &CodexAuthState) -> CodexAuthState {
    CodexAuthState::LoggedOut
}

/// Project a state into a deterministic redacted status view.
#[must_use]
pub fn status(state: &CodexAuthState) -> CodexStatus {
    let (expires_at_ms, error) = match state {
        CodexAuthState::LoggedOut => (None, None),
        CodexAuthState::PendingConsent { .. } => (None, Some("consent-required")),
        CodexAuthState::Ready { expires_at_ms } => (Some(*expires_at_ms), None),
        CodexAuthState::Expired => (None, Some("expired")),
    };
    CodexStatus {
        provider: CODEX_PROVIDER,
        auth_mode: CODEX_AUTH_MODE,
        expires_at_ms,
        error,
    }
}

/// Project status using a caller-supplied timestamp, without mutating state.
#[must_use]
pub fn status_at(state: &CodexAuthState, now_ms: u64) -> CodexStatus {
    match state {
        CodexAuthState::Ready { expires_at_ms } if *expires_at_ms <= now_ms => {
            status(&CodexAuthState::Expired)
        }
        _ => status(state),
    }
}

/// Stable native route target. No CLI impersonation or undocumented endpoint.
#[must_use]
pub const fn route_target() -> &'static str {
    CODEX_PROVIDER
}

/// Convert an explicit grant or an absent grant into the completion input.
pub trait IntoHumanGrant {
    fn into_human_grant(self) -> Option<HumanGrant>;
}

impl IntoHumanGrant for HumanGrant {
    fn into_human_grant(self) -> Option<HumanGrant> {
        Some(self)
    }
}

impl IntoHumanGrant for &HumanGrant {
    fn into_human_grant(self) -> Option<HumanGrant> {
        Some(self.clone())
    }
}

impl IntoHumanGrant for Option<HumanGrant> {
    fn into_human_grant(self) -> Option<HumanGrant> {
        self
    }
}

/// Convert an explicit refresh grant into the refresh input.
pub trait IntoRefreshGrant {
    fn into_refresh_grant(self) -> Option<RefreshGrant>;
}

impl IntoRefreshGrant for RefreshGrant {
    fn into_refresh_grant(self) -> Option<RefreshGrant> {
        Some(self)
    }
}

impl IntoRefreshGrant for &RefreshGrant {
    fn into_refresh_grant(self) -> Option<RefreshGrant> {
        Some(self.clone())
    }
}

fn validate_consent_url(url: &str) -> Result<(), CodexError> {
    if url.len() > MAX_CONSENT_URL_BYTES || !url.starts_with("https://") {
        return Err(CodexError::BadConsentUrl);
    }
    Ok(())
}

fn validate_device_code(device_code: &str) -> Result<(), CodexError> {
    if device_code.is_empty() || device_code.len() > MAX_DEVICE_CODE_BYTES {
        return Err(CodexError::BadDeviceCode);
    }
    Ok(())
}

impl CodexAuthState {
    /// Mutating method form of [`complete_login`].
    pub fn complete_login<G>(&mut self, grant: G) -> Result<CodexAuthState, CodexError>
    where
        G: IntoHumanGrant,
    {
        let next = complete_login(self, grant)?;
        *self = next.clone();
        Ok(next)
    }

    /// Mutating method form of [`refresh`].
    pub fn refresh<G>(&mut self, grant: G) -> Result<CodexAuthState, CodexError>
    where
        G: IntoRefreshGrant,
    {
        let next = refresh(self, grant)?;
        *self = next.clone();
        Ok(next)
    }

    /// Mutating logout operation.
    pub fn logout(&mut self) -> CodexAuthState {
        *self = CodexAuthState::LoggedOut;
        self.clone()
    }
}
