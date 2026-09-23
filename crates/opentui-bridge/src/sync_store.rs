#![forbid(unsafe_code)]
//! Location event store (std-only).
//!
//! Evidence (TS checkout a0d9b6c):
//! - `packages/tui/src/context/data.tsx:124-403` `handleEvent(event: V2Event)`
//! - `packages/tui/src/context/data.tsx:126-131` `catalog.updated` ->
//!   model+provider refresh
//! - `packages/tui/src/context/data.tsx:392-393` `reference.updated` ->
//!   reference refresh
//! - `packages/tui/src/context/data.tsx:395-401` `integration.updated` ->
//!   integration+model+provider refresh
//! - `packages/tui/src/context/data.tsx:464-548` `location.{agent,command,
//!   integration,model,provider,reference,skill}` keyed by
//!   `locationKey(directory, workspaceID)` (`data.tsx:50-52`)
//!
//! Divergence: TS matches typed `V2Event.type` (SDK types) and refreshes over
//! the network. std-only here, so `apply` takes the stringly-typed
//! `event.type` + opaque payload, records the location-relevant events, and
//! latches stale until the caller re-refreshes and calls `mark_fresh`.
//! Non-location `session.next.*` events are rejected with `UnknownEvent`
//! (they belong to the message store, `data.tsx:132-391`).

use crate::context_session::{SyncKind, SyncState};

/// Max retained entries; oldest evicted past this (keeps `last_seq` monotonic).
pub const MAX_ENTRIES: usize = 512;
/// Max `event.type` chars.
pub const MAX_EVENT_KIND: usize = 128;
/// Max opaque payload chars.
pub const MAX_PAYLOAD: usize = 4096;

/// Fail-closed `apply` errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncStoreError {
    UnknownEvent,
    EventKindTooLong,
    PayloadTooLong,
}

/// One retained location-relevant event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocationEntry {
    pub seq: u64,
    pub kind: String,
    pub payload: String,
}

/// Bounded log of location-relevant V2 events + `SyncState` stale latch.
#[derive(Debug, Clone)]
pub struct LocationStore {
    entries: Vec<LocationEntry>,
    last_seq: u64,
    sync: SyncState,
}

impl LocationStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            last_seq: 0,
            sync: SyncState::new(SyncKind::Connected),
        }
    }

    /// Location-relevant `V2Event.type` values (`data.tsx:126,392,395`).
    #[must_use]
    pub fn is_known(event_kind: &str) -> bool {
        matches!(
            event_kind,
            "catalog.updated" | "reference.updated" | "integration.updated"
        )
    }

    /// Record event; latches stale (upstream mutated, projection needs
    /// refresh per `data.tsx` refresh pattern). Bounds checked before dispatch.
    pub fn apply(&mut self, event_kind: &str, payload: &str) -> Result<(), SyncStoreError> {
        if event_kind.chars().count() > MAX_EVENT_KIND {
            return Err(SyncStoreError::EventKindTooLong);
        }
        if payload.chars().count() > MAX_PAYLOAD {
            return Err(SyncStoreError::PayloadTooLong);
        }
        if !Self::is_known(event_kind) {
            return Err(SyncStoreError::UnknownEvent);
        }
        if self.entries.len() >= MAX_ENTRIES {
            self.entries.remove(0);
        }
        self.last_seq = self.last_seq.wrapping_add(1);
        self.entries.push(LocationEntry {
            seq: self.last_seq,
            kind: event_kind.to_string(),
            payload: payload.to_string(),
        });
        self.sync.stale = true;
        Ok(())
    }

    #[must_use]
    pub fn entries(&self) -> &[LocationEntry] {
        &self.entries
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub const fn last_seq(&self) -> u64 {
        self.last_seq
    }

    /// Stale latch per `context_session::SyncState` semantics.
    #[must_use]
    pub const fn is_stale(&self) -> bool {
        self.sync.is_stale()
    }

    #[must_use]
    pub const fn kind(&self) -> SyncKind {
        self.sync.kind
    }

    pub fn set_kind(&mut self, kind: SyncKind) {
        self.sync.set_kind(kind);
    }

    pub const fn mark_fresh(&mut self) {
        self.sync.mark_fresh();
    }
}

impl Default for LocationStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_events_apply_and_bump_seq() {
        let mut s = LocationStore::new();
        assert!(!s.is_stale());
        for (i, k) in ["catalog.updated", "reference.updated", "integration.updated"]
            .iter()
            .enumerate()
        {
            assert!(s.apply(k, "{}").is_ok());
            assert_eq!(s.last_seq(), (i + 1) as u64);
        }
        assert_eq!(s.len(), 3);
        assert!(s.is_stale());
        assert_eq!(s.entries()[0].kind, "catalog.updated");
    }

    #[test]
    fn unknown_session_event_rejected_without_side_effects() {
        let mut s = LocationStore::new();
        assert_eq!(
            s.apply("session.next.text.delta", "{}"),
            Err(SyncStoreError::UnknownEvent)
        );
        assert_eq!(s.apply("", "{}"), Err(SyncStoreError::UnknownEvent));
        assert!(s.is_empty());
        assert_eq!(s.last_seq(), 0);
        assert!(!s.is_stale());
    }

    #[test]
    fn event_kind_bound_enforced_first() {
        let mut s = LocationStore::new();
        let long = "k".repeat(MAX_EVENT_KIND + 1);
        assert_eq!(
            s.apply(&long, "{}"),
            Err(SyncStoreError::EventKindTooLong)
        );
        assert!(s.is_empty());
    }

    #[test]
    fn payload_bound_enforced() {
        let mut s = LocationStore::new();
        let long = "v".repeat(MAX_PAYLOAD + 1);
        assert_eq!(
            s.apply("catalog.updated", &long),
            Err(SyncStoreError::PayloadTooLong)
        );
        assert!(s.is_empty());
        assert_eq!(s.last_seq(), 0);
    }

    #[test]
    fn entries_bounded_with_oldest_evicted() {
        let mut s = LocationStore::new();
        for i in 0..(MAX_ENTRIES + 10) {
            s.apply("reference.updated", &format!("p{i}")).unwrap();
        }
        assert_eq!(s.len(), MAX_ENTRIES);
        assert_eq!(s.last_seq(), (MAX_ENTRIES + 10) as u64);
        assert_eq!(s.entries()[0].seq, 11);
        assert_eq!(s.entries()[0].payload, "p10");
    }

    #[test]
    fn stale_latch_follows_context_session_semantics() {
        let mut s = LocationStore::new();
        s.apply("catalog.updated", "{}").unwrap();
        assert!(s.is_stale());
        s.mark_fresh();
        assert!(!s.is_stale());
        s.set_kind(SyncKind::Offline);
        assert!(s.is_stale());
        s.mark_fresh();
        s.set_kind(SyncKind::Reconnecting);
        assert!(!s.is_stale());
        assert_eq!(s.kind(), SyncKind::Reconnecting);
    }
}
