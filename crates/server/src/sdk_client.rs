//! SDK-001: typed SDK HTTP client with abort and redaction.
//!
//! Pure synchronous boundary over a caller-supplied [`HttpPort`]. Builds typed
//! GET/POST requests against one validated base URL, propagates the workspace
//! directory as both an `x-workspace-directory` header and a `?directory=`
//! (or `&directory=`) query rewrite, and decodes responses to [`TypedBody`].
//!
//! Performs no socket I/O, spawns no process, retains no state, logs nothing,
//! and persists nothing. Timeouts and aborts are enforced/observed through the
//! caller port and [`AbortFlag`]; this module only propagates them without
//! retaining bodies. Errors carry variant names and numeric status codes only.

#![forbid(unsafe_code)]

/// Timeout bound (seconds) enforced by the caller port as a deadline.
pub const SDK_TIMEOUT_SECS: u64 = 30;
/// Maximum base URL length in chars.
pub const MAX_BASE_URL_CHARS: usize = 2048;
/// Maximum workspace directory length in chars.
pub const MAX_DIR_CHARS: usize = 1024;
/// Maximum request path length in chars.
pub const MAX_PATH_CHARS: usize = 1024;
/// Maximum request body bytes retained.
pub const MAX_REQUEST_BODY_BYTES: usize = 1024 * 1024;
/// Maximum response body bytes retained.
pub const MAX_RESPONSE_BODY_BYTES: usize = 1024 * 1024;

/// Typed SDK failures. Variants carry no bodies, tokens, or URLs by
/// construction, so `Debug`/`Display` output is secret-free.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdkError {
    InvalidBaseUrl,
    BadStatus { status: u16 },
    Decode,
    Timeout,
    Abort,
    TooLarge,
    InvalidInput,
}

impl std::fmt::Display for SdkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidBaseUrl => write!(f, "invalid base url"),
            Self::BadStatus { status } => write!(f, "bad status: {status}"),
            Self::Decode => write!(f, "decode error"),
            Self::Timeout => write!(f, "timeout"),
            Self::Abort => write!(f, "aborted"),
            Self::TooLarge => write!(f, "too large"),
            Self::InvalidInput => write!(f, "invalid input"),
        }
    }
}

impl std::error::Error for SdkError {}

/// Request method observed by the fixture port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
}

/// Outbound request handed to the caller port. Owned by the caller port for
/// the duration of `send` only; nothing is retained here afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Inbound response returned by the caller port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Caller-supplied transport. Owns all sockets and in-flight slots; reclaims
/// the slot on `Timeout`/`Abort` before returning.
pub trait HttpPort {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, SdkError>;
}

/// Caller-supplied abort signal. `flag() == true` cancels the call.
pub trait AbortFlag {
    fn flag(&self) -> bool;
}

/// Stateless typed client over one validated base URL.
pub struct SdkClient {
    base_url: String,
}

// Manual `Debug`: never emits the base URL (may embed credentials).
impl std::fmt::Debug for SdkClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SdkClient(..)")
    }
}

impl Clone for SdkClient {
    fn clone(&self) -> Self {
        Self {
            base_url: self.base_url.clone(),
        }
    }
}

