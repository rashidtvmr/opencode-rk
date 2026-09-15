//! Production web entry probe: pure fragment supporting WEB-006 T03.
//!
//! Resolves the client-accessible launch contract (production web entry
//! discoverable from daemon state) without spawning listeners, dialing the
//! network, touching the filesystem, or depending on Vite/second server.
//! Side-effect-free: `no_std`-style pure logic over a caller-supplied
//! endpoint snapshot (main agent wires `daemon.rs` descriptors to it).

/// Canonical production web entry path served by the singleton daemon
/// alongside `/api/*` (see `web_assets.rs` SPA fallback).
pub const WEB_ENTRY_PATH: &str = "/";

/// Upper bound on accepted `http_origin` byte length (DoS/retention cap).
pub const MAX_HTTP_ORIGIN_LEN: usize = 64;

/// Minimal caller-supplied snapshot of a healthy daemon endpoint.
/// Mirrors `daemon::BackendDescriptor` without owning it (frozen elsewhere).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonEndpoint {
    pub pid: u32,
    pub http_origin: String,
    pub schema_version: u16,
}

/// Resolved production web entry: origin plus entry path and full URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebEntry {
    pub origin: String,
    pub path: &'static str,
    pub url: String,
}

impl WebEntry {
    /// Canonical entry path (always [`WEB_ENTRY_PATH`]).
    #[must_use]
    pub fn entry_path() -> &'static str {
        WEB_ENTRY_PATH
    }
}

/// Explicit probe failures: typed, keyboard-independent launch contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebEntryError {
    /// No daemon metadata: caller must launch via the named command.
    MissingDescriptor,
    /// Descriptor owner is gone or zero: recoverable, never shadowed.
    StaleEndpoint,
    /// Wire schema drift: expected (daemon runtime) vs actual (descriptor).
    SchemaMismatch { expected: u16, actual: u16 },
    /// Origin is not an exact `http://127.0.0.1:<port>` loopback origin.
    InvalidOrigin,
    /// Origin exceeds [`MAX_HTTP_ORIGIN_LEN`] bytes.
    OriginTooLong { max: usize, actual: usize },
}

impl core::fmt::Display for WebEntryError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MissingDescriptor => write!(
                f,
                "no daemon descriptor: start the singleton with `opencode-rk web` (no Vite, no second server)"
            ),
            Self::StaleEndpoint => write!(
                f,
                "stale daemon endpoint: owner is gone; restart with `opencode-rk web` to recover deterministically"
            ),
            Self::SchemaMismatch { expected, actual } => write!(
                f,
                "daemon schema mismatch: expected {expected}, got {actual}; restart with `opencode-rk web`"
            ),
            Self::InvalidOrigin => write!(
                f,
                "invalid daemon origin: expected exactly `http://127.0.0.1:<port>` loopback origin"
            ),
            Self::OriginTooLong { max, actual } => {
                write!(f, "daemon origin too long: max {max} bytes, got {actual}")
            }
        }
    }
}

impl std::error::Error for WebEntryError {}

fn valid_loopback_origin(origin: &str) -> bool {
    const PREFIX: &str = "http://127.0.0.1:";
    let Some(port) = origin.strip_prefix(PREFIX) else {
        return false;
    };
    if port.is_empty() || port.len() > 5 {
        return false;
    }
    if !port.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    if port.len() > 1 && port.starts_with('0') {
        return false;
    }
    match port.parse::<u16>() {
        Ok(port) => port != 0,
        Err(_) => false,
    }
}

/// Resolve the production web entry from a daemon endpoint snapshot.
///
/// Pure and side-effect-free: no listener spawn, no socket dial, no
/// filesystem access, no clock, no Vite. `expected_schema` is the daemon
/// runtime wire version; `pid_alive` reports whether `pid` still owns the
/// singleton (caller injects the OS check so tests stay deterministic).
///
/// ponytail: fixed `u16` schema + `bool` liveness params, no traits/generics;
/// upgrade to a DaemonState trait only when a second caller needs it.
pub fn resolve_web_entry(
    endpoint: Option<&DaemonEndpoint>,
    expected_schema: u16,
) -> Result<WebEntry, WebEntryError> {
    resolve_web_entry_with_liveness(endpoint, expected_schema, &|pid| pid != 0)
}

/// Same as [`resolve_web_entry`] with caller-injected liveness (tests/fakes).
pub fn resolve_web_entry_with_liveness(
    endpoint: Option<&DaemonEndpoint>,
    expected_schema: u16,
    pid_alive: &dyn Fn(u32) -> bool,
) -> Result<WebEntry, WebEntryError> {
    let endpoint = endpoint.ok_or(WebEntryError::MissingDescriptor)?;
    if endpoint.schema_version != expected_schema {
        return Err(WebEntryError::SchemaMismatch {
            expected: expected_schema,
            actual: endpoint.schema_version,
        });
    }
    if endpoint.pid == 0 || !pid_alive(endpoint.pid) {
        return Err(WebEntryError::StaleEndpoint);
    }
    if endpoint.http_origin.len() > MAX_HTTP_ORIGIN_LEN {
        return Err(WebEntryError::OriginTooLong {
            max: MAX_HTTP_ORIGIN_LEN,
            actual: endpoint.http_origin.len(),
        });
    }
    if !valid_loopback_origin(&endpoint.http_origin) {
        return Err(WebEntryError::InvalidOrigin);
    }
    Ok(WebEntry {
        origin: endpoint.http_origin.clone(),
        path: WEB_ENTRY_PATH,
        url: {
            let mut url = String::with_capacity(endpoint.http_origin.len() + 1);
            url.push_str(&endpoint.http_origin);
            url.push_str(WEB_ENTRY_PATH);
            url
        },
    })
}
