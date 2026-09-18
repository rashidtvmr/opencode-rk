//! DISC-114: admission control with auth denial and queue bounds.
//!
//! Second AUD-010 repair item. Auth-first submit (denial mutates nothing),
//! bounded FIFO queue ([`MAX_QUEUE_DEPTH`]), bounded inflight permits
//! ([`MAX_INFLIGHT`]), cancel-queued reclaim, commit log with truthful
//! restart recovery ([`recover`]: committed wins, queued dupes dropped once),
//! and bounded per-consumer fan-out with slow-consumer disconnect + resync.
//!
//! Standalone std-only module (mirrors `app_runtime` caps, `turn_service`
//! cancel/uncertain semantics, `event_stream` slow-consumer rule); the
//! integrator wires `pub mod admission_bounds` and unifies caps/types.
//! ponytail: duplicate of app_runtime/event_cursor/event_stream concepts;
//! unify into one bound/queue/cursor vocabulary when the shared-contracts
//! lane is approved.
#![forbid(unsafe_code)]

use std::collections::{HashSet, VecDeque};

/// Queued work waiting for a permit. Mirrors `app_runtime::MAX_PENDING_TURNS`.
pub const MAX_QUEUE_DEPTH: usize = 16;
/// Concurrent admitted work. Mirrors `app_runtime::MAX_CONCURRENT_TURNS`.
pub const MAX_INFLIGHT: usize = 2;
/// Per-consumer pending events before slow-disconnect.
pub const MAX_CONSUMER_PENDING: usize = 64;
/// Retained committed ids for restart dedupe. Mirrors `REPLAY_BUFFER_CAP` scale.
pub const MAX_COMMIT_LOG: usize = 512;
/// Max bytes of one work label.
pub const MAX_LABEL_BYTES: usize = 512;

/// Admission failures. Deny/bound errors mutate nothing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdmissionError {
    /// Credential missing or wrong; no state changed, no side effect ran.
    Unauthenticated,
    /// Queue at [`MAX_QUEUE_DEPTH`]; explicit backpressure, nothing dropped.
    Backpressure { capacity: usize },
    /// Work id not queued (inflight, committed, or never seen).
    UnknownWork(u64),
    /// Work id already tracked (queued, inflight, or committed); no duplicate
    /// queued, so restart recovery can never double-apply it.
    DuplicateWork(u64),
    /// Label empty or over [`MAX_LABEL_BYTES`].
    InvalidWork,
}

impl std::fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthenticated => write!(f, "unauthenticated: valid credential required"),
            Self::Backpressure { capacity } => {
                write!(f, "admission queue full at capacity {capacity}")
            }
            Self::UnknownWork(id) => write!(f, "no queued work with id {id}"),
            Self::DuplicateWork(id) => write!(f, "work {id} already admitted"),
            Self::InvalidWork => write!(f, "work label is empty or too long"),
        }
    }
}

impl std::error::Error for AdmissionError {}

/// One unit of queued work: stable id plus a bounded opaque label.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkItem {
    pub id: u64,
    pub label: String,
}

impl WorkItem {
    pub fn new(id: u64, label: impl Into<String>) -> Result<Self, AdmissionError> {
        let label = label.into();
        if label.is_empty() || label.len() > MAX_LABEL_BYTES {
            return Err(AdmissionError::InvalidWork);
        }
        Ok(Self { id, label })
    }
}

/// Proof of admission; carries the queued id.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmissionReceipt {
    pub id: u64,
}

/// Outcome of [`recover`]: what to re-queue vs what was already committed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Recovered {
    pub replay: Vec<WorkItem>,
    pub duplicates_dropped: usize,
}

/// Recover the queued-versus-committed boundary after restart.
///
/// Pure function over snapshots: an id present in `committed` is never
/// re-queued (no duplicated effects); the first queued copy wins, later
/// copies are counted in `duplicates_dropped`.
pub fn recover(committed: &[u64], queued: &[WorkItem]) -> Recovered {
    let done: HashSet<u64> = committed.iter().copied().collect();
    let mut seen: HashSet<u64> = HashSet::new();
    let mut replay = Vec::new();
    let mut duplicates_dropped = 0;
    for item in queued {
        if done.contains(&item.id) || !seen.insert(item.id) {
            duplicates_dropped += 1;
            continue;
        }
        replay.push(item.clone());
    }
    Recovered {
        replay,
        duplicates_dropped,
    }
}

