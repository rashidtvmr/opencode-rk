//! Bounded OpenAI ChatGPT device authorization transport.
//!
//! The endpoint and payload contract follows the latest local upstream plugin
//! at `/Users/mymac/Projects/tmp/opencode` (HEAD `450e7b924...`).  The browser
//! is deliberately not opened here: the caller receives the URL and code and
//! makes the explicit UI choice.  This module owns its redirect-disabled,
//! timeout-bounded HTTP client so callers cannot accidentally provide a client
//! with unsafe redirect or timeout policy.
#![forbid(unsafe_code)]

use std::{fmt, time::Duration};

use reqwest::{Client, Response, StatusCode, Url};
use serde::{Deserialize, Serialize, Serializer};
use std::future::Future;
use tokio::sync::watch;

pub const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub const ISSUER: &str = "https://auth.openai.com";
pub const DEVICE_AUTH_URL: &str = "https://auth.openai.com/api/accounts/deviceauth/usercode";
pub const DEVICE_TOKEN_URL: &str = "https://auth.openai.com/api/accounts/deviceauth/token";
pub const OAUTH_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
pub const DEVICE_LOGIN_URL: &str = "https://auth.openai.com/codex/device";
pub const DEVICE_CALLBACK_URL: &str = "https://auth.openai.com/deviceauth/callback";
pub const MAX_USER_CODE_BYTES: usize = 256;
pub const MAX_TOKEN_BYTES: usize = 64 * 1024;
pub const MAX_DEVICE_FIELD_BYTES: usize = 4 * 1024;
pub const MAX_RESPONSE_BYTES: usize = 256 * 1024;
pub const MAX_POLLS: usize = 240;
pub const MAX_EXPIRES_IN_SECONDS: u64 = 30 * 24 * 60 * 60;
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
pub const POLLING_SAFETY_MARGIN: Duration = Duration::from_millis(3_000);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceEndpoints {
    usercode: String,
    token: String,
    oauth_token: String,
    login_url: String,
    callback_url: String,
    local_test: bool,
}

impl Default for DeviceEndpoints {
    fn default() -> Self {
        Self {
            usercode: DEVICE_AUTH_URL.into(),
            token: DEVICE_TOKEN_URL.into(),
            oauth_token: OAUTH_TOKEN_URL.into(),
            login_url: DEVICE_LOGIN_URL.into(),
            callback_url: DEVICE_CALLBACK_URL.into(),
            local_test: false,
        }
    }
}

impl DeviceEndpoints {
    /// Explicit test-only endpoint construction. Production endpoints cannot
    /// be changed to HTTP or another host through this public API.
    pub fn for_local_test(base: &str) -> Result<Self, OpenAiDeviceAuthError> {
        let base = parse_local_base(base)?;
        Ok(Self {
            usercode: format!("{base}/api/accounts/deviceauth/usercode"),
            token: format!("{base}/api/accounts/deviceauth/token"),
            oauth_token: format!("{base}/oauth/token"),
            login_url: format!("{base}/codex/device"),
            callback_url: format!("{base}/deviceauth/callback"),
            local_test: true,
        })
    }

    fn validate(&self) -> Result<(), OpenAiDeviceAuthError> {
        let expected = Self::default();
        if self.local_test {
            let urls = [
                (&self.usercode, "/api/accounts/deviceauth/usercode"),
                (&self.token, "/api/accounts/deviceauth/token"),
                (&self.oauth_token, "/oauth/token"),
                (&self.login_url, "/codex/device"),
                (&self.callback_url, "/deviceauth/callback"),
            ];
            let parsed = urls
                .iter()
                .map(|(value, path)| parse_local_endpoint(value, path))
                .collect::<Result<Vec<_>, _>>()?;
            if parsed.iter().any(|base| base != &parsed[0]) {
                return Err(OpenAiDeviceAuthError::EndpointPolicy);
            }
            return Ok(());
        }
        if self == &expected {
            Ok(())
        } else {
            Err(OpenAiDeviceAuthError::EndpointPolicy)
        }
    }
}

fn parse_local_base(value: &str) -> Result<String, OpenAiDeviceAuthError> {
    let url = Url::parse(value).map_err(|_| OpenAiDeviceAuthError::EndpointPolicy)?;
    if url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || url.username() != ""
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        return Err(OpenAiDeviceAuthError::EndpointPolicy);
    }
    Ok(format!("http://{}", url.authority()))
}

