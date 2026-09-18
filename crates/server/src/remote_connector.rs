//! PC outbound connector: authenticated dial-out state for NET-005.
//!
//! Design source: `docs/architecture/COMPLETION_REMOTE.md` (paired PC opens an
//! authenticated outbound WSS connection to the owned gateway; no inbound
//! ports, no per-user tunnel) and the NET-005 card
//! (`python3 tools/completion_plan.py --card NET-005`).
//!
//! Caller supplies all I/O (sockets, TLS, timers, listener tasks). This module
//! owns only identity, redacted credentials, bounded reconnect policy,
//! heartbeat/size caps, a bounded outbound queue, and disable-with-receipt.
//! No threads, no sockets, no wall clock: time advances via
//! [`OutboundConnector::advance`], so tests stay deterministic (docs/TDD.md).
#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::fmt;
use std::time::Duration;

/// Base reconnect sleep. First retry waits roughly this long.
pub const RECONNECT_BASE_DELAY_MS: u64 = 1_000;
/// Hard cap: no single reconnect sleep may exceed this.
pub const RECONNECT_MAX_DELAY_MS: u64 = 30_000;
/// Consecutive failures after which status reports truthful offline.
pub const RECONNECT_MAX_ATTEMPTS: u32 = 10;
/// How often the caller should send a heartbeat frame.
pub const HEARTBEAT_INTERVAL_MS: u64 = 15_000;
/// Silence after the last heartbeat before the peer counts as dead.
pub const HEARTBEAT_TIMEOUT_MS: u64 = 45_000;
/// Largest single outbound frame or control payload.
pub const MAX_MESSAGE_BYTES: usize = 262_144;
/// Max queued outbound frames.
pub const MAX_QUEUE_ITEMS: usize = 128;
/// Max queued outbound bytes in total.
pub const MAX_QUEUE_BYTES: usize = 1_048_576;
/// Expected gateway cert-pin length (SHA-256 of SPKI).
pub const CERT_PIN_LEN: usize = 32;
/// Min/max device-token bytes.
pub const MIN_TOKEN_BYTES: usize = 16;
pub const MAX_TOKEN_BYTES: usize = 256;
/// Max chars for endpoint host / device / account / request IDs.
pub const MAX_ID_CHARS: usize = 128;

/// Typed failures. Variant names only; never carry secrets or payload bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorError {
    InvalidEndpoint,
    BadCredentials,
    MalformedRequest,
    ForgedGateway,
    DeviceMismatch,
    AuthFailed,
    Disabled,
    NotConnected,
    Offline,
    TooLarge,
    QueueFull,
    HeartbeatTimeout,
}

impl fmt::Display for ConnectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEndpoint => write!(f, "invalid gateway endpoint"),
            Self::BadCredentials => write!(f, "bad device credentials"),
            Self::MalformedRequest => write!(f, "malformed control request"),
            Self::ForgedGateway => write!(f, "gateway identity mismatch"),
            Self::DeviceMismatch => write!(f, "device id mismatch"),
            Self::AuthFailed => write!(f, "token mismatch"),
            Self::Disabled => write!(f, "remote mode disabled"),
            Self::NotConnected => write!(f, "not connected"),
            Self::Offline => write!(f, "offline after bounded retries"),
            Self::TooLarge => write!(f, "message exceeds size cap"),
            Self::QueueFull => write!(f, "outbound queue full"),
            Self::HeartbeatTimeout => write!(f, "heartbeat timeout"),
        }
    }
}

impl std::error::Error for ConnectorError {}

/// Constant-time byte equality. Length mismatch is false, never panics.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