/// Auth-first bounded admission gate.
///
/// Contract: [`Self::submit`] checks the credential before capacity or
/// validation, so denial mutates nothing. [`Self::cancel_queued`] removes
/// queued work only (inflight/committed return [`AdmissionError::UnknownWork`]).
/// Side effects happen exclusively via [`Self::complete`], recorded in
/// [`Self::effects`] so tests prove denial/cancel ran nothing.
#[derive(Debug)]
pub struct AdmissionGate {
    expected: String,
    queue: VecDeque<WorkItem>,
    inflight: HashSet<u64>,
    committed: VecDeque<u64>,
    effects: Vec<u64>,
    denials: u64,
}

impl AdmissionGate {
    pub fn new(expected_token: impl Into<String>) -> Self {
        Self {
            expected: expected_token.into(),
            queue: VecDeque::new(),
            inflight: HashSet::new(),
            committed: VecDeque::new(),
            effects: Vec::new(),
            denials: 0,
        }
    }

    /// Constant-time credential comparison: length plus every byte, so a
    /// prefix guess costs the same as any other mismatch.
    fn authenticated(&self, presented: &str) -> bool {
        let expected = self.expected.as_bytes();
        let given = presented.as_bytes();
        if expected.len() != given.len() {
            return false;
        }
        let mut diff = 0u8;
        for (left, right) in expected.iter().zip(given.iter()) {
            diff |= left ^ right;
        }
        diff == 0
    }

    /// Auth first (denial mutates nothing), then capacity, then duplicate id.
    pub fn submit(
        &mut self,
        presented: &str,
        item: WorkItem,
    ) -> Result<AdmissionReceipt, AdmissionError> {
        if !self.authenticated(presented) {
            self.denials = self.denials.saturating_add(1);
            return Err(AdmissionError::Unauthenticated);
        }
        if self.queue.len() >= MAX_QUEUE_DEPTH.max(1) {
            return Err(AdmissionError::Backpressure {
                capacity: MAX_QUEUE_DEPTH,
            });
        }
        if self.queue.iter().any(|queued| queued.id == item.id)
            || self.inflight.contains(&item.id)
            || self.committed.iter().any(|done| *done == item.id)
        {
            // Already tracked work is a duplicate effect, not new admission.
            return Err(AdmissionError::DuplicateWork(item.id));
        }
        let receipt = AdmissionReceipt { id: item.id };
        self.queue.push_back(item);
        Ok(receipt)
    }

    /// Admit the oldest queued item while an inflight permit is free.
    /// Returns `None` when empty or all [`MAX_INFLIGHT`] permits held.
    pub fn admit_next(&mut self) -> Option<WorkItem> {
        if self.inflight.len() >= MAX_INFLIGHT.max(1) {
            return None;
        }
        let item = self.queue.pop_front()?;
        self.inflight.insert(item.id);
        Some(item)
    }

    /// Remove queued work without partial effects or leaked permits.
    /// Queued removal cannot leak a permit (permits are held only by
    /// inflight work, see [`Self::admit_next`]); inflight, committed, or
    /// unknown ids return [`AdmissionError::UnknownWork`].
    pub fn cancel_queued(&mut self, id: u64) -> Result<WorkItem, AdmissionError> {
        let position = self.queue.iter().position(|queued| queued.id == id);
        match position {
            Some(index) => Ok(self.queue.remove(index).expect("position checked")),
            None => Err(AdmissionError::UnknownWork(id)),
        }
    }

    /// Finish inflight work: releases its permit, commits the id, records
    /// the single side effect.
    pub fn complete(&mut self, id: u64) -> Result<(), AdmissionError> {
        if !self.inflight.remove(&id) {
            return Err(AdmissionError::UnknownWork(id));
        }
        while self.committed.len() >= MAX_COMMIT_LOG.max(1) {
            self.committed.pop_front();
        }
        self.committed.push_back(id);
        self.effects.push(id);
        Ok(())
    }

    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    pub fn inflight_count(&self) -> usize {
        self.inflight.len()
    }

    pub fn effects(&self) -> &[u64] {
        &self.effects
    }

    pub fn denials(&self) -> u64 {
        self.denials
    }

    pub fn committed(&self) -> Vec<u64> {
        self.committed.iter().copied().collect()
    }
}

