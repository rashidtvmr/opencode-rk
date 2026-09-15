//! SHARE-001 fallback lane: deterministic share-snapshot merge + secret validation.
//!
//! Pure: no network, no DB mutation, no clock, no global state, no logging.
//! Caller owns all inputs/outputs; one call-local map dropped on return.
//! Distinct `Lane*` type names so this file never collides with `share_merge.rs`.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

pub const LANE_MAX_BATCHES: usize = 16;
pub const LANE_MAX_RECORDS_PER_BATCH: usize = 10_000;
pub const LANE_MAX_RECORD_BYTES: usize = 1_048_576;

/// Record discriminant. Declaration order is the canonical sort order:
/// `LaneMessage < LaneModel < LanePart < LaneSession < LaneSessionDiff`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(dead_code)]
pub enum LaneRecordKind {
    LaneMessage,
    LaneModel,
    LanePart,
    LaneSession,
    LaneSessionDiff,
}

/// Opaque share identifier. Never renders its bytes via `Debug`/`Display`.
#[derive(Clone)]
pub struct LaneShareId(Vec<u8>);

impl LaneShareId {
    #[must_use]
    pub fn new(id: &str) -> Self {
        Self(id.as_bytes().to_vec())
    }

    fn equals(&self, other: &Self) -> bool {
        lane_ct_eq(&self.0, &other.0)
    }
}

impl PartialEq for LaneShareId {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other)
    }
}

impl Eq for LaneShareId {}

impl fmt::Debug for LaneShareId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LaneShareId").finish_non_exhaustive()
    }
}

/// Opaque share secret. Zeroized on drop; never renders bytes.
#[derive(Clone)]
pub struct LaneShareSecret(Vec<u8>);

impl LaneShareSecret {
    #[must_use]
    pub fn from_str(secret: &str) -> Self {
        Self(secret.as_bytes().to_vec())
    }

    fn equals(&self, other: &Self) -> bool {
        lane_ct_eq(&self.0, &other.0)
    }
}

impl Drop for LaneShareSecret {
    fn drop(&mut self) {
        for b in self.0.iter_mut() {
            *b = 0;
        }
    }
}

impl fmt::Debug for LaneShareSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LaneShareSecret").finish_non_exhaustive()
    }
}

/// One shareable record. Identity key is `(kind, key)`. `Debug` renders
/// kind + key + payload length only, never payload bytes.
#[derive(Clone, PartialEq, Eq)]
pub struct LaneShareRecord {
    pub kind: LaneRecordKind,
    pub key: String,
    pub payload: Vec<u8>,
}

impl fmt::Debug for LaneShareRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LaneShareRecord")
            .field("kind", &self.kind)
            .field("key", &self.key)
            .field("payload_len", &self.payload.len())
            .finish()
    }
}

/// Merge result: key-sorted last-write-wins records plus skipped-invalid count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneMergeOutput {
    pub records: Vec<LaneShareRecord>,
    pub skipped: u32,
}

/// Failure modes. Carries no secret or payload bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaneShareError {
    NotFound,
    InvalidSecret,
    TooLarge,
    Cancelled,
}

impl fmt::Display for LaneShareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "unknown share"),
            Self::InvalidSecret => write!(f, "invalid share secret"),
            Self::TooLarge => write!(f, "share batch exceeds size caps"),
            Self::Cancelled => write!(f, "share merge cancelled"),
        }
    }
}

impl std::error::Error for LaneShareError {}

/// Constant-time byte equality: no early return on length or content.
fn lane_ct_eq(a: &[u8], b: &[u8]) -> bool {
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
pub fn validate_lane_secret(
    stored: &LaneShareSecret,
    presented: &LaneShareSecret,
) -> Result<(), LaneShareError> {
    if stored.equals(presented) {
        Ok(())
    } else {
        Err(LaneShareError::InvalidSecret)
    }
}

/// Schema check: non-empty key, non-empty payload within cap, valid JSON.
fn valid_lane_record(record: &LaneShareRecord) -> bool {
    if record.key.is_empty() || record.payload.is_empty() {
        return false;
    }
    if record.payload.len() > LANE_MAX_RECORD_BYTES {
        return false;
    }
    serde_json::from_slice::<serde_json::Value>(&record.payload).is_ok()
}

/// Deterministic merge: caps checked before allocating the working map;
/// last-write-wins per `(kind, key)`; output sorted by `(kind, key)`.
/// `cancel` is checked up front and per batch.
pub fn merge_lane_records(
    batches: &[&[LaneShareRecord]],
    cancel: &AtomicBool,
) -> Result<LaneMergeOutput, LaneShareError> {
    if cancel.load(Ordering::Relaxed) {
        return Err(LaneShareError::Cancelled);
    }
    if batches.len() > LANE_MAX_BATCHES {
        return Err(LaneShareError::TooLarge);
    }
    // Cap checks before any working-map allocation.
    for batch in batches {
        if batch.len() > LANE_MAX_RECORDS_PER_BATCH {
            return Err(LaneShareError::TooLarge);
        }
        for record in batch.iter() {
            if record.payload.len() > LANE_MAX_RECORD_BYTES {
                return Err(LaneShareError::TooLarge);
            }
        }
    }
    let mut map: BTreeMap<(LaneRecordKind, &str), &LaneShareRecord> = BTreeMap::new();
    let mut skipped: u32 = 0;
    for batch in batches {
        if cancel.load(Ordering::Relaxed) {
            return Err(LaneShareError::Cancelled);
        }
        for record in batch.iter() {
            if !valid_lane_record(record) {
                skipped = skipped.saturating_add(1);
                continue;
            }
            map.insert((record.kind, record.key.as_str()), record);
        }
    }
    Ok(LaneMergeOutput {
        records: map.values().map(|r| (*r).clone()).collect(),
        skipped,
    })
}

/// Validate secret first (unknown id => `NotFound`, mismatch =>
/// `InvalidSecret`), then merge `existing` under `incoming`.
#[allow(clippy::too_many_arguments)]
pub fn apply_lane_sync(
    registry: Option<(&LaneShareId, &LaneShareSecret)>,
    target: &LaneShareId,
    presented: &LaneShareSecret,
    existing: &[LaneShareRecord],
    incoming: &[LaneShareRecord],
    cancel: &AtomicBool,
) -> Result<LaneMergeOutput, LaneShareError> {
    let Some((known_id, stored)) = registry else {
        return Err(LaneShareError::NotFound);
    };
    if !known_id.equals(target) {
        return Err(LaneShareError::NotFound);
    }
    validate_lane_secret(stored, presented)?;
    merge_lane_records(&[existing, incoming], cancel)
}
