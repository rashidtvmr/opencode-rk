//! Bearer credential for the singleton daemon HTTP API (`RC-01`).
//!
//! Decision: `/health` stays public (liveness only: schema_version, status,
//! runtime — no session/tool state). Every `/api/*` route requires
//! `Authorization: Bearer <token>`. Missing credential is `401`, wrong
//! credential is `403`; neither mutates state (middleware rejects before the
//! handler runs).
//!
//! The token is minted at daemon start (32 bytes from `/dev/urandom`, hex)
//! and published in `backend.json` (`BackendDescriptor.auth_token`), which is
//! already a `0600`-equivalent owner-checked file (`daemon.rs`
//! `validate_descriptor_file`). Discovery stays loopback+pid AND credential:
//! a forged descriptor passes the client-side loopback parse but the attacker
//! still lacks the token the live daemon expects.
//!
//! Comparison is constant-time over bytes (manual fold, no `subtle` dep).
//! Token mint is fail-closed: no RNG source means `AuthError`, never a weak
//! fallback.

#![forbid(unsafe_code)]

use axum::{
    body::HttpBody,
    extract::{Extension, Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Length of the raw token in bytes; published form is hex, twice as long.
pub const TOKEN_BYTES: usize = 32;
/// Length of the hex-encoded token.
pub const TOKEN_HEX_LEN: usize = TOKEN_BYTES * 2;

/// Minted credential owned by one daemon instance.
#[derive(Clone)]
pub struct DaemonAuth {
    token: String,
    browser_resume: std::sync::Arc<crate::browser_resume::BrowserResumeRegistry>,
}

impl PartialEq for DaemonAuth {
    fn eq(&self, other: &Self) -> bool {
        self.token == other.token
    }
}

impl Eq for DaemonAuth {}

impl DaemonAuth {
    /// Mint a fresh credential from `/dev/urandom` (plus `BCryptGenRandom`
    /// fallback on Windows). Fail-closed on any error.
    pub fn mint() -> Result<Self, AuthError> {
        let bytes = random_bytes()?;
        Ok(Self {
            token: hex_encode(&bytes),
            browser_resume: std::sync::Arc::new(
                crate::browser_resume::BrowserResumeRegistry::default(),
            ),
        })
    }

    /// Restore the credential the daemon published at startup. Rejects empty
    /// or malformed tokens so a legacy/blank descriptor never authenticates.
    pub fn from_published(token: &str) -> Result<Self, AuthError> {
        if token.len() != TOKEN_HEX_LEN || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(AuthError::MalformedToken);
        }
        Ok(Self {
            token: token.to_owned(),
            browser_resume: std::sync::Arc::new(
                crate::browser_resume::BrowserResumeRegistry::default(),
            ),
        })
    }

    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }

    pub(crate) fn browser_resume(
        &self,
    ) -> std::sync::Arc<crate::browser_resume::BrowserResumeRegistry> {
        self.browser_resume.clone()
    }

    /// Constant-time bearer check. `None` (missing/odd header) is `false`.
    #[must_use]
    pub fn verify_bearer(&self, header_value: Option<&str>) -> bool {
        let Some(value) = header_value else {
            return false;
        };
        let Some(presented) = value.strip_prefix("Bearer ") else {
            return false;
        };
        constant_time_eq(self.token.as_bytes(), presented.as_bytes())
    }
}

/// Failures minting or restoring the daemon credential.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthError {
    /// No OS randomness available; caller must abort startup, not retry weak.
    NoRandomness(String),
    /// Published token is empty or not 64 hex chars.
    MalformedToken,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoRandomness(detail) => write!(f, "no OS randomness: {detail}"),
            Self::MalformedToken => write!(f, "published daemon token is malformed"),
        }
    }
}

impl std::error::Error for AuthError {}