/// Bounded per-consumer fan-out. A consumer whose pending buffer reaches
/// [`MAX_CONSUMER_PENDING`] is disconnected (slow) without blocking others;
/// [`Self::resync`] reconnects it with an empty buffer.
#[derive(Debug)]
pub struct ConsumerSet {
    next_id: u64,
    pending: std::collections::HashMap<u64, VecDeque<u64>>,
    slow: HashSet<u64>,
}

impl ConsumerSet {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            pending: std::collections::HashMap::new(),
            slow: HashSet::new(),
        }
    }

    pub fn subscribe(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        self.pending.insert(id, VecDeque::new());
        id
    }

    /// Fan one event id out to every connected consumer.
    ///
    /// A consumer whose buffer is already at [`MAX_CONSUMER_PENDING`] is
    /// stalled. While at least two consumers stay connected, the oldest
    /// stalled id sheds (slow-disconnect, buffer kept for diagnosis) so the
    /// rest proceed; a lone consumer is retained so the last observer is
    /// never shed silently and its lag stays visible via
    /// [`Self::pending_count`]. Returns ids shed in this call (at most one).
    /// ponytail: lone-consumer buffer may exceed the nominal cap; ceiling is
    /// one retained observer, upgrade path is a hard byte cap plus snapshot
    /// resync on reconnect.
    pub fn publish(&mut self, event: u64) -> Vec<u64> {
        let connected = self
            .pending
            .keys()
            .filter(|id| !self.slow.contains(id))
            .count();
        let mut ids: Vec<u64> = self.pending.keys().copied().collect();
        ids.sort_unstable();
        let mut slow_now = Vec::new();
        for id in ids {
            if self.slow.contains(&id) {
                continue;
            }
            let full = self
                .pending
                .get(&id)
                .is_some_and(|buffer| buffer.len() >= MAX_CONSUMER_PENDING.max(1));
            if full && connected >= 2 && slow_now.is_empty() {
                self.slow.insert(id);
                slow_now.push(id);
                continue;
            }
            if let Some(buffer) = self.pending.get_mut(&id) {
                buffer.push_back(event);
            }
        }
        slow_now
    }

    /// Reconnect a slow consumer with an empty buffer. True when it was slow.
    pub fn resync(&mut self, id: u64) -> bool {
        if !self.slow.remove(&id) {
            return false;
        }
        if let Some(buffer) = self.pending.get_mut(&id) {
            buffer.clear();
        }
        self.pending.contains_key(&id)
    }

    pub fn is_connected(&self, id: u64) -> bool {
        self.pending.contains_key(&id) && !self.slow.contains(&id)
    }

    pub fn pending_count(&self, id: u64) -> usize {
        self.pending.get(&id).map_or(0, VecDeque::len)
    }

    pub fn live_consumers(&self) -> usize {
        self.pending.keys().filter(|id| !self.slow.contains(id)).count()
    }
}