impl SdkClient {
    /// Validate the caller-supplied base URL. Allocates only the stripped URL.
    pub fn new(base_url: &str) -> Result<Self, SdkError> {
        if base_url.is_empty() || base_url.chars().count() > MAX_BASE_URL_CHARS {
            return Err(SdkError::InvalidBaseUrl);
        }
        let rest = if let Some(rest) = base_url.strip_prefix("http://") {
            rest
        } else if let Some(rest) = base_url.strip_prefix("https://") {
            rest
        } else {
            return Err(SdkError::InvalidBaseUrl);
        };
        if rest.is_empty() {
            return Err(SdkError::InvalidBaseUrl);
        }
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    /// Typed GET against `base_url + path`.
    pub fn get(
        &self,
        path: &str,
        dir: &str,
        http: &dyn HttpPort,
        abort: &dyn AbortFlag,
    ) -> Result<TypedBody, SdkError> {
        if abort.flag() {
            return Err(SdkError::Abort);
        }
        validate_path(path)?;
        validate_dir(dir)?;
        let request = HttpRequest {
            method: HttpMethod::Get,
            url: rewrite_url(&self.base_url, path, dir),
            headers: vec![("x-workspace-directory".to_string(), dir.to_string())],
            body: Vec::new(),
        };
        let response = http.send(&request)?;
        decode_response(response)
    }

    /// Typed POST with body (max 1 MiB) against `base_url + path`.
    pub fn post(
        &self,
        path: &str,
        dir: &str,
        body: &[u8],
        http: &dyn HttpPort,
        abort: &dyn AbortFlag,
    ) -> Result<TypedBody, SdkError> {
        if abort.flag() {
            return Err(SdkError::Abort);
        }
        validate_path(path)?;
        validate_dir(dir)?;
        if body.len() > MAX_REQUEST_BODY_BYTES {
            return Err(SdkError::TooLarge);
        }
        let request = HttpRequest {
            method: HttpMethod::Post,
            url: rewrite_url(&self.base_url, path, dir),
            headers: vec![("x-workspace-directory".to_string(), dir.to_string())],
            body: body.to_vec(),
        };
        let response = http.send(&request)?;
        decode_response(response)
    }
}

/// Decoded typed body. `Debug` is redacted; read via [`TypedBody::value`].
pub struct TypedBody {
    value: serde_json::Value,
}

impl TypedBody {
    /// Borrow the decoded value.
    #[must_use]
    pub fn value(&self) -> &serde_json::Value {
        &self.value
    }

    /// Consume into the decoded value.
    #[allow(dead_code)]
    #[must_use]
    pub fn into_value(self) -> serde_json::Value {
        self.value
    }
}

// Manual `Debug`: bodies may carry secrets; never emit them.
impl std::fmt::Debug for TypedBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TypedBody(..)")
    }
}

fn validate_dir(dir: &str) -> Result<(), SdkError> {
    if dir.is_empty() || dir.chars().any(|c| c.is_control()) {
        return Err(SdkError::InvalidInput);
    }
    if dir.chars().count() > MAX_DIR_CHARS {
        return Err(SdkError::TooLarge);
    }
    Ok(())
}

fn validate_path(path: &str) -> Result<(), SdkError> {
    if path.is_empty() || !path.starts_with('/') || path.chars().any(|c| c.is_control()) {
        return Err(SdkError::InvalidInput);
    }
    if path.chars().count() > MAX_PATH_CHARS {
        return Err(SdkError::TooLarge);
    }
    Ok(())
}

fn rewrite_url(base_url: &str, path: &str, dir: &str) -> String {
    let mut url = String::with_capacity(base_url.len() + path.len() + dir.len() + 12);
    url.push_str(base_url);
    url.push_str(path);
    url.push(if url.contains('?') { '&' } else { '?' });
    url.push_str("directory=");
    url.push_str(&pct_encode(dir));
    url
}

/// RFC 3986 percent-encoding over UTF-8 bytes; unreserved bytes pass through.
fn pct_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(*byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn decode_response(response: HttpResponse) -> Result<TypedBody, SdkError> {
    if !(200..=299).contains(&response.status) {
        return Err(SdkError::BadStatus {
            status: response.status,
        });
    }
    if response.body.len() > MAX_RESPONSE_BODY_BYTES {
        return Err(SdkError::TooLarge);
    }
    if response.body.is_empty() {
        return Err(SdkError::Decode);
    }
    let value: serde_json::Value =
        serde_json::from_slice(&response.body).map_err(|_| SdkError::Decode)?;
    Ok(TypedBody { value })
}
