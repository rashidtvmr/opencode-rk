//! EXT-010 deferred external-discovery boundary: bounded, auditable record of
//! configured package/file discovery intents with zero filesystem traversal,
//! zero npm resolution/install, zero dynamic import, zero external activation.
//!
//! Pure in-memory, synchronous, non-blocking; no I/O, no threads, no clock.

use thiserror::Error;

/// Maximum deferred discovery events retained before [`DiscoveryBoundary`] overflows.
pub const EXT10_MAX_DEFERRED: usize = 128;

/// Maximum package reference length in bytes (ASCII-only, so bytes == chars).
const MAX_PACKAGE_LEN: usize = 128;

/// Maximum file reference length in bytes (ASCII-only, so bytes == chars).
const MAX_FILE_LEN: usize = 256;

/// A configured external discovery intent: exactly one of `package`/`file`
/// must be `Some`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryRef {
    pub package: Option<String>,
    pub file: Option<String>,
}

/// Why an event is deferred instead of executed here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredReason {
    /// Configured external discovery recorded; loading stays design work upstream.
    ExternalDiscoveryDeferred,
}

/// One bounded deferred-discovery record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryEvent {
    /// Monotonic sequence, starts at 1, never reused after [`DiscoveryBoundary::drain`].
    pub seq: u64,
    /// Caller-owned scope id; only used for [`DiscoveryBoundary::revoke_scope`].
    pub scope: u64,
    pub reference: DiscoveryRef,
    pub reason: DeferredReason,
}

/// Deferred-discovery boundary failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DiscoveryError {
    /// Reference shape rejected (arity, charset, length, traversal); state unchanged.
    #[error("invalid discovery reference")]
    InvalidRef,
    /// Event vec (`len == EXT10_MAX_DEFERRED`) full; state unchanged.
    #[error("deferred discovery log full")]
    Overflow,
}

fn package_ok(s: &str) -> bool {
    if s.is_empty() || s.len() > MAX_PACKAGE_LEN {
        return false;
    }
    let mut bytes = s.bytes();
    match bytes.next() {
        // Scoped npm names (`@scope/b`) lead with `@`; otherwise alphanumeric.
        Some(b) if b.is_ascii_alphanumeric() || b == b'@' => {}
        _ => return false,
    }
    bytes.all(|b| {
        b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'@' || b == b'/' || b == b'-'
    })
}

fn file_ok(s: &str) -> bool {
    if s.is_empty() || s.len() > MAX_FILE_LEN {
        return false;
    }
    let mut bytes = s.bytes();
    match bytes.next() {
        Some(b) if b.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    if !bytes.all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'/' || b == b'-')
    {
        return false;
    }
    // Relative only: no leading `/` (covered by first-char check, kept
    // explicit), no empty segment (no `//`, no trailing `/`), no `..` segment.
    if s.starts_with('/') {
        return false;
    }
    s.split('/').all(|seg| !seg.is_empty() && seg != "..")
}

fn ref_ok(r: &DiscoveryRef) -> bool {
    match (&r.package, &r.file) {
        (Some(p), None) => package_ok(p),
        (None, Some(f)) => file_ok(f),
        _ => false,
    }
}

/// Bounded record of deferred external-discovery intent.
///
/// Caller-owned lifetime; all methods synchronous and constant-time.
/// Touches no registry, filesystem, network, or host.
#[derive(Debug, Clone, Default)]
pub struct DiscoveryBoundary {
    events: Vec<DiscoveryEvent>,
    next_seq: u64,
}

impl DiscoveryBoundary {
    /// Empty boundary: no allocation beyond an empty vec, no I/O, no threads.
    pub fn new() -> Self {
        DiscoveryBoundary {
            events: Vec::new(),
            next_seq: 1,
        }
    }

    /// Validate shape, record as `ExternalDiscoveryDeferred`, return its `seq`.
    /// Never touches any registry, filesystem, network, or host.
    pub fn declare(&mut self, scope: u64, reference: DiscoveryRef) -> Result<u64, DiscoveryError> {
        if !ref_ok(&reference) {
            return Err(DiscoveryError::InvalidRef);
        }
        if self.events.len() >= EXT10_MAX_DEFERRED {
            return Err(DiscoveryError::Overflow);
        }
        let seq = self.next_seq;
        self.next_seq += 1;
        self.events.push(DiscoveryEvent {
            seq,
            scope,
            reference,
            reason: DeferredReason::ExternalDiscoveryDeferred,
        });
        Ok(seq)
    }

    /// Remove only that scope's events, return the count removed.
    /// Unknown scope => `0`, log otherwise unchanged.
    pub fn revoke_scope(&mut self, scope: u64) -> u64 {
        let before = self.events.len();
        self.events.retain(|e| e.scope != scope);
        (before - self.events.len()) as u64
    }

    /// Pending events sorted by `seq` ascending.
    pub fn list_deferred(&self) -> Vec<DiscoveryEvent> {
        let mut out = self.events.clone();
        out.sort_by_key(|e| e.seq);
        out
    }

    /// Remove and return the oldest `n` events (or fewer).
    /// `drain(0)` is a no-op; `drain(n > len)` empties; never panics.
    /// Sequence numbers are never reused.
    pub fn drain(&mut self, n: usize) -> Vec<DiscoveryEvent> {
        let k = n.min(self.events.len());
        self.events.drain(..k).collect()
    }
}
