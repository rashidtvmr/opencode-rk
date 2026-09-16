//! INT-006 remote MCP transport/auth boundary (lane-owned).
//!
//! Native one-attachment boundary up to handshake: describe, validate,
//! bounded connect over a caller-provided fake transport, reclaim on close.
//! Handles are opaque ids; raw tokens never cross this boundary. No
//! network, no process spawn, no threads, no persistence, no clock;
//! validation performs zero I/O.
//!
//! Lane-owned module: included by the test via `#[path]`; the integrator
//! wires `pub mod int_mcp_lane;` into `lib.rs` later. No dependency on
//! other crate modules.

#![forbid(unsafe_code)]

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Bounded handshake time in milliseconds.
pub const HANDSHAKE_TIMEOUT_MS: u64 = 10_000;
/// Maximum handshake payload in bytes.
pub const MAX_HANDSHAKE_BYTES: usize = 64 * 1024;
/// Maximum endpoint URL length in bytes.
pub const MAX_URL_BYTES: usize = 2048;
/// Maximum stdio argv entries.
pub const MAX_ARGV: usize = 32;
/// Maximum bytes per argv entry (also applied to the binary path).
pub const MAX_ARG_BYTES: usize = 1024;
/// Maximum opaque auth-handle id bytes.
pub const MAX_HANDLE_BYTES: usize = 128;

/// Transport-side failures for the MCP boundary.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum McpError {
    /// Endpoint rejected before any connection attempt.
    #[error("invalid endpoint")]
    InvalidEndpoint,
    /// Auth handle unknown or missing.
    #[error("auth rejected")]
    AuthRejected,
    /// Handshake exceeded the time budget.
    #[error("handshake timeout")]
    HandshakeTimeout,
    /// Handshake exceeded the byte budget.
    #[error("handshake too large")]
    HandshakeTooLarge,
    /// Caller cancelled before or during connect.
    #[error("cancelled")]
    Cancelled,
    /// Remote closed or reset mid-handshake.
    #[error("transport closed")]
    TransportClosed,
}

/// Remote or local transport descriptor.
#[derive(Clone, Eq, PartialEq)]
pub enum McpTransport {
    /// Server-sent events over HTTPS.
    Sse {
        /// Endpoint URL (must be `https://`, max 2 KiB).
        url: String,
    },
    /// WebSocket over TLS.
    WebSocket {
        /// Endpoint URL (must be `wss://`, max 2 KiB).
        url: String,
    },
    /// Local stdio process descriptor (never spawned here).
    Stdio {
        /// Absolute binary path.
        bin: String,
        /// Argument vector (max 32 entries x 1 KiB).
        argv: Vec<String>,
    },
}

impl fmt::Debug for McpTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sse { .. } => write!(f, "Sse"),
            Self::WebSocket { .. } => write!(f, "WebSocket"),
            Self::Stdio { .. } => write!(f, "Stdio"),
        }
    }
}

/// Authentication mode selector. Handles are opaque 128-B-max ids.
#[derive(Clone, Eq, PartialEq)]
pub enum McpAuth {
    /// No authentication.
    None,
    /// Bearer credential resolved from an opaque handle.
    BearerHandle {
        /// Opaque handle id; raw tokens never cross this boundary.
        handle: String,
    },
    /// OAuth credential resolved from an opaque handle.
    OAuthHandle {
        /// Opaque handle id; raw tokens never cross this boundary.
        handle: String,
    },
}

impl fmt::Debug for McpAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::BearerHandle { .. } => write!(f, "BearerHandle(hdl-***)"),
            Self::OAuthHandle { .. } => write!(f, "OAuthHandle(hdl-***)"),
        }
    }
}

/// Declared auth mode echoed back on the session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthMode {
    /// No authentication.
    None,
    /// Bearer handle auth.
    Bearer,
    /// OAuth handle auth.
    OAuth,
}

/// Transport kind echoed back on the session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportKind {
    /// Server-sent events.
    Sse,
    /// WebSocket.
    WebSocket,
    /// Local stdio.
    Stdio,
}

/// One remote MCP server attachment.
#[derive(Clone, Eq, PartialEq)]
pub struct McpAttachment {
    /// Transport descriptor.
    pub transport: McpTransport,
    /// Auth descriptor.
    pub auth: McpAuth,
}

impl fmt::Debug for McpAttachment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McpAttachment")
            .field("transport", &self.transport)
            .field("auth", &self.auth)
            .finish()
    }
}

/// A single scripted fake-transport step (virtual clock, no wall sleep).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FakeStep {
    /// Deliver payload bytes after `elapsed_ms` of virtual time.
    Bytes {
        /// Payload bytes.
        data: Vec<u8>,
        /// Virtual elapsed milliseconds for this chunk.
        elapsed_ms: u64,
    },
    /// Remote closed or reset mid-handshake.
    Closed,
}