/// Constant-time equality: always walks all bytes of the longer side.
fn constant_time_eq(expected: &[u8], presented: &[u8]) -> bool {
    let mut diff = expected.len() ^ presented.len();
    for i in 0..expected.len().max(presented.len()) {
        let a = *expected.get(i).unwrap_or(&0);
        let b = *presented.get(i).unwrap_or(&0);
        diff |= usize::from(a ^ b);
    }
    diff == 0
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(unix)]
pub(crate) fn random_bytes() -> Result<[u8; TOKEN_BYTES], AuthError> {
    use std::io::Read;
    let mut file = std::fs::File::open("/dev/urandom")
        .map_err(|error| AuthError::NoRandomness(error.to_string()))?;
    let mut bytes = [0u8; TOKEN_BYTES];
    file.read_exact(&mut bytes)
        .map_err(|error| AuthError::NoRandomness(error.to_string()))?;
    Ok(bytes)
}

#[cfg(windows)]
pub(crate) fn random_bytes() -> Result<[u8; TOKEN_BYTES], AuthError> {
    // No winapi dep: BCrypt via explicit dynamic load is out of scope for the
    // lean harness; fail closed and let the operator run on Unix or extend.
    Err(AuthError::NoRandomness(
        "OS randomness backend not implemented on Windows".to_owned(),
    ))
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn random_bytes() -> Result<[u8; TOKEN_BYTES], AuthError> {
    Err(AuthError::NoRandomness(
        "OS randomness backend not implemented on this platform".to_owned(),
    ))
}

/// Gate API and credential routes while keeping health and static assets public.
pub async fn require_bearer(
    State(auth): State<DaemonAuth>,
    mut request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_owned();
    if !path.starts_with("/api/") && !path.starts_with("/auth/") {
        return next.run(request).await;
    }
    let method = request.method().clone();
    let authorization = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let origin_header = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let local_origin = request
        .headers()
        .get(header::HOST)
        .and_then(|host| host.to_str().ok())
        .and_then(loopback_origin_from_host);
    if let Some(origin) = origin_header.as_deref() {
        if !valid_loopback_origin(origin) || local_origin.as_deref() != Some(origin) {
            return forbidden("browser origin denied");
        }
    }
    let resume_api_path = path == "/api/capabilities"
        || path == "/api/workspaces"
        || path == "/api/models"
        || path.starts_with("/api/models/")
        || path == "/api/sessions"
        || path.starts_with("/api/sessions/")
        || path.starts_with("/api/session/");
    let is_issue = path == "/auth/browser-resume" && method == axum::http::Method::POST;
    let is_revoke = path == "/auth/browser-resume" && method == axum::http::Method::DELETE;
    let is_exchange = path == "/auth/browser-resume/exchange" && method == axum::http::Method::POST;
    if path.starts_with("/auth/browser-resume") {
        if is_issue {
            if origin_header.is_none() || local_origin.as_deref() != origin_header.as_deref() {
                return forbidden("browser origin denied");
            }
            return match authorization.as_deref() {
                None => (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"message":"trusted native launch required"})),
                )
                    .into_response(),
                Some(value) if auth.verify_bearer(Some(value)) => {
                    request.extensions_mut().insert(auth.clone());
                    next.run(request).await
                }
                Some(_) => forbidden("trusted native launch required"),
            };
        }
        if is_revoke || is_exchange {
            if origin_header.is_none() || local_origin.as_deref() != origin_header.as_deref() {
                return forbidden("browser origin denied");
            }
            if is_exchange
                && request
                    .body()
                    .size_hint()
                    .upper()
                    .is_none_or(|size| size > 4096)
            {
                return (
                    StatusCode::PAYLOAD_TOO_LARGE,
                    Json(json!({"message":"browser resume request too large"})),
                )
                    .into_response();
            }
            let Some(capability) = authorization
                .as_deref()
                .and_then(|value| value.strip_prefix("BrowserResume "))
            else {
                return forbidden("browser resume denied");
            };
            let registry = auth.browser_resume();
            if !registry.verify(capability, origin_header.as_deref().unwrap_or_default()) {
                return forbidden("browser resume denied");
            }
            request.extensions_mut().insert(auth.browser_resume());
            return next.run(request).await;
        }
        return match authorization.as_deref() {
            None => missing_credential(),
            Some(value) if auth.verify_bearer(Some(value)) => next.run(request).await,
            Some(_) => forbidden("bad bearer credential"),
        };
    }
    if !path.starts_with("/api/") {
        return match authorization.as_deref() {
            None => missing_credential(),
            Some(value) if auth.verify_bearer(Some(value)) => next.run(request).await,
            Some(_) => forbidden("bad bearer credential"),
        };
    }
    if let Some(value) = authorization.as_deref() {
        if auth.verify_bearer(Some(value)) {
            return next.run(request).await;
        }
        if resume_api_path {
            if let Some(capability) = value.strip_prefix("BrowserResume ") {
                let safe_method =
                    method == axum::http::Method::GET || method == axum::http::Method::HEAD;
                let permission_reply = path.starts_with("/api/session/")
                    && path.ends_with("/reply")
                    && method == axum::http::Method::POST;
                let bound_origin = origin_header.as_deref().or(local_origin.as_deref());
                if (safe_method || permission_reply || origin_header.is_some())
                    && bound_origin
                        .is_some_and(|origin| auth.browser_resume().verify(capability, origin))
                {
                    request.extensions_mut().insert(auth.browser_resume());
                    return next.run(request).await;
                }
            }
        }
    }
    match authorization.as_deref() {
        None => missing_credential(),
        Some(_) => forbidden("bad bearer credential"),
    }
}

