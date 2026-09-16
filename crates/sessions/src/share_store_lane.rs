//! Local secret-free share-metadata lifecycle (SHARE-004, lane fragment).
//!
//! One record per shared session: share id + public URL + last-synced marker.
//! The caller-owned [`ShareSecret`] is borrowed for the call only and is
//! never stored, cloned, or logged. No network, no DB, no clock, no threads,
//! no retained output beyond the bounded record map.
//!
//! Lane-owned module: included by the test via `#[path]`; the integrator
//! wires `pub mod share_store_lane;` into `lib.rs` later. Depends only on
//! contracts id types.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt;

use opencode_rk_contracts::SessionId;
use thiserror::Error;

/// Hard cap on records (one per shared session).
pub const MAX_SHARES: usize = 1024;
/// Maximum `public_url` bytes.
pub const MAX_URL_BYTES: usize = 2048;

/// Opaque share identifier. `Debug` redacts the suffix.
#[derive(Clone, PartialEq, Eq)]
pub struct ShareId(String);

impl ShareId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// Shows a 2-char prefix only; the suffix never appears in logs.
impl fmt::Debug for ShareId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prefix: String = self.0.chars().take(2).collect();
        f.debug_tuple("ShareId")
            .field(&format!("{prefix}***"))
            .finish()
    }
}

/// Caller-owned share secret. Borrowed by [`ShareStore::create`] for the call
/// only; never stored, cloned, or logged. Not `Clone` by construction.
/// Owner calls [`ShareSecret::zeroize`] after the call; `Drop` zeroizes too.
pub struct ShareSecret(Vec<u8>);

impl ShareSecret {
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn zeroize(&mut self) {
        for b in self.0.iter_mut() {
            *b = 0;
        }
    }
}

impl Drop for ShareSecret {
    fn drop(&mut self) {
        self.zeroize();
    }
}

// Carries no bytes, only the type name.
impl fmt::Debug for ShareSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ShareSecret").field(&"***").finish()
    }
}

/// Secret-free share record. No secret field exists by construction.
#[derive(Clone, PartialEq, Eq)]
pub struct ShareMeta {
    pub session: SessionId,
    pub share_id: ShareId,
    pub public_url: String,
    pub last_synced_seq: u64,
}

// Redacts the share-id suffix (via ShareId) and any URL query/fragment.
impl fmt::Debug for ShareMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShareMeta")
            .field("session", &self.session)
            .field("share_id", &self.share_id)
            .field("public_url", &redact_url(&self.public_url))
            .finish_non_exhaustive()
    }
}

/// Typed lifecycle failures. Carry no secret, URL, or id bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum StoreError {
    #[error("session already shared")]
    AlreadyShared,
    #[error("unknown shared session")]
    NotFound,
    #[error("seq regression")]
    SeqRegression,
    #[error("invalid public url")]
    InvalidUrl,
    #[error("share store full")]
    Full,
}

/// Single-owner in-memory share map. Empty store allocates nothing.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct ShareStore {
    // ponytail: BTreeMap over HashMap for deterministic iteration/debug
    // with zero new deps; O(log n), trivial at MAX_SHARES=1024 ceiling.
    records: BTreeMap<SessionId, ShareMeta>,
}

// Counts plus redacted records only; never secret, query, or fragment bytes.
impl fmt::Debug for ShareStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShareStore")
            .field("len", &self.records.len())
            .field("records", &self.records.values().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

impl ShareStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            records: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Record a share for `session`. `secret` is borrowed for the call only
    /// (handed to the transport by the caller) and never retained.
    /// Duplicate session leaves the existing record unchanged.
    pub fn create(
        &mut self,
        session: SessionId,
        share_id: ShareId,
        public_url: &str,
        _secret: &ShareSecret,
    ) -> Result<&ShareMeta, StoreError> {
        if self.records.contains_key(&session) {
            return Err(StoreError::AlreadyShared);
        }
        if !valid_url(public_url) {
            return Err(StoreError::InvalidUrl);
        }
        if self.records.len() >= MAX_SHARES {
            return Err(StoreError::Full);
        }
        self.records.insert(
            session,
            ShareMeta {
                session,
                share_id,
                public_url: public_url.to_owned(),
                last_synced_seq: 0,
            },
        );
        Ok(&self.records[&session])
    }

    #[must_use]
    pub fn get(&self, session: SessionId) -> Option<&ShareMeta> {
        self.records.get(&session)
    }

    /// Advance the last-synced marker. Equal seq is an idempotent re-ack;
    /// only a strict regression is rejected, marker unchanged.
    pub fn mark_synced(&mut self, session: SessionId, seq: u64) -> Result<(), StoreError> {
        match self.records.get_mut(&session) {
            None => Err(StoreError::NotFound),
            Some(meta) if seq < meta.last_synced_seq => Err(StoreError::SeqRegression),
            Some(meta) => {
                meta.last_synced_seq = seq;
                Ok(())
            }
        }
    }

    pub fn remove(&mut self, session: SessionId) -> Result<(), StoreError> {
        match self.records.remove(&session) {
            None => Err(StoreError::NotFound),
            Some(_) => Ok(()),
        }
    }

    /// Session-deletion cascade: drops the share record with the session.
    /// Missing record is a harmless no-op.
    pub fn remove_session(&mut self, session: SessionId) {
        self.records.remove(&session);
    }
}

fn valid_url(url: &str) -> bool {
    if url.is_empty() || url.len() > MAX_URL_BYTES {
        return false;
    }
    if url
        .bytes()
        .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
    {
        return false;
    }
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    !rest[..host_end].is_empty()
}

/// Strip query (`?...`) and fragment (`#...`) for log output.
fn redact_url(url: &str) -> &str {
    let cut = match (url.find('?'), url.find('#')) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) | (None, Some(a)) => Some(a),
        (None, None) => None,
    };
    match cut {
        Some(i) => &url[..i],
        None => url,
    }
}