impl FakeStep {
    /// Build a byte-delivery step.
    #[must_use]
    pub fn bytes(data: Vec<u8>, elapsed_ms: u64) -> Self {
        Self::Bytes { data, elapsed_ms }
    }

    /// Remote-close marker used mid-handshake scripts.
    pub const CLOSED: Self = Self::Closed;
}

/// Disposable fixture loopback/fake transport. Never a live socket or a
/// real process; the handshake runs on a virtual clock so bound tests
/// return promptly without wall sleeps.
#[derive(Debug, Default)]
pub struct LoopbackTransport {
    script: Vec<FakeStep>,
    attempts: usize,
    open: Rc<Cell<usize>>,
}

impl LoopbackTransport {
    /// Marker proving tests use the fake transport, never live network.
    pub const FAKE_MARKER: &'static str = "loopback-fake-v1";

    /// Immediate-success fixture.
    #[must_use]
    pub fn ok() -> Self {
        Self::default()
    }

    /// Scripted fixture.
    #[must_use]
    pub fn with_script(script: Vec<FakeStep>) -> Self {
        Self {
            script,
            attempts: 0,
            open: Rc::new(Cell::new(0)),
        }
    }

    /// Always true: this is the fake transport.
    #[must_use]
    pub fn is_loopback(&self) -> bool {
        true
    }

    /// Number of connect attempts so far.
    #[must_use]
    pub fn connect_attempts(&self) -> usize {
        self.attempts
    }

    /// Number of currently open handles.
    #[must_use]
    pub fn open_handles(&self) -> usize {
        self.open.get()
    }
}

/// Established MCP session: label only, no retained body. Owns exactly
/// one transport handle; `close` consumes the session and reclaims it,
/// and `Drop` reclaims if not explicitly closed.
pub struct McpSession {
    /// Redacted endpoint label (no query secrets, no argv).
    pub endpoint_label: String,
    /// Transport kind.
    pub transport: TransportKind,
    /// Declared auth mode.
    pub auth_mode: AuthMode,
    handle: Option<Rc<Cell<usize>>>,
}

impl fmt::Debug for McpSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McpSession")
            .field("endpoint_label", &self.endpoint_label)
            .field("transport", &self.transport)
            .field("auth_mode", &self.auth_mode)
            .field("closed", &self.is_closed())
            .finish()
    }
}

impl McpSession {
    /// Whether the session has been closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.handle.is_none()
    }

    /// Consume the session and reclaim its transport handle.
    pub fn close(mut self) {
        self.reclaim();
    }

    fn reclaim(&mut self) {
        if let Some(open) = self.handle.take() {
            open.set(open.get().saturating_sub(1));
        }
    }
}

impl Drop for McpSession {
    fn drop(&mut self) {
        self.reclaim();
    }
}

fn valid_url(url: &str, scheme: &str) -> bool {
    if url.is_empty() || url.len() > MAX_URL_BYTES {
        return false;
    }
    let Some(rest) = url.strip_prefix(scheme) else {
        return false;
    };
    if rest.is_empty() {
        return false;
    }
    !rest.bytes().any(|b| b <= b' ' || b == 0x7f)
}

fn valid_handle(handle: &str) -> bool {
    !handle.is_empty() && handle.len() <= MAX_HANDLE_BYTES && !handle.contains('\0')
}

impl McpAttachment {
    /// Scheme/allowlist, length, and arg-count checks. Performs zero I/O.
    pub fn validate(&self) -> Result<(), McpError> {
        match &self.transport {
            McpTransport::Sse { url } => {
                if !valid_url(url, "https://") {
                    return Err(McpError::InvalidEndpoint);
                }
            }
            McpTransport::WebSocket { url } => {
                if !valid_url(url, "wss://") {
                    return Err(McpError::InvalidEndpoint);
                }
            }
            McpTransport::Stdio { bin, argv } => {
                if bin.is_empty()
                    || bin.len() > MAX_ARG_BYTES
                    || !bin.starts_with('/')
                    || bin.contains('\0')
                {
                    return Err(McpError::InvalidEndpoint);
                }
                if argv.len() > MAX_ARGV {
                    return Err(McpError::InvalidEndpoint);
                }
                if argv
                    .iter()
                    .any(|a| a.len() > MAX_ARG_BYTES || a.contains('\0'))
                {
                    return Err(McpError::InvalidEndpoint);
                }
            }
        }
        match &self.auth {
            McpAuth::None => Ok(()),
            McpAuth::BearerHandle { handle } | McpAuth::OAuthHandle { handle } => {
                if valid_handle(handle) {
                    Ok(())
                } else {
                    Err(McpError::AuthRejected)
                }
            }
        }
    }

    /// Redacted endpoint label: scheme + authority + path only, no query
    /// string or fragment; for stdio, the binary path (never argv).
    #[must_use]
    pub fn endpoint_label(&self) -> String {
        match &self.transport {
            McpTransport::Sse { url } | McpTransport::WebSocket { url } => {
                let cut = url.find(['?', '#']).unwrap_or(url.len());
                url[..cut].to_owned()
            }
            McpTransport::Stdio { bin, .. } => bin.clone(),
        }
    }