fn missing_credential() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"error":"missing bearer credential"})),
    )
        .into_response()
}

fn forbidden(message: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(json!({"message":message,"error":"bad bearer credential"})),
    )
        .into_response()
}

fn valid_loopback_origin(origin: &str) -> bool {
    let Some(port) = origin.strip_prefix("http://127.0.0.1:") else {
        return false;
    };
    !port.is_empty()
        && port.len() <= 5
        && port.bytes().all(|b| b.is_ascii_digit())
        && !(port.len() > 1 && port.starts_with('0'))
        && port.parse::<u16>().is_ok_and(|port| port != 0)
}

fn loopback_origin_from_host(host: &str) -> Option<String> {
    let origin = format!("http://{host}");
    valid_loopback_origin(&origin).then_some(origin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rc01_t01_mint_is_hex_and_unique() {
        let first = DaemonAuth::mint().expect("mint");
        let second = DaemonAuth::mint().expect("mint");
        assert_eq!(first.token().len(), TOKEN_HEX_LEN);
        assert!(first.token().bytes().all(|b| b.is_ascii_hexdigit()));
        assert_ne!(first.token(), second.token());
    }

    #[test]
    fn rc01_t02_verify_roundtrip() {
        let auth = DaemonAuth::mint().expect("mint");
        let header = format!("Bearer {}", auth.token());
        assert!(auth.verify_bearer(Some(&header)));
    }

    #[test]
    fn rc01_t03_missing_or_odd_header_rejected() {
        let auth = DaemonAuth::mint().expect("mint");
        assert!(!auth.verify_bearer(None));
        assert!(!auth.verify_bearer(Some("")));
        assert!(!auth.verify_bearer(Some(auth.token())));
        assert!(!auth.verify_bearer(Some("Basic abc")));
    }

    #[test]
    fn rc01_t04_wrong_token_rejected() {
        let auth = DaemonAuth::mint().expect("mint");
        assert!(!auth.verify_bearer(Some("Bearer 00")));
        let other = DaemonAuth::mint().expect("mint");
        assert!(!auth.verify_bearer(Some(&format!("Bearer {}", other.token()))));
    }

    #[test]
    fn rc01_t05_published_restore_rejects_legacy_blank() {
        assert_eq!(
            DaemonAuth::from_published(""),
            Err(AuthError::MalformedToken)
        );
        assert_eq!(
            DaemonAuth::from_published("short"),
            Err(AuthError::MalformedToken)
        );
        let auth = DaemonAuth::mint().expect("mint");
        let restored = DaemonAuth::from_published(auth.token()).expect("roundtrip");
        assert!(restored.verify_bearer(Some(&format!("Bearer {}", auth.token()))));
    }
}
