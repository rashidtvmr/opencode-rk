//! Pure, documentation-backed request profiles for provider API calls.
//!
//! This module owns request shape, not credentials.  A profile contains an
//! allowlisted provider endpoint and authentication *kinds* only.  Secret
//! values, arbitrary headers, network access, and client-impersonation fields
//! deliberately have no representation here.

#![forbid(unsafe_code)]

use serde::Serialize;
use thiserror::Error;

/// Maximum UTF-8 byte length of a provider endpoint.
pub const MAX_ENDPOINT_BYTES: usize = 2_048;
/// Maximum authentication header kinds in one profile.
pub const MAX_AUTH_HEADERS: usize = 8;
/// Smallest accepted request timeout.
pub const MIN_TIMEOUT_MS: u64 = 1_000;
/// Default request timeout.
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;
/// Largest accepted request timeout.
pub const MAX_TIMEOUT_MS: u64 = 120_000;
/// Default number of retries after the initial request.
pub const DEFAULT_MAX_RETRIES: u32 = 1;
/// Largest accepted number of retries after the initial request.
pub const MAX_RETRIES: u32 = 3;

/// Documented endpoint kinds.  There is intentionally no custom endpoint
/// variant: adding an endpoint requires a reviewed compatibility entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EndpointKind {
    /// OpenAI Chat Completions endpoint.
    ChatCompletions,
    /// Anthropic Messages endpoint.
    Messages,
}

/// Closed set of provider authentication header kinds.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthHeaderKind {
    /// `Authorization: Bearer ...`; the bearer value is supplied elsewhere.
    Bearer,
    /// Provider API-key header; the key value is supplied elsewhere.
    ApiKeyHeader,
}

/// A serialization-safe marker returned in place of a header value.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
pub enum RedactedMarker {
    /// Header value intentionally omitted.
    #[serde(rename = "[REDACTED]")]
    Redacted,
}

impl RedactedMarker {
    /// Convenient spelling for callers rendering redacted header maps.
    pub const REDACTED: Self = Self::Redacted;
}

/// Validated, secret-free request metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RequestProfile {
    /// Exact provider identity from the compatibility catalog.
    pub provider_id: String,
    /// Exact documented HTTPS endpoint.
    pub endpoint: String,
    /// Required documented authentication kinds, never values.
    pub auth_headers: Vec<AuthHeaderKind>,
    /// Bounded request timeout in milliseconds.
    pub timeout_ms: u64,
    /// Bounded retries after the initial attempt.
    pub max_retries: u32,
}

/// Caller-selected non-secret request limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct RequestProfileOptions {
    pub timeout_ms: u64,
    pub max_retries: u32,
}

impl Default for RequestProfileOptions {
    fn default() -> Self {
        Self {
            timeout_ms: DEFAULT_TIMEOUT_MS,
            max_retries: DEFAULT_MAX_RETRIES,
        }
    }
}

/// Failure codes deliberately carry no caller-provided endpoint, header, or
/// credential text.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Error)]
pub enum RequestError {
    #[error("unknown provider")]
    UnknownProvider,
    #[error("undocumented endpoint")]
    UndocumentedEndpoint,
    #[error("header not allowed")]
    HeaderNotAllowed,
    #[error("endpoint must be a valid https endpoint of at most {MAX_ENDPOINT_BYTES} bytes")]
    BadEndpoint,
    #[error(
        "request bounds invalid: timeout {MIN_TIMEOUT_MS}..={MAX_TIMEOUT_MS} ms, retries 0..={MAX_RETRIES}; defaults {DEFAULT_TIMEOUT_MS} ms/{DEFAULT_MAX_RETRIES}"
    )]
    BadBounds,
}

/// Secret-free wire diagnostic.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RedactedDiagnostic {
    pub provider_id: String,
    pub endpoint: String,
    pub attempt: u32,
    pub error_code: Option<String>,
}

/// Endpoint selector accepted by [`profile_for`].
#[derive(Clone, Debug)]
pub enum EndpointSelector<'a> {
    /// Typed endpoint kind from the closed compatibility set.
    Kind(EndpointKind),
    /// Documented endpoint-kind spelling or exact documented URL.
    Name(&'a str),
    /// Owned selector for callers that already have a bounded `String`.
    Owned(String),
}

impl<'a> From<EndpointKind> for EndpointSelector<'a> {
    fn from(kind: EndpointKind) -> Self {
        Self::Kind(kind)
    }
}

impl<'a> From<&'a str> for EndpointSelector<'a> {
    fn from(name: &'a str) -> Self {
        Self::Name(name)
    }
}

impl<'a> From<&'a String> for EndpointSelector<'a> {
    fn from(name: &'a String) -> Self {
        Self::Name(name.as_str())
    }
}