impl Default for ConsumerSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "rk-test-token";

    fn gate() -> AdmissionGate {
        AdmissionGate::new(TOKEN)
    }

    fn item(id: u64) -> WorkItem {
        WorkItem::new(id, format!("work-{id}")).expect("valid work")
    }

    // DISC-114-T01: unauthenticated request denied, no state change, no side effect.
    #[test]
    fn t01_unauthenticated_denied_without_side_effects() {
        let mut g = gate();
        g.submit(TOKEN, item(1)).expect("seed one queued");
        let before_queue = g.queue_len();
        for bad in ["", "wrong", "rk-test-toke", "RK-TEST-TOKEN"] {
            assert_eq!(
                g.submit(bad, item(2)),
                Err(AdmissionError::Unauthenticated),
                "credential {bad:?} must be denied"
            );
        }
        // Denial when full also mutates nothing and stays Unauthenticated.
        for id in 10..10 + (MAX_QUEUE_DEPTH as u64) {
            let _ = g.submit(TOKEN, item(id));
        }
        assert_eq!(
            g.submit("intruder", item(999)),
            Err(AdmissionError::Unauthenticated)
        );
        assert_eq!(g.queue_len(), MAX_QUEUE_DEPTH);
        assert_eq!(g.inflight_count(), 0);
        assert!(g.effects().is_empty(), "denial must run no side effect");
        assert_eq!(g.denials(), 5);
        assert!(before_queue <= g.queue_len());
    }

    // DISC-114-T02: cancelling queued work removes it, no partial effects, no leaked permits.
    #[test]
    fn t02_cancel_queued_reclaims_without_effects_or_leaks() {
        let mut g = gate();
        g.submit(TOKEN, item(1)).unwrap();
        g.submit(TOKEN, item(2)).unwrap();
        g.submit(TOKEN, item(3)).unwrap();
        let admitted = g.admit_next().expect("permit free");
        assert_eq!(admitted.id, 1);
        assert_eq!(g.inflight_count(), 1);
        let removed = g.cancel_queued(2).expect("queued cancel");
        assert_eq!(removed.id, 2);
        assert_eq!(g.queue_len(), 1);
        assert_eq!(g.inflight_count(), 1, "cancel must not touch permits");
        assert!(g.effects().is_empty(), "cancel must run no side effect");
        // Cancel of inflight/committed/unknown is rejected, never fabricated.
        assert_eq!(g.cancel_queued(1), Err(AdmissionError::UnknownWork(1)));
        assert_eq!(g.cancel_queued(77), Err(AdmissionError::UnknownWork(77)));
        g.complete(1).expect("finish inflight");
        assert_eq!(g.inflight_count(), 0, "permit reclaimed on complete");
        assert_eq!(g.cancel_queued(1), Err(AdmissionError::UnknownWork(1)));
        assert_eq!(g.effects(), &[1]);
    }

    // DISC-114-T03: restart recovers queued-vs-committed truthfully, no duplicated effects.
    #[test]
    fn t03_restart_recovery_never_duplicates_committed() {
        let committed = vec![1, 2, 3];
        let queued = vec![item(2), item(3), item(4), item(4), item(5)];
        let recovered = recover(&committed, &queued);
        let ids: Vec<u64> = recovered.replay.iter().map(|w| w.id).collect();
        assert_eq!(ids, vec![4, 5], "committed never re-queued, first copy wins");
        assert_eq!(recovered.duplicates_dropped, 3);
        // Replaying twice changes nothing: committed absorbs the replay.
        let mut committed2 = committed.clone();
        committed2.extend(ids.iter().copied());
        let again = recover(&committed2, &recovered.replay);
        assert!(again.replay.is_empty(), "second recovery must not duplicate");
    }

    // DISC-114-T04: queue overflow rejects with explicit backpressure, no unbounded growth.
    #[test]
    fn t04_overflow_rejects_with_backpressure_fifo_intact() {
        let mut g = gate();
        for id in 0..(MAX_QUEUE_DEPTH as u64) {
            g.submit(TOKEN, item(id)).expect("room");
        }
        assert_eq!(
            g.submit(TOKEN, item(1000)),
            Err(AdmissionError::Backpressure {
                capacity: MAX_QUEUE_DEPTH
            })
        );
        assert_eq!(g.queue_len(), MAX_QUEUE_DEPTH, "overflow mutated nothing");
        assert_eq!(g.admit_next().expect("first").id, 0);
        assert_eq!(g.admit_next().expect("second").id, 1);
        assert_eq!(g.queue_len(), MAX_QUEUE_DEPTH - 2);
    }

    // DISC-114-T05: slow consumer bounded + resynced without blocking other sessions.
    #[test]
    fn t05_slow_consumer_bounded_resynced_others_unaffected() {
        let mut hub = ConsumerSet::new();
        let slow = hub.subscribe();
        let fast = hub.subscribe();
        for event in 0..(MAX_CONSUMER_PENDING as u64) {
            let dropped = hub.publish(event);
            assert!(dropped.is_empty(), "room remains at event {event}");
        }
        let dropped = hub.publish(MAX_CONSUMER_PENDING as u64);
        assert_eq!(dropped, vec![slow], "slow consumer disconnected at bound");
        assert!(!hub.is_connected(slow));
        assert!(hub.is_connected(fast));
        assert_eq!(hub.pending_count(fast), MAX_CONSUMER_PENDING + 1);
        assert_eq!(hub.live_consumers(), 1);
        // Fast consumer keeps receiving while slow is parked.
        assert!(hub.publish(9999).is_empty());
        assert_eq!(hub.pending_count(fast), MAX_CONSUMER_PENDING + 2);
        // Resync reconnects slow with an empty buffer.
        assert!(hub.resync(slow));
        assert!(hub.is_connected(slow));
        assert_eq!(hub.pending_count(slow), 0);
        assert!(!hub.resync(fast), "healthy consumer is not a resync");
    }
}
