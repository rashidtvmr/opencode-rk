//! SHARE-001 deterministic share-snapshot merge + secret validation.
//!
//! Pure: no network, no DB mutation, no clock, no global state, no logging.
//! Caller owns all inputs/outputs; one call-local map dropped on return.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

pub const MAX_BATCHES: usize = 16;
pub const MAX_RECORDS_PER_BATCH: usize = 10_000;
pub const MAX_RECORD_BYTES: usize = 1_048_576;

/// Record discriminant. Declaration order is the canonical sort order:
/// `Message < Model < Part < Session < SessionDiff`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(dead_code)]
pub enum RecordKind {
    Message,
    Model,
    Part,
    Session,
    SessionDiff,
}

/// Opaque share identifier. Never renders its bytes via `Debug`/`Display`.
#[derive(Clone)]
pub struct ShareId(Vec<u8>);

impl ShareId {
    #[must_use]
    pub fn new(id: &str) -> Self {
        Self(id.as_bytes().to_vec())
    }

    fn equals(&self, other: &Self) -> bool {
        ct_eq(&self.0, &other.0)
    }
}

impl PartialEq for ShareId {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other)
    }
}

impl Eq for ShareId {}

impl fmt::Debug for ShareId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShareId").finish_non_exhaustive()
    }
}

/// Opaque share secret. Zeroized on drop; never renders bytes.
#[derive(Clone)]
pub struct ShareSecret(Vec<u8>);

impl ShareSecret {
    #[must_use]
    pub fn from_str(secret: &str) -> Self {
        Self(secret.as_bytes().to_vec())
    }

    fn equals(&self, other: &Self) -> bool {
        ct_eq(&self.0, &other.0)
    }
}

impl Drop for ShareSecret {
    fn drop(&mut self) {
        for b in self.0.iter_mut() {
            *b = 0;
        }
    }
}

impl fmt::Debug for ShareSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShareSecret").finish_non_exhaustive()
    }
}

/// One shareable record. Identity key is `(kind, key)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareRecord {
    pub kind: RecordKind,
    pub key: String,
    pub payload: Vec<u8>,
}

/// Merge result: key-sorted last-write-wins records plus skipped-invalid count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeOutput {
    pub records: Vec<ShareRecord>,
    pub skipped: u32,
}

/// Failure modes. Carries no secret or payload bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShareError {
    NotFound,
    InvalidSecret,
    TooLarge,
    Cancelled,
}

impl fmt::Display for ShareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "unknown share"),
            Self::InvalidSecret => write!(f, "invalid share secret"),
            Self::TooLarge => write!(f, "share batch exceeds size caps"),
            Self::Cancelled => write!(f, "share merge cancelled"),
        }
    }
}

impl std::error::Error for ShareError {}

/// Constant-time byte equality: no early return on length or content.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = (a.len() as u64) ^ (b.len() as u64);
    let n = a.len().max(b.len());
    for i in 0..n {
        let x = if i < a.len() { a[i] } else { 0 };
        let y = if i < b.len() { b[i] } else { 0 };
        diff |= u64::from(x ^ y);
    }
    diff == 0
}

/// Validate a presented secret against the stored one in constant time.
pub fn validate(stored: &ShareSecret, presented: &ShareSecret) -> Result<(), ShareError> {
    if stored.equals(presented) {
        Ok(())
    } else {
        Err(ShareError::InvalidSecret)
    }
}

/// Schema check: non-empty key, non-empty payload within cap, valid JSON.
fn valid_record(record: &ShareRecord) -> bool {
    if record.key.is_empty() || record.payload.is_empty() {
        return false;
    }
    if record.payload.len() > MAX_RECORD_BYTES {
        return false;
    }
    serde_json::from_slice::<serde_json::Value>(&record.payload).is_ok()
}

/// Deterministic merge: caps checked before allocating the working map;
/// last-write-wins per `(kind, key)`; output sorted by `(kind, key)`.
/// `cancel` is checked up front and per batch.
pub fn merge_share_records(
    batches: &[&[ShareRecord]],
    cancel: &AtomicBool,
) -> Result<MergeOutput, ShareError> {
    if cancel.load(Ordering::Relaxed) {
        return Err(ShareError::Cancelled);
    }
    if batches.len() > MAX_BATCHES {
        return Err(ShareError::TooLarge);
    }
    // Cap checks before any working-map allocation.
    for batch in batches {
        if batch.len() > MAX_RECORDS_PER_BATCH {
            return Err(ShareError::TooLarge);
        }
        for record in batch.iter() {
            if record.payload.len() > MAX_RECORD_BYTES {
                return Err(ShareError::TooLarge);
            }
        }
    }
    let mut map: BTreeMap<(RecordKind, &str), &ShareRecord> = BTreeMap::new();
    let mut skipped: u32 = 0;
    for batch in batches {
        if cancel.load(Ordering::Relaxed) {
            return Err(ShareError::Cancelled);
        }
        for record in batch.iter() {
            if !valid_record(record) {
                skipped = skipped.saturating_add(1);
                continue;
            }
            map.insert((record.kind, record.key.as_str()), record);
        }
    }
    Ok(MergeOutput {
        records: map.values().map(|r| (*r).clone()).collect(),
        skipped,
    })
}

/// Validate secret first (unknown id => `NotFound`, mismatch =>
/// `InvalidSecret`), then merge `existing` under `incoming`.
#[allow(clippy::too_many_arguments)]
pub fn apply_sync(
    registry: Option<(&ShareId, &ShareSecret)>,
    target: &ShareId,
    presented: &ShareSecret,
    existing: &[ShareRecord],
    incoming: &[ShareRecord],
    cancel: &AtomicBool,
) -> Result<MergeOutput, ShareError> {
    let Some((known_id, stored)) = registry else {
        return Err(ShareError::NotFound);
    };
    if !known_id.equals(target) {
        return Err(ShareError::NotFound);
    }
    validate(stored, presented)?;
    merge_share_records(&[existing, incoming], cancel)
}