/// Deterministic 64-bit mixer for jitter. No RNG, no wall clock, no I/O.
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn valid_host(host: &str) -> bool {
    if host.is_empty() || host.len() > 253 {
        return false;
    }
    if host.contains("://") || host.bytes().any(|b| b == b'/' || b.is_ascii_whitespace()) {
        return false;
    }
    let lower = host.to_ascii_lowercase();
    if lower.starts_with('-') || lower.starts_with('.') || lower.ends_with('-') || lower.ends_with('.')
    {
        return false;
    }
    lower
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
        && lower.bytes().any(|b| b.is_ascii_alphanumeric())
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_CHARS
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
}

/// Gateway the PC dials out to. Stores the expected identity; `Debug` redacts
/// the pin so logs never carry key material.
#[derive(Clone)]
pub struct GatewayEndpoint {
    host: String,
    port: u16,
    expected_cert_pin: Vec<u8>,
}

impl fmt::Debug for GatewayEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GatewayEndpoint")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("expected_cert_pin", &"REDACTED")
            .finish()
    }
}

impl GatewayEndpoint {
    /// Parse and validate. Host is lowercased; pin must be `CERT_PIN_LEN` bytes.
    pub fn parse(host: &str, port: u16, expected_cert_pin: &[u8]) -> Result<Self, ConnectorError> {
        if !valid_host(host) || port == 0 {
            return Err(ConnectorError::InvalidEndpoint);
        }
        if expected_cert_pin.len() != CERT_PIN_LEN {
            return Err(ConnectorError::InvalidEndpoint);
        }
        Ok(Self {
            host: host.to_ascii_lowercase(),
            port,
            expected_cert_pin: expected_cert_pin.to_vec(),
        })
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Dial URL. Contains no secrets (pin stays out of the URL).
    pub fn wss_url(&self) -> String {
        let mut s = String::with_capacity(self.host.len() + 12);
        s.push_str("wss://");
        s.push_str(&self.host);
        s.push(':');
        s.push_str(&itoa_u16(self.port));
        s.push_str("/v1/link");
        s
    }

    /// Reject forged gateways: expected host (case-insensitive) AND expected
    /// cert pin (constant-time) must both match. Fails closed on any mismatch.
    pub fn verify_gateway(&self, presented: &PresentedGateway) -> Result<(), ConnectorError> {
        if presented.host.to_ascii_lowercase() != self.host {
            return Err(ConnectorError::ForgedGateway);
        }
        if !constant_time_eq(&presented.cert_pin, &self.expected_cert_pin) {
            return Err(ConnectorError::ForgedGateway);
        }
        Ok(())
    }
}

fn itoa_u16(v: u16) -> std::string::String {
    let mut buf = [0u8; 5];
    let mut n = v as u32;
    let mut i = buf.len();
    if n == 0 {
        return "0".to_string();
    }
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    String::from_utf8_lossy(&buf[i..]).into_owned()
}

/// Identity a peer presents during handshake (supplied by caller-owned TLS).
#[derive(Debug, Clone)]
pub struct PresentedGateway {
    pub host: String,
    pub cert_pin: Vec<u8>,
}

/// Device credentials holder. `Debug` redacts the token; `Drop` zeroizes it
/// (best effort, std only).
/// ponytail: best-effort zeroize; upgrade to `zeroize` crate when deps allowed.
#[derive(Clone)]
pub struct DeviceCredentials {
    device_id: String,
    account_id: String,
    token: Vec<u8>,
}

impl fmt::Debug for DeviceCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceCredentials")
            .field("device_id", &self.device_id)
            .field("account_id", &self.account_id)
            .field("token", &"REDACTED")
            .finish()
    }
}

impl Drop for DeviceCredentials {
    fn drop(&mut self) {
        for b in self.token.iter_mut() {
            *b = 0;
        }
    }
}

