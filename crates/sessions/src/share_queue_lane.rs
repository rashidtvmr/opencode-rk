//! SHARE-002 bounded local share-event coalescing queue (lane fragment).
//!
//! Pure: no network, no DB mutation, no clock, no thread/timer, no logging of
//! values. Caller supplies location filter `accepts(session) -> bool` and drives
//! the delayed flush (`drain`); on downstream failure caller retains the batch
//! and calls `requeue`. Latest value wins per `(session, kind, id)`.
//!
//! Lane-owned module: included by the test via `#[path]`; the integrator
//! wires `pub mod share_queue_lane;` into `lib.rs` later. Distinct `Lane*`
//! type names so this file never collides with `share_queue.rs`.
//!
//! Security: `Debug` impls and error variants carry keys/counts only, never
//! event-value bytes. Values carry no secrets by contract.
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fmt;

use thiserror::Error;

/// Identity key for coalescing: `(kind, id)` within a session.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LaneDataKey {
    pub kind: String,
    pub id: String,
}

/// One shareable event. Full identity is `(session, kind, id)`.
/// Debug redacts the value (shows `value_len` only); log keys/counts, never values.
#[derive(Clone, PartialEq, Eq)]
pub struct LaneShareEvent {
    pub session: String,
    pub key: LaneDataKey,
    pub value: Vec<u8>,
}

impl fmt::Debug for LaneShareEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LaneShareEvent")
            .field("session", &self.session)
            .field("key", &self.key)
            .field("value_len", &self.value.len())
            .finish_non_exhaustive()
    }
}

/// Resource caps. All three bounds enforced simultaneously.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaneQueueCaps {
    pub max_items: usize,
    pub max_bytes: usize,
    pub max_sessions: usize,
}

impl Default for LaneQueueCaps {
    fn default() -> Self {
        Self {
            max_items: 4096,
            max_bytes: 8_388_608,
            max_sessions: 256,
        }
    }
}

/// Failure modes. Carry no value bytes.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LaneQueueError {
    #[error("share queue finalized")]
    Finalized,
    #[error("share event too large")]
    TooLarge,
}

/// Count-only snapshot. Safe to log; carries no value bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaneQueueStats {
    pub len: usize,
    pub bytes: usize,
    pub filtered: u64,
    pub evicted: u64,
}

type FullKey = (String, String, String);

/// Bounded latest-value-per-key queue with insertion-order eviction.
pub struct LaneCoalescingQueue {
    caps: LaneQueueCaps,
    accepts: Box<dyn Fn(&str) -> bool>,
    // ponytail: BTreeMap + insertion-order VecDeque instead of IndexMap
    // (no new dep); O(n) removal, fine at max_items=4096 ceiling.
    entries: BTreeMap<FullKey, LaneShareEvent>,
    order: VecDeque<FullKey>,
    order_set: HashSet<FullKey>,
    bytes: usize,
    sessions: HashMap<String, usize>,
    filtered: u64,
    evicted: u64,
    finalized: bool,
}

// Debug carries counts only, never values, keys, or sessions.
impl fmt::Debug for LaneCoalescingQueue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LaneCoalescingQueue")
            .field("len", &self.entries.len())
            .field("bytes", &self.bytes)
            .field("filtered", &self.filtered)
            .field("evicted", &self.evicted)
            .field("finalized", &self.finalized)
            .finish_non_exhaustive()
    }
}

impl LaneCoalescingQueue {
    pub fn new(caps: LaneQueueCaps, accepts: impl Fn(&str) -> bool + 'static) -> Self {
        Self {
            caps,
            accepts: Box::new(accepts),
            entries: BTreeMap::new(),
            order: VecDeque::new(),
            order_set: HashSet::new(),
            bytes: 0,
            sessions: HashMap::new(),
            filtered: 0,
            evicted: 0,
            finalized: false,
        }
    }

    fn oversized(&self, value_len: usize) -> bool {
        value_len > self.caps.max_bytes / 8
    }

    fn full_key(event: &LaneShareEvent) -> FullKey {
        (
            event.session.clone(),
            event.key.kind.clone(),
            event.key.id.clone(),
        )
    }

    fn touch_order(&mut self, key: &FullKey) {
        if self.order_set.insert(key.clone()) {
            self.order.push_back(key.clone());
        }
    }

    fn remove_key(&mut self, key: &FullKey) {
        if let Some(old) = self.entries.remove(key) {
            self.bytes = self.bytes.saturating_sub(old.value.len());
            let (session, _, _) = key;
            if let Some(n) = self.sessions.get_mut(session) {
                *n = n.saturating_sub(1);
                if *n == 0 {
                    self.sessions.remove(session);
                }
            }
        }
        if self.order_set.remove(key) {
            self.order.retain(|k| k != key);
        }
    }

