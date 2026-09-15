//! INT-003 provider-method dispatch boundary.
//!
//! Bounded static table mapping `(provider, method)` pairs to caller-owned
//! handler ids. Dispatch routes to exactly one registered handler and returns
//! an opaque receipt; execution, secrets, env, persistence, network, and
//! events stay with the caller behind the receipt. No I/O, no logging.

use std::collections::BTreeMap;

/// Maximum providers in the bounded static table.
pub const MAX_PROVIDERS: usize = 64;
/// Maximum methods per provider.
pub const MAX_METHODS_PER_PROVIDER: usize = 16;
/// Maximum id bytes for a provider id or method name.
pub const MAX_ID_BYTES: usize = 64;
/// Maximum caller input bytes inspected per dispatch.
pub const MAX_INPUT_BYTES: usize = 4096;

/// Typed dispatch failure.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MethodError {
    /// Provider or method id failed validation (empty, >64 bytes, bad charset).
    #[error("invalid id")]
    InvalidId,
    /// No such provider registered.
    #[error("unknown provider")]
    UnknownProvider,
    /// Provider known, method not registered.
    #[error("unknown method")]
    UnknownMethod,
    /// Bounded table is full; existing entries untouched.
    #[error("method table full")]
    TableFull,
}

/// Which provider-specific method a call targets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderMethodKind {
    /// Generic key method.
    Key,
    /// Generic OAuth method.
    OAuth,
    /// Generic env method.
    Env,
    /// Provider-named method.
    ProviderNamed {
        /// Validated provider id.
        provider: String,
        /// Validated method name.
        method: String,
    },
}

impl ProviderMethodKind {
    /// Build a provider-named kind; both ids validated before construction.
    pub fn named(provider: &str, method: &str) -> Result<Self, MethodError> {
        validate_id(provider)?;
        validate_id(method)?;
        Ok(Self::ProviderNamed {
            provider: provider.to_owned(),
            method: method.to_owned(),
        })
    }
}

/// Borrowed caller input. Opaque handles only; never retained or logged.
pub struct MethodInput<'a> {
    /// Caller bytes, borrowed for the duration of the dispatch call only.
    pub bytes: &'a [u8],
}

impl<'a> MethodInput<'a> {
    /// Borrow caller bytes.
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }
}

impl std::fmt::Debug for MethodInput<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Redact: never render handle bytes.
        f.debug_struct("MethodInput")
            .field("len", &self.bytes.len())
            .finish()
    }
}

impl Clone for MethodInput<'_> {
    fn clone(&self) -> Self {
        Self { bytes: self.bytes }
    }
}

impl Copy for MethodInput<'_> {}

/// Opaque dispatch receipt. Carries no input bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MethodReceipt {
    /// Registered handler id that owns execution.
    pub handler_id: u64,
    /// Whether the handler accepted the call.
    pub accepted: bool,
}

/// Bounded static method table: `(provider, method) -> handler id`.
///
/// `BTreeMap` keeps listings sorted regardless of registration order and
/// allocates nothing until the first registration.
#[derive(Clone, Debug, Default)]
pub struct MethodTable {
    entries: BTreeMap<String, BTreeMap<String, u64>>,
}

impl MethodTable {
    /// Empty table. Allocates nothing until first register.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a handler id. Duplicate `(provider, method)` replaces in
    /// place (length unchanged). Overflow leaves the table untouched.
    pub fn register(
        &mut self,
        provider: &str,
        method: &str,
        handler_id: u64,
    ) -> Result<(), MethodError> {
        validate_id(provider)?;
        validate_id(method)?;
        if let Some(methods) = self.entries.get(provider) {
            if methods.contains_key(method) {
                self.entries
                    .get_mut(provider)
                    .expect("provider presence checked above")
                    .insert(method.to_owned(), handler_id);
                return Ok(());
            }
            if methods.len() >= MAX_METHODS_PER_PROVIDER {
                return Err(MethodError::TableFull);
            }
        } else if self.entries.len() >= MAX_PROVIDERS {
            return Err(MethodError::TableFull);
        }
        self.entries
            .entry(provider.to_owned())
            .or_default()
            .insert(method.to_owned(), handler_id);
        Ok(())
    }

    /// Dispatch to exactly one registered handler. Ids validated first, so
    /// malformed ids report [`MethodError::InvalidId`] even when nothing is
    /// registered. Inspects at most [`MAX_INPUT_BYTES`] caller bytes and
    /// retains nothing. No network, secret storage, env read, or event.
    pub fn dispatch(
        &self,
        provider: &str,
        method: &str,
        input: &MethodInput<'_>,
    ) -> Result<MethodReceipt, MethodError> {
        validate_id(provider)?;
        validate_id(method)?;
        // Bounded inspection: only the first MAX_INPUT_BYTES are ever looked
        // at; nothing is retained after this call returns.
        let _inspected = &input.bytes[..input.bytes.len().min(MAX_INPUT_BYTES)];
        let methods = self
            .entries
            .get(provider)
            .ok_or(MethodError::UnknownProvider)?;
        let handler_id = methods.get(method).ok_or(MethodError::UnknownMethod)?;
        Ok(MethodReceipt {
            handler_id: *handler_id,
            accepted: true,
        })
    }

    /// Sorted provider ids. Registration order-insensitive.
    pub fn providers(&self) -> Vec<String> {
        self.entries.keys().cloned().collect()
    }

    /// Sorted method names for a provider; empty when unknown.
    pub fn list_methods(&self, provider: &str) -> Vec<String> {
        self.entries
            .get(provider)
            .map(|methods| methods.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Total registered `(provider, method)` entries.
    pub fn len(&self) -> usize {
        self.entries.values().map(BTreeMap::len).sum()
    }

    /// True when no entries registered.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Validate a provider id or method name: non-empty, at most 64 bytes,
/// charset `a-z0-9-_` only.
fn validate_id(id: &str) -> Result<(), MethodError> {
    if id.is_empty() || id.len() > MAX_ID_BYTES {
        return Err(MethodError::InvalidId);
    }
    if !id
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
    {
        return Err(MethodError::InvalidId);
    }
    Ok(())
}