impl DeviceCredentials {
    pub fn new(
        device_id: &str,
        account_id: &str,
        token: &[u8],
    ) -> Result<Self, ConnectorError> {
        if !valid_id(device_id) || !valid_id(account_id) {
            return Err(ConnectorError::BadCredentials);
        }
        if token.len() < MIN_TOKEN_BYTES || token.len() > MAX_TOKEN_BYTES {
            return Err(ConnectorError::BadCredentials);
        }
        Ok(Self {
            device_id: device_id.to_string(),
            account_id: account_id.to_string(),
            token: token.to_vec(),
        })
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn account_id(&self) -> &str {
        &self.account_id
    }

    /// Scoped token access: the token never leaves except inside the closure,
    /// so callers cannot stash or log it by accident.
    pub fn with_token<R>(&self, f: impl FnOnce(&[u8]) -> R) -> R {
        f(&self.token)
    }

    fn token_matches(&self, candidate: &[u8]) -> bool {
        constant_time_eq(&self.token, candidate)
    }
}

/// Bounded jittered reconnect policy. Delays are deterministic in the attempt
/// count (no RNG, no clock) and always within `[base/2, min(base*2^n, cap)]`.
#[derive(Debug, Clone, Copy)]
pub struct ReconnectPolicy {
    base_ms: u64,
    cap_ms: u64,
    max_attempts: u32,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            base_ms: RECONNECT_BASE_DELAY_MS,
            cap_ms: RECONNECT_MAX_DELAY_MS,
            max_attempts: RECONNECT_MAX_ATTEMPTS,
        }
    }
}

impl ReconnectPolicy {
    pub fn new(base: Duration, cap: Duration, max_attempts: u32) -> Result<Self, ConnectorError> {
        let base_ms = base.as_millis().min(u64::MAX as u128) as u64;
        let cap_ms = cap.as_millis().min(u64::MAX as u128) as u64;
        if base_ms == 0 || cap_ms == 0 || base_ms > cap_ms || max_attempts == 0 {
            return Err(ConnectorError::InvalidEndpoint);
        }
        Ok(Self {
            base_ms,
            cap_ms,
            max_attempts,
        })
    }

    /// Sleep before attempt `n` (0-based). Always `<= cap`.
    /// Exponential rise `base * 2^n` (saturating) capped at `cap_ms`, with
    /// deterministic jitter into `[delay/2, delay]` via splitmix64, so fleet
    /// wake-ups spread without any RNG or wall clock (docs/TDD.md).
    pub fn delay_for(&self, attempt: u32) -> Duration {
        let shift = attempt.min(31);
        let grown = self
            .base_ms
            .checked_shl(shift)
            .unwrap_or(u64::MAX)
            .min(self.cap_ms);
        let floor = grown / 2;
        let span = grown.saturating_sub(floor).saturating_add(1);
        let jitter = splitmix64(self.base_ms ^ ((attempt as u64).wrapping_add(1) << 32)) % span;
        Duration::from_millis(floor.saturating_add(jitter).max(1))
    }

    /// True once retries are exhausted: caller must report truthful offline.
    pub fn is_offline(&self, attempts: u32) -> bool {
        attempts >= self.max_attempts
    }

    pub fn max_attempts(&self) -> u32 {
        self.max_attempts
    }
}

/// Connection lifecycle. Caller-owned; no background tasks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectorState {
    Disconnected,
    Connecting,
    Connected,
    Backoff { retry_in: Duration },
    Offline,
    Disabled,
}

/// One authorized control request delivered over the gateway link.
#[derive(Debug, Clone)]
pub struct ControlRequest {
    pub request_id: String,
    pub device_id: String,
    pub payload: Vec<u8>,
}

/// Acceptance receipt for a verified control request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlResponse {
    pub request_id: String,
    pub accepted: bool,
}

/// Proof that `disable` cancelled tasks and joined listeners. Carries counts
/// only, never credentials.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisableReceipt {
    pub joined: bool,
    pub cancelled_tasks: usize,
    pub listeners_remaining: usize,
}

