//! Pure, documentation-backed request profiles for provider API calls.
//!
//! This module owns request shape, not credentials.  A profile contains an
//! allowlisted provider endpoint and authentication *kinds* only.  Secret
//! values, arbitrary headers, network access, and client-impersonation fields
//! deliberately have no representation here.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs::File, io::Read, path::PathBuf};
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
const CATALOG_ENV_VAR: &str = "OPENCODE_RK_PROVIDER_CATALOG_PATH";
const MAX_CATALOG_BYTES: usize = 256 * 1024;
const MAX_PROVIDERS: usize = 32;
const MAX_ENDPOINTS_PER_PROVIDER: usize = 16;
const MAX_MODEL_ALIASES_PER_PROVIDER: usize = 32;
const MAX_LIMITATIONS_PER_PROVIDER: usize = 32;
const MAX_CATALOG_FIELD_BYTES: usize = 4 * 1024;
const MAX_ID_BYTES: usize = 128;
const ALLOWED_MODEL_PLACEHOLDER: &str = "{model=models/*}";
const EMBEDDED_CATALOG: &str = include_str!("../../../docs/provider-compatibility.json");

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
    validate_endpoint(&endpoint)?;
    validate_bounds(options)?;

    let profile = RequestProfile {
        provider_id: provider_id.to_owned(),
        endpoint,
        auth_headers,
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
        if endpoint != self.endpoint || self.auth_headers != auth_headers {
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompatibilityCatalog {
    catalog_version: String,
    providers: Vec<CatalogProvider>,
    updated: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogProvider {
    id: String,
    display_name: String,
    doc_source: String,
    doc_date: String,
    endpoints: Vec<CatalogEndpoint>,
    model_aliases: Vec<String>,
    refresh: CatalogRefresh,
    limitations: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogEndpoint {
    kind: String,
    url_template: String,
    auth_headers: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogRefresh {
    supported: bool,
    method: String,
}

fn documented_entry(
    provider_id: &str,
    input: EndpointSelector<'_>,
) -> Result<(String, Vec<AuthHeaderKind>), RequestError> {
    let catalog = load_catalog()?;
    let provider = catalog
        .providers
        .iter()
        .find(|entry| entry.id == provider_id)
        .ok_or(RequestError::UnknownProvider)?;
    let selector = match input {
        EndpointSelector::Kind(EndpointKind::ChatCompletions) => "chat-completions".to_owned(),
        EndpointSelector::Kind(EndpointKind::Messages) => "messages".to_owned(),
        EndpointSelector::Name(value) => value.to_owned(),
        EndpointSelector::Owned(value) => value,
    };
    let endpoint = provider
        .endpoints
        .iter()
        .find(|entry| entry.kind == selector || entry.url_template == selector)
        .ok_or_else(|| {
            if (selector.starts_with("http://") || selector.starts_with("https://"))
                && !is_https_endpoint(&selector)
            {
                RequestError::BadEndpoint
            } else {
                RequestError::UndocumentedEndpoint
            }
        })?;
    Ok((
        endpoint.url_template.clone(),
        map_auth_headers(&endpoint.auth_headers)?,
    ))
}

fn load_catalog() -> Result<CompatibilityCatalog, RequestError> {
    let override_path = std::env::var_os(CATALOG_ENV_VAR)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    let raw = match override_path {
        Some(path) => read_bounded_catalog(path)?,
        None => EMBEDDED_CATALOG.as_bytes().to_vec(),
    };
    if raw.len() > MAX_CATALOG_BYTES || contains_secret_like_bytes(&raw) {
        return Err(RequestError::BadEndpoint);
    }
    let catalog: CompatibilityCatalog =
        serde_json::from_slice(&raw).map_err(|_| RequestError::BadEndpoint)?;
    validate_catalog(&catalog)?;
    Ok(catalog)
}

fn read_bounded_catalog(path: PathBuf) -> Result<Vec<u8>, RequestError> {
    let file = File::open(path).map_err(|_| RequestError::BadEndpoint)?;
    let size = file
        .metadata()
        .map_err(|_| RequestError::BadEndpoint)?
        .len();
    if size > MAX_CATALOG_BYTES as u64 {
        return Err(RequestError::BadEndpoint);
    }
    let mut raw = Vec::with_capacity(size as usize);
    file.take((MAX_CATALOG_BYTES + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| RequestError::BadEndpoint)?;
    if raw.len() > MAX_CATALOG_BYTES {
        return Err(RequestError::BadEndpoint);
    }
    Ok(raw)
}

fn validate_catalog(catalog: &CompatibilityCatalog) -> Result<(), RequestError> {
    if catalog.catalog_version != "1"
        || !is_date(&catalog.updated)
        || catalog.providers.is_empty()
        || catalog.providers.len() > MAX_PROVIDERS
    {
        return Err(RequestError::BadEndpoint);
    }
    let mut provider_ids = HashSet::new();
    for provider in &catalog.providers {
        if !is_id(&provider.id)
            || !provider_ids.insert(provider.id.as_str())
            || !is_bounded_text(&provider.display_name, MAX_CATALOG_FIELD_BYTES)
            || !is_date(&provider.doc_date)
            || !is_safe_https_url(&provider.doc_source, false)
            || provider.endpoints.is_empty()
            || provider.endpoints.len() > MAX_ENDPOINTS_PER_PROVIDER
            || provider.model_aliases.is_empty()
            || provider.model_aliases.len() > MAX_MODEL_ALIASES_PER_PROVIDER
            || provider.limitations.len() > MAX_LIMITATIONS_PER_PROVIDER
            || !is_bounded_text(&provider.refresh.method, MAX_CATALOG_FIELD_BYTES)
        {
            return Err(RequestError::BadEndpoint);
        }
        let _refresh_supported = provider.refresh.supported;
        if provider
            .model_aliases
            .iter()
            .any(|value| !is_bounded_text(value, MAX_ID_BYTES))
            || provider
                .limitations
                .iter()
                .any(|value| !is_bounded_text(value, MAX_CATALOG_FIELD_BYTES))
        {
            return Err(RequestError::BadEndpoint);
        }
        let mut aliases = HashSet::new();
        if provider
            .model_aliases
            .iter()
            .any(|alias| !aliases.insert(alias.as_str()))
        {
            return Err(RequestError::BadEndpoint);
        }
        let mut endpoint_kinds = HashSet::new();
        for endpoint in &provider.endpoints {
            if !is_id(&endpoint.kind)
                || !endpoint_kinds.insert(endpoint.kind.as_str())
                || endpoint.auth_headers.is_empty()
                || endpoint.auth_headers.len() > MAX_AUTH_HEADERS
                || validate_endpoint(&endpoint.url_template).is_err()
                || map_auth_headers(&endpoint.auth_headers).is_err()
            {
                return Err(RequestError::BadEndpoint);
            }
        }
    }
    Ok(())
}

fn map_auth_headers(headers: &[String]) -> Result<Vec<AuthHeaderKind>, RequestError> {
    let mut names = HashSet::new();
    let mut kinds = Vec::new();
    for header in headers {
        if header.is_empty()
            || header.len() > MAX_ID_BYTES
            || !header
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || !names.insert(header.as_str())
        {
            return Err(RequestError::HeaderNotAllowed);
        }
        let kind = match header.as_str() {
            "Authorization" => Some(AuthHeaderKind::Bearer),
            "x-api-key" | "x-goog-api-key" => Some(AuthHeaderKind::ApiKeyHeader),
            "anthropic-version" => None,
            _ => return Err(RequestError::HeaderNotAllowed),
        };
        if let Some(kind) = kind {
            if !kinds.contains(&kind) {
                kinds.push(kind);
            }
        }
    }
    if kinds.is_empty() {
        return Err(RequestError::HeaderNotAllowed);
    }
    Ok(kinds)
}

fn contains_secret_like_bytes(raw: &[u8]) -> bool {
    [
        b"sk-".as_slice(),
        b"sk-ant-".as_slice(),
        b"Bearer ".as_slice(),
    ]
    .iter()
    .any(|needle| raw.windows(needle.len()).any(|window| window == *needle))
}

fn is_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn is_bounded_text(value: &str, limit: usize) -> bool {
    !value.is_empty()
        && value.len() <= limit
        && !value
            .bytes()
            .any(|byte| byte.is_ascii_control() && !matches!(byte, b'\n' | b'\r' | b'\t'))
}

fn is_date(value: &str) -> bool {
    if value.len() != 10
        || value.as_bytes()[4] != b'-'
        || value.as_bytes()[7] != b'-'
        || !value
            .bytes()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
    {
        return false;
    }
    let month = value[5..7].parse::<u8>().ok();
    let day = value[8..10].parse::<u8>().ok();
    matches!(month, Some(1..=12)) && matches!(day, Some(1..=31))
}

fn validate_endpoint(endpoint: &str) -> Result<(), RequestError> {
    if !is_https_endpoint(endpoint) {
        return Err(RequestError::BadEndpoint);
    }
    Ok(())
}

fn is_https_endpoint(endpoint: &str) -> bool {
    is_safe_https_url(endpoint, true)
}

fn is_safe_https_url(value: &str, allow_model_placeholder: bool) -> bool {
    if value.is_empty()
        || value.len() > MAX_ENDPOINT_BYTES
        || value
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
    {
        return false;
    }
    let parsed_value = if allow_model_placeholder {
        let occurrences = value.matches(ALLOWED_MODEL_PLACEHOLDER).count();
        if occurrences > 1 {
            return false;
        }
        let without_allowed = value.replace(ALLOWED_MODEL_PLACEHOLDER, "models/model");
        if without_allowed.contains(['{', '}']) {
            return false;
        }
        without_allowed
    } else {
        if value.contains(['{', '}']) {
            return false;
        }
        value.to_owned()
    };
    let Ok(url) = reqwest::Url::parse(&parsed_value) else {
        return false;
    };
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return false;
    }
    !url.query_pairs().any(|(name, _)| {
        matches!(
            name.to_ascii_lowercase().as_str(),
            "key" | "api_key" | "apikey" | "token" | "secret" | "password" | "authorization"
        )
    })
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
