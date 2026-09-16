//! INT-003: provider-specific integration method dispatch boundary.
//!
//! Bounded static method table plus caller-driven dispatch. Registration maps
//! `(provider, method)` pairs to opaque caller-owned handler ids; dispatch
//! routes to exactly one registered handler id and returns an opaque receipt.
//! Dispatch performs no network, secret storage, env read, or event publish;
//! handler execution (if any) is the caller's authority behind the receipt.
//! Method inputs borrow caller bytes only; nothing is retained after dispatch
//! returns. Synchronous, no threads, no I/O.

use std::fmt;

use thiserror::Error;

/// Maximum registered providers per table (frozen by INT-003-T03).
pub const MAX_PROVIDERS: usize = 64;
/// Maximum methods per provider (frozen by INT-003-T03).
pub const MAX_METHODS_PER_PROVIDER: usize = 16;
/// Maximum id length in bytes (frozen by INT-003-T04).
pub const MAX_ID_LEN: usize = 64;
/// Maximum caller input bytes inspected per dispatch (frozen by T05 safety).
pub const MAX_INPUT_BYTES: usize = 4096;

/// Validated provider identifier (non-empty `a-z0-9-_`, max 64 bytes).
pub type ProviderId = String;
/// Validated method name (non-empty `a-z0-9-_`, max 64 bytes).
pub type MethodName = String;
/// Opaque caller-owned handler id returned by dispatch receipt.
pub type HandlerId = u64;

/// Provider method kind: well-known `key`/`oauth`/`env` methods or a
/// provider-named method.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderMethodKind {
    Key,
    OAuth,
    Env,
    ProviderNamed {
        provider: ProviderId,
        method: MethodName,
    },
}

/// Classify a `(provider, method)` pair into its method kind.
///
/// Well-known method names `key`, `oauth`, and `env` map to their variants;
/// any other valid method name maps to [`ProviderMethodKind::ProviderNamed`].
/// Invalid ids yield [`MethodError::InvalidId`] and classify nothing.
pub fn classify(provider: &str, method: &str) -> Result<ProviderMethodKind, MethodError> {
    let provider = checked_id(provider)?;
    let method = checked_id(method)?;
    match method.as_str() {
        "key" => Ok(ProviderMethodKind::Key),
        "oauth" => Ok(ProviderMethodKind::OAuth),
        "env" => Ok(ProviderMethodKind::Env),
        _ => Ok(ProviderMethodKind::ProviderNamed { provider, method }),
    }
}

/// Typed method-table failures. Unknown routes carry no payload so no
/// provider reason or secret material leaks through the error.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum MethodError {
    #[error("unknown provider")]
    UnknownProvider,
    #[error("unknown method")]
    UnknownMethod,
    #[error("invalid provider or method id")]
    InvalidId,
    #[error("method table full")]
    TableFull,
}

/// Borrowed dispatch input. `handle` is an opaque caller-owned reference and
/// `bytes` are caller-owned bytes; both are borrowed for the call only and
/// never retained. Debug redacts both fields.
pub struct MethodInput<'a> {
    pub handle: &'a str,
    pub bytes: &'a [u8],
}

impl fmt::Debug for MethodInput<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MethodInput")
            .field("handle", &"<redacted>")
            .field("bytes", &"<redacted>")
            .finish()
    }
}

/// Opaque dispatch receipt: the single registered handler id that owns
/// execution, plus an acceptance flag. No secret or input bytes carried.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodReceipt {
    pub handler_id: HandlerId,
    pub accepted: bool,
}

fn valid_id(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_ID_LEN {
        return false;
    }
    value
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_'))
}

fn checked_id(value: &str) -> Result<String, MethodError> {
    if valid_id(value) {
        Ok(value.to_owned())
    } else {
        Err(MethodError::InvalidId)
    }
}

#[derive(Clone, Debug)]
struct MethodEntry {
    method: MethodName,
    handler_id: HandlerId,
}

#[derive(Clone, Debug)]
struct ProviderEntry {
    provider: ProviderId,
    methods: Vec<MethodEntry>,
}

/// Bounded caller-owned method table. Synchronous, no threads, no I/O.
/// `Vec::new()` allocates nothing, so a default table holds zero heap
/// allocations until the first successful register.
#[derive(Clone, Debug, Default)]
pub struct MethodTable {
    providers: Vec<ProviderEntry>,
}

impl MethodTable {
    #[must_use]
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register `(provider, method)` to `handler_id`.
    ///
    /// Duplicate pairs replace deterministically in place (length unchanged).
    /// Invalid ids yield [`MethodError::InvalidId`]; a full table yields
    /// [`MethodError::TableFull`]. Both leave the table untouched.
    pub fn register(
        &mut self,
        provider: &str,
        method: &str,
        handler_id: HandlerId,
    ) -> Result<(), MethodError> {
        let provider = checked_id(provider)?;
        let method = checked_id(method)?;
        if let Some(entry) = self.providers.iter_mut().find(|e| e.provider == provider) {
            if let Some(slot) = entry.methods.iter_mut().find(|m| m.method == method) {
                slot.handler_id = handler_id;
                return Ok(());
            }
            if entry.methods.len() >= MAX_METHODS_PER_PROVIDER {
                return Err(MethodError::TableFull);
            }
            entry.methods.push(MethodEntry { method, handler_id });
            return Ok(());
        }
        if self.providers.len() >= MAX_PROVIDERS {
            return Err(MethodError::TableFull);
        }
        self.providers.push(ProviderEntry {
            provider,
            methods: vec![MethodEntry { method, handler_id }],
        });
        Ok(())
    }

    /// Dispatch to exactly one registered handler id.
    ///
    /// Pure lookup: no network, no secret storage, no env read, no event
    /// publish, no handler execution. Input bytes beyond [`MAX_INPUT_BYTES`]
    /// are rejected with [`MethodError::InvalidId`] before any lookup.
    pub fn dispatch(
        &self,
        provider: &str,
        method: &str,
        input: &MethodInput<'_>,
    ) -> Result<MethodReceipt, MethodError> {
        if input.bytes.len() > MAX_INPUT_BYTES || input.handle.len() > MAX_INPUT_BYTES {
            return Err(MethodError::InvalidId);
        }
        if !valid_id(provider) || !valid_id(method) {
            return Err(MethodError::InvalidId);
        }
        let entry = self
            .providers
            .iter()
            .find(|e| e.provider == provider)
            .ok_or(MethodError::UnknownProvider)?;
        let slot = entry
            .methods
            .iter()
            .find(|m| m.method == method)
            .ok_or(MethodError::UnknownMethod)?;
        Ok(MethodReceipt {
            handler_id: slot.handler_id,
            accepted: true,
        })
    }

    /// Sorted provider ids. Order-insensitive: identical registration sets
    /// always produce identical output.
    #[must_use]
    pub fn providers(&self) -> Vec<ProviderId> {
        let mut out: Vec<ProviderId> = self.providers.iter().map(|e| e.provider.clone()).collect();
        out.sort();
        out
    }

    /// Sorted method names for `provider`; empty for unknown providers.
    #[must_use]
    pub fn list_methods(&self, provider: &str) -> Vec<MethodName> {
        let mut out: Vec<MethodName> = self
            .providers
            .iter()
            .find(|e| e.provider == provider)
            .map(|e| e.methods.iter().map(|m| m.method.clone()).collect())
            .unwrap_or_default();
        out.sort();
        out
    }

    /// Total registered `(provider, method)` pairs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.providers.iter().map(|e| e.methods.len()).sum()
    }

    /// True when no pairs are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