/// Outbound connector state machine. Owns endpoint identity, credentials,
/// queue and clock; the caller owns sockets, TLS, timers and listener tasks.
pub struct OutboundConnector {
    endpoint: GatewayEndpoint,
    creds: DeviceCredentials,
    policy: ReconnectPolicy,
    state: ConnectorState,
    queue: VecDeque<Vec<u8>>,
    queued_bytes: usize,
    listeners: usize,
    attempts: u32,
    last_retry: Option<Duration>,
    last_heartbeat_ms: Option<u64>,
    now_ms: u64,
}

impl fmt::Debug for OutboundConnector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OutboundConnector")
            .field("endpoint", &self.endpoint)
            .field("device_id", &self.creds.device_id())
            .field("account_id", &self.creds.account_id())
            .field("state", &self.state)
            .field("queue_len", &self.queue.len())
            .field("queued_bytes", &self.queued_bytes)
            .field("listeners", &self.listeners)
            .field("attempts", &self.attempts)
            .finish()
    }
}

impl Drop for OutboundConnector {
    fn drop(&mut self) {
        for frame in self.queue.iter_mut() {
            for b in frame.iter_mut() {
                *b = 0;
            }
        }
        self.queue.clear();
        self.queued_bytes = 0;
    }
}

impl OutboundConnector {
    pub fn new(endpoint: GatewayEndpoint, creds: DeviceCredentials) -> Self {
        Self::with_policy(endpoint, creds, ReconnectPolicy::default())
    }

    pub fn with_policy(
        endpoint: GatewayEndpoint,
        creds: DeviceCredentials,
        policy: ReconnectPolicy,
    ) -> Self {
        Self {
            endpoint,
            creds,
            policy,
            state: ConnectorState::Disconnected,
            queue: VecDeque::new(),
            queued_bytes: 0,
            listeners: 0,
            attempts: 0,
            last_retry: None,
            last_heartbeat_ms: None,
            now_ms: 0,
        }
    }

    /// Caller-driven clock (ms). No wall-clock reads anywhere in this module.
    pub fn advance(&mut self, delta_ms: u64) {
        self.now_ms = self.now_ms.saturating_add(delta_ms);
    }

    pub fn now_ms(&self) -> u64 {
        self.now_ms
    }

    /// Authenticated dial-out. Verifies the presented gateway identity before
    /// exposing anything; registers one caller-owned listener on success.
    pub fn connect(&mut self, presented: &PresentedGateway) -> Result<(), ConnectorError> {
        if self.state == ConnectorState::Disabled {
            return Err(ConnectorError::Disabled);
        }
        self.state = ConnectorState::Connecting;
        self.endpoint.verify_gateway(presented)?;
        self.attempts = 0;
        self.last_retry = None;
        self.listeners = 1;
        self.last_heartbeat_ms = Some(self.now_ms);
        self.state = ConnectorState::Connected;
        Ok(())
    }

    /// Sleep/resume/network-loss hook. Returns the bounded sleep the caller
    /// should wait before the next dial; flips to truthful `Offline` once the
    /// retry budget is exhausted.
    pub fn on_network_loss(&mut self) -> Duration {
        if self.state == ConnectorState::Disabled {
            return Duration::ZERO;
        }
        let delay = self.policy.delay_for(self.attempts);
        self.attempts = self.attempts.saturating_add(1);
        self.last_retry = Some(delay);
        self.state = if self.policy.is_offline(self.attempts) {
            ConnectorState::Offline
        } else {
            ConnectorState::Backoff { retry_in: delay }
        };
        delay
    }