fn parse_local_endpoint(value: &str, expected_path: &str) -> Result<String, OpenAiDeviceAuthError> {
    let url = Url::parse(value).map_err(|_| OpenAiDeviceAuthError::EndpointPolicy)?;
    if url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || url.username() != ""
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != expected_path
    {
        return Err(OpenAiDeviceAuthError::EndpointPolicy);
    }
    Ok(format!("http://{}", url.authority()))
}

#[derive(Clone, Eq, PartialEq)]
pub struct DeviceAuthorization {
    pub device_auth_id: String,
    pub user_code: String,
    pub verification_url: String,
    pub interval: Duration,
}

impl fmt::Debug for DeviceAuthorization {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceAuthorization")
            .field("device_auth_id", &"[REDACTED]")
            .field("user_code", &"[REDACTED]")
            .field("verification_url", &self.verification_url)
            .field("interval", &self.interval)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct OpenAiCredential {
    pub access: String,
    pub refresh: String,
    pub expires_at_ms: u64,
    pub account_id: Option<String>,
}

impl Serialize for OpenAiCredential {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct Redacted {
            expires_at_ms: u64,
        }
        Redacted {
            expires_at_ms: self.expires_at_ms,
        }
        .serialize(serializer)
    }
}

impl fmt::Debug for OpenAiCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenAiCredential")
            .field("access", &"[REDACTED]")
            .field("refresh", &"[REDACTED]")
            .field("expires_at_ms", &self.expires_at_ms)
            .field("account_id", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, thiserror::Error)]
pub enum OpenAiDeviceAuthError {
    #[error("request failed")]
    Request,
    #[error("invalid provider response")]
    InvalidResponse,
    #[error("response body exceeds limit")]
    BodyTooLarge,
    #[error("device authorization denied")]
    Denied,
    #[error("device authorization expired")]
    Expired,
    #[error("device authorization cancelled")]
    Cancelled,
    #[error("device authorization poll limit reached")]
    PollLimit,
    #[error("endpoint policy rejected")]
    EndpointPolicy,
    #[error("expiry is invalid or unreasonable")]
    InvalidExpiry,
}

/// Owned auth transport: redirects are disabled and every request has a hard
/// timeout. The caller cannot substitute an unsafe `reqwest::Client`.
#[derive(Clone)]
pub struct AuthTransport {
    client: Client,
}

impl AuthTransport {
    pub fn new() -> Result<Self, OpenAiDeviceAuthError> {
        Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(REQUEST_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map(|client| Self { client })
            .map_err(|_| OpenAiDeviceAuthError::Request)
    }
}

#[derive(Deserialize)]
struct DeviceResponse {
    device_auth_id: String,
    user_code: String,
    interval: String,
}
#[derive(Deserialize)]
struct DeviceTokenResponse {
    authorization_code: String,
    code_verifier: String,
}
#[derive(Deserialize)]
struct TokenResponse {
    id_token: String,
    access_token: String,
    refresh_token: String,
    expires_in: Option<u64>,
}
#[derive(Serialize)]
struct UserCodeRequest<'a> {
    client_id: &'a str,
}
#[derive(Serialize)]
struct DeviceTokenRequest<'a> {
    device_auth_id: &'a str,
    user_code: &'a str,
}

pub async fn begin_device_authorization(
    transport: &AuthTransport,
    endpoints: &DeviceEndpoints,
    cancel: &mut watch::Receiver<bool>,
) -> Result<DeviceAuthorization, OpenAiDeviceAuthError> {
    endpoints.validate()?;
    if *cancel.borrow() {
        return Err(OpenAiDeviceAuthError::Cancelled);
    }
    let request = transport
        .client
        .post(&endpoints.usercode)
        .header("content-type", "application/json")
        .header("user-agent", "opencode")
        .json(&UserCodeRequest {
            client_id: CLIENT_ID,
        })
        .send();
    let response = cancellable(cancel, request).await?;
    if !response.status().is_success() {
        return Err(OpenAiDeviceAuthError::Request);
    }
    let data: DeviceResponse = read_json(response, cancel).await?;
    if data.device_auth_id.is_empty()
        || data.device_auth_id.len() > MAX_DEVICE_FIELD_BYTES
        || data.user_code.is_empty()
        || data.user_code.len() > MAX_USER_CODE_BYTES
    {
        return Err(OpenAiDeviceAuthError::InvalidResponse);
    }
    let seconds = data
        .interval
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0 && *value <= 3600)
        .unwrap_or(5);
    Ok(DeviceAuthorization {
        device_auth_id: data.device_auth_id,
        user_code: data.user_code,
        verification_url: endpoints.login_url.clone(),
        interval: Duration::from_secs(seconds),
    })
}