    /// Redacted one-line description safe for logs: kind, auth mode, and
    /// label only, plus the fixed `hdl-***` marker. Never carries handle
    /// bytes, argv, or URL query secrets.
    #[must_use]
    pub fn safe_describe(&self) -> String {
        let (transport, auth_mode) = match (&self.transport, &self.auth) {
            (McpTransport::Sse { .. }, McpAuth::None) => ("Sse", AuthMode::None),
            (McpTransport::Sse { .. }, McpAuth::BearerHandle { .. }) => ("Sse", AuthMode::Bearer),
            (McpTransport::Sse { .. }, McpAuth::OAuthHandle { .. }) => ("Sse", AuthMode::OAuth),
            (McpTransport::WebSocket { .. }, McpAuth::None) => ("WebSocket", AuthMode::None),
            (McpTransport::WebSocket { .. }, McpAuth::BearerHandle { .. }) => {
                ("WebSocket", AuthMode::Bearer)
            }
            (McpTransport::WebSocket { .. }, McpAuth::OAuthHandle { .. }) => {
                ("WebSocket", AuthMode::OAuth)
            }
            (McpTransport::Stdio { .. }, McpAuth::None) => ("Stdio", AuthMode::None),
            (McpTransport::Stdio { .. }, McpAuth::BearerHandle { .. }) => {
                ("Stdio", AuthMode::Bearer)
            }
            (McpTransport::Stdio { .. }, McpAuth::OAuthHandle { .. }) => ("Stdio", AuthMode::OAuth),
        };
        format!(
            "mcp transport={transport} auth={auth_mode:?} endpoint={} handle=hdl-***",
            self.endpoint_label()
        )
    }

    fn declared(&self) -> (TransportKind, AuthMode) {
        let transport = match &self.transport {
            McpTransport::Sse { .. } => TransportKind::Sse,
            McpTransport::WebSocket { .. } => TransportKind::WebSocket,
            McpTransport::Stdio { .. } => TransportKind::Stdio,
        };
        let auth_mode = match &self.auth {
            McpAuth::None => AuthMode::None,
            McpAuth::BearerHandle { .. } => AuthMode::Bearer,
            McpAuth::OAuthHandle { .. } => AuthMode::OAuth,
        };
        (transport, auth_mode)
    }

    /// Bounded connect/handshake over the caller-provided fake transport.
    ///
    /// Order: pre-set cancel, validation, auth-handle resolution, then one
    /// handle open and a virtual-clock handshake capped at
    /// [`HANDSHAKE_TIMEOUT_MS`] and [`MAX_HANDSHAKE_BYTES`]. Every failure
    /// path reclaims the handle and drops partial bytes.
    pub fn connect(
        &self,
        transport: &mut LoopbackTransport,
        known_handles: &[&str],
        cancel: &AtomicBool,
    ) -> Result<McpSession, McpError> {
        if cancel.load(Ordering::SeqCst) {
            return Err(McpError::Cancelled);
        }
        self.validate()?;
        match &self.auth {
            McpAuth::None => {}
            McpAuth::BearerHandle { handle } | McpAuth::OAuthHandle { handle } => {
                if !known_handles.iter().any(|k| *k == handle) {
                    return Err(McpError::AuthRejected);
                }
            }
        }
        transport.attempts += 1;
        transport.open.set(transport.open.get() + 1);
        let guard = transport.open.clone();
        let fail = |open: &Rc<Cell<usize>>, err: McpError| -> McpError {
            open.set(open.get().saturating_sub(1));
            err
        };

        let mut bytes: usize = 0;
        let mut elapsed_ms: u64 = 0;
        let steps = transport.script.len();
        for index in 0..steps {
            if cancel.load(Ordering::SeqCst) {
                return Err(fail(&guard, McpError::Cancelled));
            }
            let step = transport.script[index].clone();
            match step {
                FakeStep::Bytes {
                    data,
                    elapsed_ms: dt,
                } => {
                    bytes = bytes.saturating_add(data.len());
                    if bytes > MAX_HANDSHAKE_BYTES {
                        return Err(fail(&guard, McpError::HandshakeTooLarge));
                    }
                    elapsed_ms = elapsed_ms.saturating_add(dt);
                    if elapsed_ms > HANDSHAKE_TIMEOUT_MS {
                        return Err(fail(&guard, McpError::HandshakeTimeout));
                    }
                }
                FakeStep::Closed => {
                    return Err(fail(&guard, McpError::TransportClosed));
                }
            }
        }

        let (kind, auth_mode) = self.declared();
        Ok(McpSession {
            endpoint_label: self.endpoint_label(),
            transport: kind,
            auth_mode,
            handle: Some(guard),
        })
    }
}