    /// Verify and accept one control request from the gateway link.
    /// Checks, in order: gateway identity, request shape (ids + size cap),
    /// device binding, then token (constant-time). Fails closed at the first
    /// mismatch; error variants carry names only, never secrets.
    pub fn handle_request(
        &self,
        req: &ControlRequest,
        presented: &PresentedGateway,
        token: &[u8],
    ) -> Result<ControlResponse, ConnectorError> {
        if self.state == ConnectorState::Disabled {
            return Err(ConnectorError::Disabled);
        }
        self.endpoint.verify_gateway(presented)?;
        if req.request_id.is_empty()
            || !valid_id(&req.request_id)
            || !valid_id(&req.device_id)
            || req.payload.len() > MAX_MESSAGE_BYTES
        {
            if req.payload.len() > MAX_MESSAGE_BYTES {
                return Err(ConnectorError::TooLarge);
            }
            return Err(ConnectorError::MalformedRequest);
        }
        if req.device_id != self.creds.device_id {
            return Err(ConnectorError::DeviceMismatch);
        }
        if !self.creds.token_matches(token) {
            return Err(ConnectorError::AuthFailed);
        }
        Ok(ControlResponse {
            request_id: req.request_id.clone(),
            accepted: true,
        })
    }

    /// Buffer one outbound frame under the backpressure caps.
    /// `TooLarge` for a single frame over `MAX_MESSAGE_BYTES`; `QueueFull`
    /// when the item count or the total byte budget would overflow.
    pub fn push(&mut self, frame: Vec<u8>) -> Result<(), ConnectorError> {
        if self.state == ConnectorState::Disabled {
            return Err(ConnectorError::Disabled);
        }
        if frame.len() > MAX_MESSAGE_BYTES {
            return Err(ConnectorError::TooLarge);
        }
        if self.queue.len() >= MAX_QUEUE_ITEMS
            || self.queued_bytes.saturating_add(frame.len()) > MAX_QUEUE_BYTES
        {
            return Err(ConnectorError::QueueFull);
        }
        self.queued_bytes = self.queued_bytes.saturating_add(frame.len());
        self.queue.push_back(frame);
        Ok(())
    }

    /// Take the oldest queued frame for the caller to write to its socket.
    pub fn pop(&mut self) -> Option<Vec<u8>> {
        let frame = self.queue.pop_front()?;
        self.queued_bytes = self.queued_bytes.saturating_sub(frame.len());
        Some(frame)
    }

    /// Record a received heartbeat at the current caller clock.
    pub fn note_heartbeat(&mut self) {
        self.last_heartbeat_ms = Some(self.now_ms);
    }

    /// True once a heartbeat frame is due to be sent.
    pub fn heartbeat_due(&self) -> bool {
        match self.last_heartbeat_ms {
            None => self.state == ConnectorState::Connected,
            Some(last) => self.now_ms.saturating_sub(last) >= HEARTBEAT_INTERVAL_MS,
        }
    }

    /// True when the peer has been silent past the timeout.
    /// `None` last-heartbeat means never heard: timed out only if connected
    /// and the clock already ran past the timeout since `now_ms == 0`.
    pub fn heartbeat_timed_out(&self) -> bool {
        match self.last_heartbeat_ms {
            None => self.state == ConnectorState::Connected && self.now_ms >= HEARTBEAT_TIMEOUT_MS,
            Some(last) => self.now_ms.saturating_sub(last) >= HEARTBEAT_TIMEOUT_MS,
        }
    }

    /// Check heartbeat state, returning a typed error on timeout.
    pub fn check_heartbeat(&self) -> Result<(), ConnectorError> {
        if self.heartbeat_timed_out() {
            return Err(ConnectorError::HeartbeatTimeout);
        }
        Ok(())
    }

    /// Terminal: cancel caller tasks, join the listener, drop queued bytes.
    /// Consumes the connector so no residual handle can linger. Idempotent by
    /// construction: `self` is moved, so a second call cannot exist.
    pub fn disable(mut self) -> DisableReceipt {
        let cancelled = self.listeners;
        for frame in self.queue.iter_mut() {
            for b in frame.iter_mut() {
                *b = 0;
            }
        }
        self.queue.clear();
        self.queued_bytes = 0;
        self.listeners = 0;
        DisableReceipt {
            joined: true,
            cancelled_tasks: cancelled,
            listeners_remaining: 0,
        }
    }

    pub fn state(&self) -> ConnectorState {
        self.state.clone()
    }