pub async fn authorize_device(
    transport: &AuthTransport,
    endpoints: &DeviceEndpoints,
    device: &DeviceAuthorization,
    cancel: &mut watch::Receiver<bool>,
) -> Result<OpenAiCredential, OpenAiDeviceAuthError> {
    endpoints.validate()?;
    if *cancel.borrow() {
        return Err(OpenAiDeviceAuthError::Cancelled);
    }
    if device.device_auth_id.is_empty()
        || device.device_auth_id.len() > MAX_DEVICE_FIELD_BYTES
        || device.user_code.is_empty()
        || device.user_code.len() > MAX_USER_CODE_BYTES
        || device.interval.is_zero()
        || device.interval > Duration::from_secs(3600)
    {
        return Err(OpenAiDeviceAuthError::InvalidResponse);
    }
    let delay = device
        .interval
        .checked_add(POLLING_SAFETY_MARGIN)
        .ok_or(OpenAiDeviceAuthError::InvalidResponse)?;
    for _ in 0..MAX_POLLS {
        if *cancel.borrow() {
            return Err(OpenAiDeviceAuthError::Cancelled);
        }
        await_or_cancel(cancel, tokio::time::sleep(delay))
            .await
            .map_err(|_| OpenAiDeviceAuthError::Cancelled)?;
        let request = transport
            .client
            .post(&endpoints.token)
            .header("content-type", "application/json")
            .header("user-agent", "opencode")
            .json(&DeviceTokenRequest {
                device_auth_id: &device.device_auth_id,
                user_code: &device.user_code,
            })
            .send();
        let response = cancellable(cancel, request).await?;
        if response.status().is_success() {
            let token: DeviceTokenResponse = read_json(response, cancel).await?;
            if token.authorization_code.is_empty()
                || token.authorization_code.len() > MAX_DEVICE_FIELD_BYTES
                || token.code_verifier.is_empty()
                || token.code_verifier.len() > MAX_DEVICE_FIELD_BYTES
            {
                return Err(OpenAiDeviceAuthError::InvalidResponse);
            }
            return exchange_authorization_code(
                transport,
                endpoints,
                &token.authorization_code,
                &token.code_verifier,
                cancel,
            )
            .await;
        }
        if response.status() != StatusCode::FORBIDDEN && response.status() != StatusCode::NOT_FOUND
        {
            return Err(OpenAiDeviceAuthError::Denied);
        }
    }
    Err(OpenAiDeviceAuthError::PollLimit)
}

pub async fn exchange_authorization_code(
    transport: &AuthTransport,
    endpoints: &DeviceEndpoints,
    code: &str,
    verifier: &str,
    cancel: &mut watch::Receiver<bool>,
) -> Result<OpenAiCredential, OpenAiDeviceAuthError> {
    endpoints.validate()?;
    if *cancel.borrow() {
        return Err(OpenAiDeviceAuthError::Cancelled);
    }
    validate_nonempty_bounded(code, MAX_DEVICE_FIELD_BYTES)?;
    validate_nonempty_bounded(verifier, MAX_DEVICE_FIELD_BYTES)?;
    let request = transport
        .client
        .post(&endpoints.oauth_token)
        .header("content-type", "application/x-www-form-urlencoded")
        .header("user-agent", "opencode")
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", endpoints.callback_url.as_str()),
            ("client_id", CLIENT_ID),
            ("code_verifier", verifier),
        ])
        .send();
    parse_token_response(cancellable(cancel, request).await?, cancel).await
}

pub async fn refresh(
    transport: &AuthTransport,
    endpoints: &DeviceEndpoints,
    refresh_token: &str,
    cancel: &mut watch::Receiver<bool>,
) -> Result<OpenAiCredential, OpenAiDeviceAuthError> {
    endpoints.validate()?;
    validate_nonempty_bounded(refresh_token, MAX_TOKEN_BYTES)?;
    let request = transport
        .client
        .post(&endpoints.oauth_token)
        .header("content-type", "application/x-www-form-urlencoded")
        .header("user-agent", "opencode")
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", CLIENT_ID),
        ])
        .send();
    parse_token_response(cancellable(cancel, request).await?, cancel).await
}

async fn cancellable<F>(
    cancel: &mut watch::Receiver<bool>,
    request: F,
) -> Result<Response, OpenAiDeviceAuthError>
where
    F: std::future::Future<Output = Result<Response, reqwest::Error>>,
{
    await_or_cancel(cancel, request)
        .await
        .map_err(|error| error.unwrap_or(OpenAiDeviceAuthError::Request))?
        .map_err(|_| OpenAiDeviceAuthError::Request)
}

