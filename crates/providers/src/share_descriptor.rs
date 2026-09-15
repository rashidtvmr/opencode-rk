//! INT-010: pure share-descriptor validation + last-write-wins merge.
//!
//! Caller-supplied descriptors only (`key_ref` is an opaque reference, never
//! secret bytes). No network, no persistence, no env reads, no threads. All
//! hosted partitions are refused by type via [`SyncOp::ProbeHosted`].

use std::collections::BTreeMap;
use std::fmt;

use thiserror::Error;

/// Maximum retained shares (frozen by INT-010-T03).
pub const MAX_SHARES: usize = 256;
/// Maximum `id` length in chars (frozen by INT-010-T03).
pub const MAX_ID_LEN: usize = 64;
/// Maximum `key_ref` length in chars (frozen by INT-010-T03).
pub const MAX_KEYREF_LEN: usize = 64;

/// Caller-supplied share descriptor. `key_ref` is opaque; `digest` is a
/// caller-computed 64-hex payload digest; `seq` is caller-supplied.
#[derive(Clone, Eq, PartialEq)]
pub struct ShareDesc {
    pub id: String,
    pub key_ref: String,
    pub digest: String,
    pub seq: u64,
}

impl fmt::Debug for ShareDesc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShareDesc")
            .field("id", &self.id)
            .field("key_ref", &"<redacted>")
            .field("digest", &"<redacted>")
            .field("seq", &self.seq)
            .finish()
    }
}

/// Last-write-wins projection of one share id.
#[derive(Clone, Eq, PartialEq)]
pub struct ShareView {
    pub id: String,
    pub key_ref: String,
    pub digest: String,
    pub seq: u64,
}

impl fmt::Debug for ShareView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShareView")
            .field("id", &self.id)
            .field("key_ref", &"<redacted>")
            .field("digest", &"<redacted>")
            .field("seq", &self.seq)
            .finish()
    }
}

/// Refused hosted partitions (heterogeneity boundary, INT-010).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostedKind {
    ShareHttp,
    SyncWsR2,
    SupportRelay,
    GithubExchange,
    Deploy,
}

/// Sync operation: seq-gated upsert/remove, or a hosted-partition probe that
/// always fails closed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyncOp {
    Upsert(ShareDesc),
    Remove { id: String, seq: u64 },
    ProbeHosted(HostedKind),
}

/// Typed sync failures. Variants carry no caller material so error rendering
/// never leaks `key_ref`/digest bytes.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum ShareError {
    #[error("invalid share id")]
    InvalidId,
    #[error("invalid secret reference")]
    SecretInvalid,
    #[error("invalid payload digest")]
    InvalidDigest,
    #[error("share table full")]
    Overflow,
    #[error("hosted partition refused")]
    HostedPartition,
}

fn valid_token(value: &str, max_len: usize) -> bool {
    if value.is_empty() || value.len() > max_len {
        return false;
    }
    let mut bytes = value.bytes();
    let first = bytes.next().expect("non-empty checked above");
    if !first.is_ascii_alphanumeric() {
        return false;
    }
    bytes.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Caller-owned in-memory sync view. Synchronous, no threads, no I/O.
#[derive(Clone, Debug, Default)]
pub struct ShareSync {
    shares: BTreeMap<String, ShareView>,
}

impl ShareSync {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate and apply one op. Stale `seq` (`<= stored.seq`) never
    /// mutates; unknown-id `Remove` is `Ok(None)`.
    pub fn apply(&mut self, op: SyncOp) -> Result<Option<ShareView>, ShareError> {
        match op {
            SyncOp::ProbeHosted(_) => Err(ShareError::HostedPartition),
            SyncOp::Upsert(desc) => {
                if !valid_token(&desc.id, MAX_ID_LEN) {
                    return Err(ShareError::InvalidId);
                }
                if !valid_token(&desc.key_ref, MAX_KEYREF_LEN) {
                    return Err(ShareError::SecretInvalid);
                }
                if !valid_digest(&desc.digest) {
                    return Err(ShareError::InvalidDigest);
                }
                match self.shares.get(&desc.id) {
                    Some(stored) if desc.seq <= stored.seq => Ok(Some(stored.clone())),
                    _ => {
                        if !self.shares.contains_key(&desc.id)
                            && self.shares.len() >= MAX_SHARES
                        {
                            return Err(ShareError::Overflow);
                        }
                        // Digest bytes used for the retained copy only; never logged.
                        let view = ShareView {
                            id: desc.id.clone(),
                            key_ref: desc.key_ref.clone(),
                            digest: desc.digest.clone(),
                            seq: desc.seq,
                        };
                        self.shares.insert(view.id.clone(), view.clone());
                        Ok(Some(view))
                    }
                }
            }
            SyncOp::Remove { id, seq } => {
                if !valid_token(&id, MAX_ID_LEN) {
                    return Err(ShareError::InvalidId);
                }
                match self.shares.get(&id) {
                    None => Ok(None),
                    Some(stored) if seq <= stored.seq => Ok(Some(stored.clone())),
                    _ => {
                        self.shares.remove(&id);
                        Ok(None)
                    }
                }
            }
        }
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<ShareView> {
        self.shares.get(id).cloned()
    }

    #[must_use]
    pub fn list(&self) -> Vec<ShareView> {
        self.shares.values().cloned().collect()
    }
}
