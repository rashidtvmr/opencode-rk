//! EXT-010 fallback lane: deferred external-discovery boundary (records only).
//! Pure in-memory, synchronous, non-blocking; no I/O, no threads, no clock.

use thiserror::Error;

/// Maximum deferred discovery events retained before [`DiscoveryBoundary`] overflows.
pub const EXT10_MAX_DEFERRED: usize = 128;

const MAX_PACKAGE_LEN: usize = 128;
const MAX_FILE_LEN: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryRef {
    pub package: Option<String>,
    pub file: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredReason {
    ExternalDiscoveryDeferred,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryEvent {
    pub seq: u64,
    pub scope: u64,
    pub reference: DiscoveryRef,
    pub reason: DeferredReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DiscoveryError {
    #[error("invalid discovery reference")]
    InvalidRef,
    #[error("deferred discovery log full")]
    Overflow,
}

fn package_ok(s: &str) -> bool {
    if s.is_empty() || s.len() > MAX_PACKAGE_LEN {
        return false;
    }
    let mut bytes = s.bytes();
    match bytes.next() {
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

#[derive(Debug, Clone, Default)]
pub struct DiscoveryBoundary {
    events: Vec<DiscoveryEvent>,
    next_seq: u64,
}

impl DiscoveryBoundary {
    pub fn new() -> Self {
        DiscoveryBoundary {
            events: Vec::new(),
            next_seq: 1,
        }
    }

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

    pub fn revoke_scope(&mut self, scope: u64) -> u64 {
        let before = self.events.len();
        self.events.retain(|e| e.scope != scope);
        (before - self.events.len()) as u64
    }

    pub fn list_deferred(&self) -> Vec<DiscoveryEvent> {
        let mut out = self.events.clone();
        out.sort_by_key(|e| e.seq);
        out
    }

    pub fn drain(&mut self, n: usize) -> Vec<DiscoveryEvent> {
        let k = n.min(self.events.len());
        self.events.drain(..k).collect()
    }
}
