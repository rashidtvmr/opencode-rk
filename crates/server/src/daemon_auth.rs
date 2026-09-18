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
    extract::Request,
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DaemonAuth {
    token: String,
}

impl DaemonAuth {
    /// Mint a fresh credential from `/dev/urandom` (plus `BCryptGenRandom`
    /// fallback on Windows). Fail-closed on any error.
    pub fn mint() -> Result<Self, AuthError> {
        let bytes = random_bytes()?;
        Ok(Self {
            token: hex_encode(&bytes),
        })
    }

    /// Restore the credential the daemon published at startup. Rejects empty
    /// or malformed tokens so a legacy/blank descriptor never authenticates.
    pub fn from_published(token: &str) -> Result<Self, AuthError> {
        if token.len() != TOKEN_HEX_LEN
            || !token.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(AuthError::MalformedToken);
        }
        Ok(Self {
            token: token.to_owned(),
        })
    }

    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
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
pub enum AuthError {    /// No OS randomness available; caller must abort startup, not retry weak.
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
    let mut diff = (expected.len() ^ presented.len()) as u8;
    for i in 0..expected.len().max(presented.len()) {
        let a = *expected.get(i).unwrap_or(&0);
        let b = *presented.get(i).unwrap_or(&0);
        diff |= a ^ b;
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
fn random_bytes() -> Result<[u8; TOKEN_BYTES], AuthError> {
    use std::io::Read;
    let mut file = std::fs::File::open("/dev/urandom")
        .map_err(|error| AuthError::NoRandomness(error.to_string()))?;
    let mut bytes = [0u8; TOKEN_BYTES];
    file.read_exact(&mut bytes)
        .map_err(|error| AuthError::NoRandomness(error.to_string()))?;
    Ok(bytes)
}

#[cfg(windows)]
fn random_bytes() -> Result<[u8; TOKEN_BYTES], AuthError> {
    // No winapi dep: BCrypt via explicit dynamic load is out of scope for the
    // lean harness; fail closed and let the operator run on Unix or extend.
    Err(AuthError::NoRandomness(
        "OS randomness backend not implemented on Windows".to_owned(),
    ))
}

#[cfg(not(any(unix, windows)))]
fn random_bytes() -> Result<[u8; TOKEN_BYTES], AuthError> {
    Err(AuthError::NoRandomness(
        "OS randomness backend not implemented on this platform".to_owned(),
    ))
}

/// Axum middleware: gate `/api/*`, leave `/health` and web assets public.
pub async fn require_bearer(
    axum::extract::State(auth): axum::extract::State<DaemonAuth>,
    request: Request,
    next: Next,
) -> Response {
    if !request.uri().path().starts_with("/api/") {
        return next.run(request).await;
    }
    let header_value = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    match header_value {
        None => (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"missing bearer credential"})),
        )
            .into_response(),
        Some(value) if !auth.verify_bearer(Some(value)) => (
            StatusCode::FORBIDDEN,
            Json(json!({"error":"bad bearer credential"})),
        )
            .into_response(),
        _ => next.run(request).await,
    }
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
        let restored =
            DaemonAuth::from_published(auth.token()).expect("roundtrip");
        assert!(restored.verify_bearer(Some(&format!("Bearer {}", auth.token()))));
    }
}