async fn await_or_cancel<T, F>(
    cancel: &mut watch::Receiver<bool>,
    future: F,
) -> Result<T, Option<OpenAiDeviceAuthError>>
where
    F: Future<Output = T>,
{
    if *cancel.borrow() {
        return Err(Some(OpenAiDeviceAuthError::Cancelled));
    }
    tokio::pin!(future);
    loop {
        tokio::select! {
            changed = cancel.changed() => {
                if changed.is_err() {
                    return Err(Some(OpenAiDeviceAuthError::Cancelled));
                }
                if *cancel.borrow() {
                    return Err(Some(OpenAiDeviceAuthError::Cancelled));
                }
            }
            value = &mut future => return Ok(value),
        }
    }
}

async fn read_json<T: for<'de> Deserialize<'de>>(
    mut response: Response,
    cancel: &mut watch::Receiver<bool>,
) -> Result<T, OpenAiDeviceAuthError> {
    if *cancel.borrow() {
        return Err(OpenAiDeviceAuthError::Cancelled);
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(OpenAiDeviceAuthError::BodyTooLarge);
    }
    let mut body = Vec::new();
    loop {
        let chunk = {
            let chunk = await_or_cancel(cancel, response.chunk())
                .await
                .map_err(|_| OpenAiDeviceAuthError::Cancelled)?
                .map_err(|_| OpenAiDeviceAuthError::Request)?;
            chunk
        };
        let Some(chunk) = chunk else {
            break;
        };
        if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(OpenAiDeviceAuthError::BodyTooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| OpenAiDeviceAuthError::InvalidResponse)
}

async fn parse_token_response(
    response: Response,
    cancel: &mut watch::Receiver<bool>,
) -> Result<OpenAiCredential, OpenAiDeviceAuthError> {
    if !response.status().is_success() {
        return Err(OpenAiDeviceAuthError::Request);
    }
    let token: TokenResponse = read_json(response, cancel).await?;
    let expires_in = token.expires_in.unwrap_or(3_600);
    if expires_in == 0 || expires_in > MAX_EXPIRES_IN_SECONDS {
        return Err(OpenAiDeviceAuthError::InvalidExpiry);
    }
    let expires_at_ms = now_ms()
        .checked_add(
            expires_in
                .checked_mul(1_000)
                .ok_or(OpenAiDeviceAuthError::InvalidExpiry)?,
        )
        .ok_or(OpenAiDeviceAuthError::InvalidExpiry)?;
    if token.access_token.is_empty()
        || token.refresh_token.is_empty()
        || token.access_token.len() > MAX_TOKEN_BYTES
        || token.refresh_token.len() > MAX_TOKEN_BYTES
        || token.id_token.len() > MAX_TOKEN_BYTES
    {
        return Err(OpenAiDeviceAuthError::InvalidResponse);
    }
    Ok(OpenAiCredential {
        account_id: extract_account_id(&token.id_token)
            .or_else(|| extract_account_id(&token.access_token)),
        access: token.access_token,
        refresh: token.refresh_token,
        expires_at_ms,
    })
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

// This is only an unverified display/routing hint. It is never used to grant
// authorization or choose a credential. Signature verification is performed by
// the provider, not claimed by this metadata extractor.
fn extract_account_id(token: &str) -> Option<String> {
    let mut parts = token.split('.');
    let _header = parts.next()?;
    let payload = parts.next()?;
    let _signature = parts.next()?;
    if parts.next().is_some() || payload.len() > MAX_TOKEN_BYTES {
        return None;
    }
    let bytes = decode_base64url(payload)?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let id = claims
        .get("chatgpt_account_id")
        .and_then(|value| value.as_str())
        .or_else(|| {
            claims
                .get("https://api.openai.com/auth")?
                .get("chatgpt_account_id")?
                .as_str()
        })
        .or_else(|| {
            claims
                .get("organizations")?
                .as_array()?
                .first()?
                .get("id")?
                .as_str()
        })?;
    valid_account_id(id)
}

fn valid_account_id(id: &str) -> Option<String> {
    ((1..=256).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte)))
        .then(|| id.to_owned())
}

fn validate_nonempty_bounded(value: &str, limit: usize) -> Result<(), OpenAiDeviceAuthError> {
    if value.is_empty() || value.len() > limit {
        Err(OpenAiDeviceAuthError::InvalidResponse)
    } else {
        Ok(())
    }
}

fn decode_base64url(input: &str) -> Option<Vec<u8>> {
    let mut bits = 0u32;
    let mut count = 0u8;
    let mut out = Vec::new();
    for byte in input.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return None,
        } as u32;
        bits = (bits << 6) | value;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    Some(out)
}