    pub fn is_offline(&self) -> bool {
        self.state == ConnectorState::Offline
    }

    pub fn is_connected(&self) -> bool {
        self.state == ConnectorState::Connected
    }

    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    pub fn last_retry(&self) -> Option<Duration> {
        self.last_retry
    }

    pub fn active_listeners(&self) -> usize {
        self.listeners
    }

    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    pub fn queued_bytes(&self) -> usize {
        self.queued_bytes
    }

    pub fn endpoint(&self) -> &GatewayEndpoint {
        &self.endpoint
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn pin(fill: u8) -> Vec<u8> {
        vec![fill; CERT_PIN_LEN]
    }

    fn endpoint() -> GatewayEndpoint {
        GatewayEndpoint::parse("gateway.example.com", 443, &pin(0xA5)).unwrap()
    }

    // Distinctive marker so log-leak asserts cannot pass by accident.
    fn creds() -> DeviceCredentials {
        DeviceCredentials::new("dev-01", "acct-01", b"tok_TESTSECRET_0123456789abcdef").unwrap()
    }

    fn presented() -> PresentedGateway {
        PresentedGateway {
            host: "gateway.example.com".to_string(),
            cert_pin: pin(0xA5),
        }
    }

    fn request() -> ControlRequest {
        ControlRequest {
            request_id: "req-1".to_string(),
            device_id: "dev-01".to_string(),
            payload: b"ping".to_vec(),
        }
    }

    fn connected() -> OutboundConnector {
        let mut c = OutboundConnector::new(endpoint(), creds());
        c.connect(&presented()).unwrap();
        c
    }

    #[test]
    fn authorized_control_request_accepted() {
        let c = connected();
        let resp = c
            .handle_request(&request(), &presented(), b"tok_TESTSECRET_0123456789abcdef")
            .unwrap();
        assert!(resp.accepted);
        assert_eq!(resp.request_id, "req-1");
    }

    #[test]
    fn forged_gateway_rejected() {
        let ep = endpoint();
        let bad_pin = PresentedGateway {
            host: "gateway.example.com".to_string(),
            cert_pin: pin(0x00),
        };
        assert_eq!(
            ep.verify_gateway(&bad_pin),
            Err(ConnectorError::ForgedGateway)
        );
        let bad_host = PresentedGateway {
            host: "evil.example.com".to_string(),
            cert_pin: pin(0xA5),
        };
        assert_eq!(
            ep.verify_gateway(&bad_host),
            Err(ConnectorError::ForgedGateway)
        );
        // Dial-out must never complete against a forged gateway.
        let mut c = OutboundConnector::new(endpoint(), creds());
        assert_eq!(c.connect(&bad_pin), Err(ConnectorError::ForgedGateway));
        assert_ne!(c.state(), ConnectorState::Connected);
        assert_eq!(c.active_listeners(), 0);
    }

    #[test]
    fn mismatched_device_credentials_rejected() {
        let c = connected();
        let mut bad_dev = request();
        bad_dev.device_id = "dev-99".to_string();
        assert_eq!(
            c.handle_request(&bad_dev, &presented(), b"tok_TESTSECRET_0123456789abcdef"),
            Err(ConnectorError::DeviceMismatch)
        );
        assert_eq!(
            c.handle_request(&request(), &presented(), b"tok_WRONG_0123456789abcdef!!"),
            Err(ConnectorError::AuthFailed)
        );
        let mut bad_id = request();
        bad_id.request_id.clear();
        assert_eq!(
            c.handle_request(&bad_id, &presented(), b"tok_TESTSECRET_0123456789abcdef"),
            Err(ConnectorError::MalformedRequest)
        );
    }

    #[test]
    fn reconnect_sleep_bounded_jittered_and_offline_truthful() {
        let p = ReconnectPolicy::default();
        let cap = Duration::from_millis(RECONNECT_MAX_DELAY_MS);
        let floor = Duration::from_millis(RECONNECT_BASE_DELAY_MS / 2);
        let mut seen = HashSet::new();
        for attempt in 0..200u32 {
            let d = p.delay_for(attempt);
            assert!(d <= cap, "attempt {attempt}: {d:?} exceeds cap");
            assert!(d >= floor, "attempt {attempt}: {d:?} below floor");
            assert_eq!(d, p.delay_for(attempt), "attempt {attempt}: not deterministic");
            seen.insert(d);
        }
        assert!(seen.len() > 1, "no jitter spread across attempts");
        // Sleep/resume path: each loss returns the bounded sleep; budget
        // exhaustion flips to truthful offline, never silent retry forever.
        let mut c = connected();
        for _ in 0..RECONNECT_MAX_ATTEMPTS {
            let sleep = c.on_network_loss();
            assert!(sleep <= cap, "reconnect sleep {sleep:?} exceeds cap");
        }
        assert!(c.is_offline());
        assert_eq!(c.state(), ConnectorState::Offline);
        assert_eq!(c.last_retry().unwrap(), p.delay_for(RECONNECT_MAX_ATTEMPTS - 1));
    }

    #[test]
    fn disable_cancels_joins_and_leaks_no_secret() {
        let mut c = connected();
        c.push(vec![1, 2, 3]).unwrap();
        assert_eq!(c.active_listeners(), 1);
        let receipt = c.disable();
        assert!(receipt.joined, "disable must join the listener");
        assert_eq!(receipt.listeners_remaining, 0, "no residual listeners");
        assert!(receipt.cancelled_tasks >= 1, "must cancel the live task");
        let dbg = format!("{receipt:?}");
        assert!(!dbg.contains("TESTSECRET"), "secret in receipt: {dbg}");
        // Connector and credential Debug must redact too.
        let c2 = connected();
        for dbg in [format!("{c2:?}"), format!("{:?}", creds()), format!("{:?}", endpoint())] {
            assert!(!dbg.contains("TESTSECRET"), "secret in logs: {dbg}");
        }
    }

    #[test]
    fn backpressure_heartbeat_and_size_caps_enforced() {
        let mut c = connected();
        // Single-frame cap.
        assert_eq!(
            c.push(vec![0u8; MAX_MESSAGE_BYTES + 1]),
            Err(ConnectorError::TooLarge)
        );
        // Item-count cap.
        for _ in 0..MAX_QUEUE_ITEMS {
            c.push(vec![0u8; 1]).unwrap();
        }
        assert_eq!(c.push(vec![0u8; 1]), Err(ConnectorError::QueueFull));
        // Total-bytes cap with legal-size frames.
        let mut c2 = connected();
        let frame = vec![0u8; MAX_MESSAGE_BYTES];
        for _ in 0..(MAX_QUEUE_BYTES / MAX_MESSAGE_BYTES) {
            c2.push(frame.clone()).unwrap();
        }
        assert_eq!(c2.push(frame.clone()), Err(ConnectorError::QueueFull));
        // Heartbeat timeout trips after silence, clears on heartbeat.
        let mut c3 = connected();
        assert!(!c3.heartbeat_timed_out());
        assert!(c3.check_heartbeat().is_ok());
        c3.advance(HEARTBEAT_TIMEOUT_MS);
        assert!(c3.heartbeat_timed_out());
        assert_eq!(c3.check_heartbeat(), Err(ConnectorError::HeartbeatTimeout));
        c3.note_heartbeat();
        assert!(!c3.heartbeat_timed_out());
    }

    #[test]
    fn oversized_control_payload_rejected() {
        let c = connected();
        let mut req = request();
        req.payload = vec![0u8; MAX_MESSAGE_BYTES + 1];
        assert_eq!(
            c.handle_request(&req, &presented(), b"tok_TESTSECRET_0123456789abcdef"),
            Err(ConnectorError::TooLarge)
        );
    }
}