    fn evict_oldest(&mut self) {
        if let Some(oldest) = self.order.pop_front() {
            self.order_set.remove(&oldest);
            if let Some(old) = self.entries.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(old.value.len());
                let (session, _, _) = &oldest;
                if let Some(n) = self.sessions.get_mut(session) {
                    *n = n.saturating_sub(1);
                    if *n == 0 {
                        self.sessions.remove(session);
                    }
                }
            }
            self.evicted = self.evicted.saturating_add(1);
        }
    }

    fn insert_inner(&mut self, event: LaneShareEvent) {
        // Degenerate caps admit nothing: drop, count eviction, no loop.
        if self.caps.max_items == 0 || self.caps.max_bytes == 0 || self.caps.max_sessions == 0 {
            self.evicted = self.evicted.saturating_add(1);
            return;
        }
        let key = Self::full_key(&event);
        // Replace path: remove old accounting first, then admit as new.
        if self.entries.contains_key(&key) {
            self.remove_key(&key);
        }
        let is_new_session = !self.sessions.contains_key(&event.session);
        if is_new_session {
            while self.sessions.len() >= self.caps.max_sessions {
                // Guarded non-zero above; queue non-empty here when capped,
                // but break defensively against a no-op eviction.
                let before = self.entries.len();
                self.evict_oldest();
                if self.entries.len() == before {
                    self.evicted = self.evicted.saturating_add(1);
                    return;
                }
            }
        }
        while self.entries.len() + 1 > self.caps.max_items
            || self.bytes + event.value.len() > self.caps.max_bytes
        {
            let before = self.entries.len();
            self.evict_oldest();
            if self.entries.len() == before {
                // Value fits oversize check but not remaining cap (cap <
                // single value allowance): drop, count eviction.
                self.evicted = self.evicted.saturating_add(1);
                return;
            }
        }
        self.bytes += event.value.len();
        *self.sessions.entry(event.session.clone()).or_insert(0) += 1;
        self.touch_order(&key);
        self.entries.insert(key, event);
    }

    /// Latest value wins per key. Non-accepted locations dropped (`filtered += 1`).
    /// Oldest key evicted on overflow (`evicted += 1`).
    pub fn push(&mut self, event: LaneShareEvent) -> Result<(), LaneQueueError> {
        if self.finalized {
            return Err(LaneQueueError::Finalized);
        }
        if !(self.accepts)(&event.session) {
            self.filtered = self.filtered.saturating_add(1);
            return Ok(());
        }
        if self.oversized(event.value.len()) {
            return Err(LaneQueueError::TooLarge);
        }
        self.insert_inner(event);
        Ok(())
    }

    /// Sorted `(session, kind, id)` batch; clears queue exactly once.
    pub fn drain(&mut self) -> Vec<LaneShareEvent> {
        let out: Vec<LaneShareEvent> = self.entries.values().cloned().collect();
        self.entries.clear();
        self.order.clear();
        self.order_set.clear();
        self.sessions.clear();
        self.bytes = 0;
        out
    }

    /// Retain-on-failure retry: reinsert a drained batch in given order.
    /// Non-accepted entries count as `filtered`; oversized entries abort with
    /// `TooLarge` (earlier reinserted entries retained).
    pub fn requeue(&mut self, batch: Vec<LaneShareEvent>) -> Result<(), LaneQueueError> {
        if self.finalized {
            return Err(LaneQueueError::Finalized);
        }
        for event in batch {
            if !(self.accepts)(&event.session) {
                self.filtered = self.filtered.saturating_add(1);
                continue;
            }
            if self.oversized(event.value.len()) {
                return Err(LaneQueueError::TooLarge);
            }
            self.insert_inner(event);
        }
        Ok(())
    }

    /// Clear queue + cache; reject all future pushes/requeues.
    pub fn finalize(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.order_set.clear();
        self.sessions.clear();
        self.bytes = 0;
        self.finalized = true;
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    #[must_use]
    pub fn filtered(&self) -> u64 {
        self.filtered
    }

    #[must_use]
    pub fn evicted(&self) -> u64 {
        self.evicted
    }

    #[must_use]
    pub fn stats(&self) -> LaneQueueStats {
        LaneQueueStats {
            len: self.entries.len(),
            bytes: self.bytes,
            filtered: self.filtered,
            evicted: self.evicted,
        }
    }
}