impl From<String> for EndpointSelector<'static> {
    fn from(name: String) -> Self {
        Self::Owned(name)
    }
}

impl<'a> From<&'a EndpointKind> for EndpointSelector<'a> {
    fn from(kind: &'a EndpointKind) -> Self {
        Self::Kind(*kind)
    }
}

/// Resolve a documented provider and endpoint using default request bounds.
pub fn profile_for<'a, K: Into<EndpointSelector<'a>>>(
    provider_id: &str,
    endpoint_kind: K,
) -> Result<RequestProfile, RequestError> {
    profile_for_with_options(provider_id, endpoint_kind, RequestProfileOptions::default())
}

/// Resolve a documented profile with caller-selected bounded timeout/retries.
pub fn profile_for_with_bounds<'a, K: Into<EndpointSelector<'a>>>(
    provider_id: &str,
    endpoint_kind: K,
    timeout_ms: u64,
    max_retries: u32,
) -> Result<RequestProfile, RequestError> {
    profile_for_with_options(
        provider_id,
        endpoint_kind,
        RequestProfileOptions {
            timeout_ms,
            max_retries,
        },
    )
}

/// Resolve a documented profile with explicit non-secret options.
pub fn profile_for_with_options<'a, K: Into<EndpointSelector<'a>>>(
    provider_id: &str,
    endpoint_kind: K,
    options: RequestProfileOptions,
) -> Result<RequestProfile, RequestError> {
    let input = endpoint_kind.into();
    let (endpoint, auth_headers) = documented_entry(provider_id, input)?;
    validate_endpoint(endpoint)?;
    validate_bounds(options)?;

    let profile = RequestProfile {
        provider_id: provider_id.to_owned(),
        endpoint: endpoint.to_owned(),
        auth_headers: auth_headers.to_vec(),
        timeout_ms: options.timeout_ms,
        max_retries: options.max_retries,
    };
    debug_assert!(profile.validate().is_ok());
    Ok(profile)
}

/// Alias for callers that use "limits" terminology.
pub fn profile_for_with_limits<'a, K: Into<EndpointSelector<'a>>>(
    provider_id: &str,
    endpoint_kind: K,
    timeout_ms: u64,
    max_retries: u32,
) -> Result<RequestProfile, RequestError> {
    profile_for_with_bounds(provider_id, endpoint_kind, timeout_ms, max_retries)
}

/// Validate an already-held profile before it reaches a transport boundary.
pub fn validate_profile(profile: &RequestProfile) -> Result<(), RequestError> {
    profile.validate()
}

impl RequestProfile {
    /// Validate identity, exact endpoint, closed header set, and resource
    /// bounds.  This is useful when a profile crossed an untrusted data
    /// boundary after construction.
    pub fn validate(&self) -> Result<(), RequestError> {
        let input = EndpointSelector::Name(self.endpoint.as_str());
        let (endpoint, auth_headers) = documented_entry(&self.provider_id, input)?;
        validate_endpoint(&self.endpoint)?;
        validate_bounds(RequestProfileOptions {
            timeout_ms: self.timeout_ms,
            max_retries: self.max_retries,
        })?;
        if endpoint != self.endpoint || self.auth_headers.as_slice() != auth_headers {
            return Err(RequestError::HeaderNotAllowed);
        }
        if self.auth_headers.len() > MAX_AUTH_HEADERS {
            return Err(RequestError::HeaderNotAllowed);
        }
        Ok(())
    }
}

/// Return header names paired with redaction markers, never header values.
/// Invalid or manually altered profiles produce no wire headers.
pub fn headers_for(profile: &RequestProfile) -> Vec<(String, RedactedMarker)> {
    if profile.validate().is_err() {
        return Vec::new();
    }

    profile
        .auth_headers
        .iter()
        .take(MAX_AUTH_HEADERS)
        .map(|kind| {
            let name = match kind {
                AuthHeaderKind::Bearer => "Authorization",
                AuthHeaderKind::ApiKeyHeader => "x-api-key",
            };
            (name.to_owned(), RedactedMarker::Redacted)
        })
        .collect()
}

/// Build a diagnostic containing provider identity and attempt number only.
pub fn with_diagnostics(profile: &RequestProfile, attempt: u32) -> RedactedDiagnostic {
    RedactedDiagnostic {
        provider_id: profile.provider_id.clone(),
        endpoint: profile.endpoint.clone(),
        attempt,
        error_code: None,
    }
}

impl RedactedDiagnostic {
    /// Attach a fixed error code without copying an error message or cause.
    #[must_use]
    pub fn with_error_code(mut self, error: RequestError) -> Self {
        self.error_code = Some(error_code(error).to_owned());
        self
    }
}

