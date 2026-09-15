//! INT-002 lane: caller-driven key/OAuth connection projection.
//!
//! Distinct lane file from `integration_connection.rs` (different slice).
//! Pure in-memory table over caller-supplied material. `code` is borrowed
//! `&str`, never retained. No network, no clock, no threads, no I/O.

use std::fmt;

use thiserror::Error;

/// Maximum retained connections per table (frozen by INT-002).
pub const MAX_CONNECTIONS: usize = 64;
/// Maximum accepted authorization-code length in bytes (frozen by INT-002).
pub const MAX_CODE_LEN: usize = 512;
/// Name/method length cap (INT-001 id rule: `[A-Za-z0-9][A-Za-z0-9._-]*`, 1..=64).
const MAX_NAME_LEN: usize = 64;

/// Method kind answered by the caller-supplied `known` oracle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MethodKind {
    Key,
    OAuth,
}

/// Typed connection outcome. `CodeRequired` is a connect-time verdict only;
/// only `Active` connections are ever recorded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionState {
    Active,
    CodeRequired,
}

/// Monotonic connection id, assigned from 1 in connect order.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ConnId(pub u64);

/// Connect request. All material is borrowed; nothing is retained by the
/// table beyond validated `integration`/`method_id` copies.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ConnectInput<'a> {
    pub integration: &'a str,
    pub method_id: &'a str,
    pub code: Option<&'a str>,
}

impl fmt::Debug for ConnectInput<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectInput")
            .field("integration", &self.integration)
            .field("method_id", &self.method_id)
            .field("code", &self.code.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl fmt::Debug for ConnectionTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectionTable")
            .field("ids", &self.ids())
            .field("len", &self.connections.len())
            .finish()
    }
}

/// Retained connection record. No code/secret/token field by construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionInfo {
    pub id: ConnId,
    pub integration: String,
    pub method_id: String,
    pub state: ConnectionState,
}

/// Typed connect failures. Unknown vs disallowed pairs both normalize to
/// `NotFound` so no provider reason leaks.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ConnectError {
    #[error("connection not found")]
    NotFound,
    #[error("authorization code required")]
    CodeRequired,
    #[error("invalid connection input")]
    InvalidInput,
    #[error("connection table full")]
    Overflow,
}

fn valid_name(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_NAME_LEN {
        return false;
    }
    let mut bytes = value.bytes();
    match bytes.next() {
        Some(first) if first.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    bytes.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn valid_code(code: &str) -> bool {
    if code.is_empty() || code.len() > MAX_CODE_LEN {
        return false;
    }
    code.bytes().all(|b| b.is_ascii_graphic() || b == b' ')
}

/// Caller-owned connection table. Synchronous, no threads, no I/O.
#[derive(Clone, Default)]
pub struct ConnectionTable {
    connections: Vec<ConnectionInfo>,
    next_id: u64,
}

impl ConnectionTable {
    #[must_use]
    pub fn new() -> Self {
        Self {
            connections: Vec::new(),
            next_id: 1,
        }
    }

    /// Project a connect: validate input, ask the caller oracle once, record
    /// `Active` only for key methods or OAuth-with-code.
    pub fn connect(
        &mut self,
        input: &ConnectInput<'_>,
        known: &dyn Fn(&str, &str) -> Option<MethodKind>,
    ) -> Result<ConnectionInfo, ConnectError> {
        // The caller oracle is consulted exactly once per `connect` call,
        // before any validation, so callers can assert no hidden
        // retries/callbacks from the call count. Validation still takes
        // precedence over the oracle verdict; the table is unchanged unless
        // every check passes.
        let kind = known(input.integration, input.method_id);
        if !valid_name(input.integration) || !valid_name(input.method_id) {
            return Err(ConnectError::InvalidInput);
        }
        if let Some(code) = input.code {
            if !valid_code(code) {
                return Err(ConnectError::InvalidInput);
            }
        }
        let kind = kind.ok_or(ConnectError::NotFound)?;
        match kind {
            MethodKind::Key => {}
            MethodKind::OAuth if input.code.is_some() => {}
            MethodKind::OAuth => return Err(ConnectError::CodeRequired),
        }
        if self.connections.len() >= MAX_CONNECTIONS {
            return Err(ConnectError::Overflow);
        }
        // Code bytes used for the verdict above only; never cloned or stored.
        let info = ConnectionInfo {
            id: ConnId(self.next_id),
            integration: input.integration.to_owned(),
            method_id: input.method_id.to_owned(),
            state: ConnectionState::Active,
        };
        self.next_id += 1;
        self.connections.push(info.clone());
        Ok(info)
    }

    #[must_use]
    pub fn get(&self, id: ConnId) -> Option<&ConnectionInfo> {
        self.connections.iter().find(|info| info.id == id)
    }

    #[must_use]
    pub fn list(&self) -> Vec<&ConnectionInfo> {
        let mut out: Vec<&ConnectionInfo> = self.connections.iter().collect();
        out.sort_by_key(|info| info.id.0);
        out
    }

    /// Remove one id. Unknown id returns `false`, table otherwise unchanged.
    pub fn disconnect(&mut self, id: ConnId) -> bool {
        if let Some(index) = self.connections.iter().position(|info| info.id == id) {
            self.connections.remove(index);
            true
        } else {
            false
        }
    }

    fn ids(&self) -> Vec<u64> {
        let mut ids: Vec<u64> = self.connections.iter().map(|info| info.id.0).collect();
        ids.sort_unstable();
        ids
    }
}
