//! Versioned in-memory sync event log with projector replay (SYNC-001 slice).
//!
//! Caller-owned sequencing. No network, no persistence, no threads, no clock.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Maximum stored events per log.
pub const MAX_SYNC_EVENTS: usize = 4096;
/// Maximum registered projectors.
pub const MAX_PROJECTORS: usize = 32;
/// Maximum payload bytes per event.
pub const MAX_PAYLOAD_BYTES: usize = 64 * 1024;

/// Inbound sync event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncEvent {
    pub event_id: String,
    pub event_type: String,
    pub payload: Vec<u8>,
}

/// Sequenced stored event; seq starts at 1, never zero, never reused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequencedEvent {
    pub seq: u64,
    pub event: SyncEvent,
}

/// Sync-log failures; carry variant names only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncError {
    Full,
    InvalidInput,
    UnknownType,
    TooManyProjectors,
    UnknownProjector,
    BadCursor,
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full => f.write_str("sync log is full"),
            Self::InvalidInput => f.write_str("invalid sync input"),
            Self::UnknownType => f.write_str("unknown event type"),
            Self::TooManyProjectors => f.write_str("too many projectors"),
            Self::UnknownProjector => f.write_str("unknown projector"),
            Self::BadCursor => f.write_str("bad replay cursor"),
        }
    }
}

impl std::error::Error for SyncError {}

fn valid_token(value: &str, max: usize) -> bool {
    if value.is_empty() || value.len() > max {
        return false;
    }
    let mut chars = value.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn valid_event(event: &SyncEvent) -> bool {
    valid_token(&event.event_id, 128)
        && valid_token(&event.event_type, 64)
        && event.payload.len() <= MAX_PAYLOAD_BYTES
}

/// Caller-owned versioned sync log.
#[derive(Default)]
pub struct SyncLog {
    events: Vec<SequencedEvent>,
    by_id: HashMap<String, u64>,
    projectors: Vec<(String, Box<dyn Fn(&SequencedEvent) + Send + Sync>)>,
    frozen: bool,
    seen_types: HashMap<String, ()>,
}

impl SyncLog {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of stored events.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    #[must_use]
    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    #[must_use]
    pub fn projector_count(&self) -> usize {
        self.projectors.len()
    }

    #[must_use]
    pub fn events(&self) -> &[SequencedEvent] {
        &self.events
    }

    /// One-way freeze: unknown types rejected afterwards.
    pub fn freeze(&mut self) {
        self.frozen = true;
    }

    /// Register a pure projector sink for one event type.
    pub fn register_projector(
        &mut self,
        event_type: &str,
        projector: impl Fn(&SequencedEvent) + Send + Sync + 'static,
    ) -> Result<(), SyncError> {
        if !valid_token(event_type, 64) {
            return Err(SyncError::InvalidInput);
        }
        if self.projectors.len() >= MAX_PROJECTORS {
            return Err(SyncError::TooManyProjectors);
        }
        self.projectors
            .push((event_type.to_owned(), Box::new(projector)));
        Ok(())
    }

    /// Append one event. Duplicates return the original seq without re-emit.
    pub fn append(&mut self, event: SyncEvent) -> Result<u64, SyncError> {
        if !valid_event(&event) {
            return Err(SyncError::InvalidInput);
        }
        if let Some(seq) = self.by_id.get(&event.event_id) {
            return Ok(*seq);
        }
        if self.frozen && !self.seen_types.contains_key(&event.event_type) {
            return Err(SyncError::UnknownType);
        }
        if self.events.len() >= MAX_SYNC_EVENTS {
            return Err(SyncError::Full);
        }
        let seq = self.events.len() as u64 + 1;
        let stored = SequencedEvent { seq, event };
        self.events.push(stored.clone());
        self.by_id.insert(stored.event.event_id.clone(), seq);
        self.seen_types.insert(stored.event.event_type.clone(), ());
        for (_, projector) in &self.projectors {
            projector(&stored);
        }
        Ok(seq)
    }

    /// Replay stored events with seq >= from_seq into one projector.
    pub fn replay(&self, projector_index: usize, from_seq: u64) -> Result<usize, SyncError> {
        let (_, projector) = self
            .projectors
            .get(projector_index)
            .ok_or(SyncError::UnknownProjector)?;
        if from_seq == 0 || from_seq > self.events.len() as u64 + 1 {
            if self.events.is_empty() || from_seq > self.events.len() as u64 {
                if from_seq == self.events.len() as u64 + 1 && !self.events.is_empty() {
                    return Ok(0);
                }
                return Err(SyncError::BadCursor);
            }
            return Err(SyncError::BadCursor);
        }
        let mut count = 0usize;
        for stored in &self.events {
            if stored.seq >= from_seq {
                projector(stored);
                count += 1;
            }
        }
        if count == 0 && from_seq > self.events.len() as u64 {
            return Err(SyncError::BadCursor);
        }
        Ok(count)
    }
}

impl std::fmt::Debug for SyncLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncLog")
            .field("len", &self.events.len())
            .field("projectors", &self.projectors.len())
            .field("frozen", &self.frozen)
            .finish()
    }
}