/// Reject a requested raw-header map.  This function exists only as an
/// explicit fail-closed boundary for adapters; it never inspects, stores, or
/// emits the supplied values.
pub fn reject_raw_headers<T>(_headers: T) -> Result<(), RequestError> {
    Err(RequestError::HeaderNotAllowed)
}

/// Compatibility spelling for adapters attempting to add arbitrary headers.
/// No profile is created and no supplied value is retained.
pub fn profile_for_with_headers<'a, K: Into<EndpointSelector<'a>>, H>(
    _provider_id: &str,
    _endpoint_kind: K,
    _headers: H,
) -> Result<RequestProfile, RequestError> {
    Err(RequestError::HeaderNotAllowed)
}

fn documented_entry(
    provider_id: &str,
    input: EndpointSelector<'_>,
) -> Result<(&'static str, &'static [AuthHeaderKind]), RequestError> {
    let entry = match provider_id {
        "openai" => match input {
            EndpointSelector::Kind(EndpointKind::ChatCompletions) => Some((
                "https://api.openai.com/v1/chat/completions",
                &[AuthHeaderKind::Bearer][..],
            )),
            EndpointSelector::Kind(EndpointKind::Messages) => None,
            EndpointSelector::Name(url) => match url {
                "chat-completions" | "https://api.openai.com/v1/chat/completions" => Some((
                    "https://api.openai.com/v1/chat/completions",
                    &[AuthHeaderKind::Bearer][..],
                )),
                _ => None,
            },
            EndpointSelector::Owned(ref url) => match url.as_str() {
                "chat-completions" | "https://api.openai.com/v1/chat/completions" => Some((
                    "https://api.openai.com/v1/chat/completions",
                    &[AuthHeaderKind::Bearer][..],
                )),
                _ => None,
            },
        },
        "anthropic" => match input {
            EndpointSelector::Kind(EndpointKind::Messages) => Some((
                "https://api.anthropic.com/v1/messages",
                &[AuthHeaderKind::ApiKeyHeader][..],
            )),
            EndpointSelector::Kind(EndpointKind::ChatCompletions) => None,
            EndpointSelector::Name(url) => match url {
                "messages" | "https://api.anthropic.com/v1/messages" => Some((
                    "https://api.anthropic.com/v1/messages",
                    &[AuthHeaderKind::ApiKeyHeader][..],
                )),
                _ => None,
            },
            EndpointSelector::Owned(ref url) => match url.as_str() {
                "messages" | "https://api.anthropic.com/v1/messages" => Some((
                    "https://api.anthropic.com/v1/messages",
                    &[AuthHeaderKind::ApiKeyHeader][..],
                )),
                _ => None,
            },
        },
        _ => return Err(RequestError::UnknownProvider),
    };

    entry.ok_or_else(|| match input {
        EndpointSelector::Name(url)
            if (url.starts_with("http://") || url.starts_with("https://"))
                && !is_https_endpoint(url) =>
        {
            RequestError::BadEndpoint
        }
        EndpointSelector::Owned(url)
            if (url.starts_with("http://") || url.starts_with("https://"))
                && !is_https_endpoint(&url) =>
        {
            RequestError::BadEndpoint
        }
        _ => RequestError::UndocumentedEndpoint,
    })
}

fn validate_endpoint(endpoint: &str) -> Result<(), RequestError> {
    if !is_https_endpoint(endpoint) {
        return Err(RequestError::BadEndpoint);
    }
    Ok(())
}

fn is_https_endpoint(endpoint: &str) -> bool {
    if endpoint.is_empty()
        || endpoint.len() > MAX_ENDPOINT_BYTES
        || !endpoint.starts_with("https://")
    {
        return false;
    }
    if endpoint
        .bytes()
        .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
    {
        return false;
    }

    let authority = &endpoint["https://".len()..];
    let authority_end = authority.find(['/', '?', '#']).unwrap_or(authority.len());
    !authority[..authority_end].is_empty()
}

fn validate_bounds(options: RequestProfileOptions) -> Result<(), RequestError> {
    if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&options.timeout_ms)
        || options.max_retries > MAX_RETRIES
    {
        return Err(RequestError::BadBounds);
    }
    Ok(())
}

fn error_code(error: RequestError) -> &'static str {
    match error {
        RequestError::UnknownProvider => "unknown-provider",
        RequestError::UndocumentedEndpoint => "undocumented-endpoint",
        RequestError::HeaderNotAllowed => "header-not-allowed",
        RequestError::BadEndpoint => "bad-endpoint",
        RequestError::BadBounds => "bad-bounds",
    }
}
